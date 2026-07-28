using Companion.Application.Abstractions;
using Companion.Application.Events;
using Companion.Domain.Entities;

namespace Companion.Application.Services;

public sealed class PomodoroService(ICompanionStore store, IClock clock, IEventBus eventBus)
{
    public TimeSpan FocusDuration { get; set; } = TimeSpan.FromMinutes(25);
    public TimeSpan ShortBreakDuration { get; set; } = TimeSpan.FromMinutes(5);
    public TimeSpan LongBreakDuration { get; set; } = TimeSpan.FromMinutes(15);
    public int LongBreakEvery { get; set; } = 4;

    public async Task<PomodoroSession> StartAsync(
        PomodoroPhase phase = PomodoroPhase.Focus,
        int completedRounds = 0,
        CancellationToken cancellationToken = default)
    {
        var now = clock.Now;
        var duration = GetDuration(phase);
        var session = new PomodoroSession(
            Guid.NewGuid(),
            phase,
            now,
            now.Add(duration),
            null,
            (int)duration.TotalSeconds,
            completedRounds,
            PomodoroStatus.Running);
        await store.UpsertPomodoroAsync(session, cancellationToken);
        eventBus.Publish(new ActionRequested(phase == PomodoroPhase.Focus ? "focus" : "relax", now));
        return session;
    }

    public async Task<PomodoroSession?> RestoreAsync(CancellationToken cancellationToken = default)
    {
        var session = await store.GetCurrentPomodoroAsync(cancellationToken);
        if (session is null || session.Status != PomodoroStatus.Running)
        {
            return session;
        }

        if (clock.Now < session.ExpectedEndAt)
        {
            return session with
            {
                RemainingSeconds = Math.Max(0, (int)(session.ExpectedEndAt - clock.Now).TotalSeconds)
            };
        }

        var completed = session with { Status = PomodoroStatus.Completed, RemainingSeconds = 0 };
        await store.UpsertPomodoroAsync(completed, cancellationToken);
        eventBus.Publish(new PomodoroPhaseEnded(session.Phase.ToString(), clock.Now));
        eventBus.Publish(new ActionRequested("celebrate", clock.Now));
        return completed;
    }

    public async Task<PomodoroSession> PauseAsync(
        PomodoroSession session,
        CancellationToken cancellationToken = default)
    {
        var remaining = Math.Max(0, (int)(session.ExpectedEndAt - clock.Now).TotalSeconds);
        var paused = session with
        {
            PausedAt = clock.Now,
            RemainingSeconds = remaining,
            Status = PomodoroStatus.Paused
        };
        await store.UpsertPomodoroAsync(paused, cancellationToken);
        return paused;
    }

    public async Task<PomodoroSession> ResumeAsync(
        PomodoroSession session,
        CancellationToken cancellationToken = default)
    {
        if (session.PausedAt is not null && clock.Now - session.PausedAt > TimeSpan.FromHours(2))
        {
            var stopped = session with { Status = PomodoroStatus.Stopped };
            await store.UpsertPomodoroAsync(stopped, cancellationToken);
            return stopped;
        }

        var resumed = session with
        {
            PausedAt = null,
            ExpectedEndAt = clock.Now.AddSeconds(session.RemainingSeconds),
            Status = PomodoroStatus.Running
        };
        await store.UpsertPomodoroAsync(resumed, cancellationToken);
        return resumed;
    }

    public PomodoroPhase GetNextPhase(PomodoroSession completed)
    {
        if (completed.Phase != PomodoroPhase.Focus)
        {
            return PomodoroPhase.Focus;
        }

        var round = completed.CompletedFocusRounds + 1;
        return round % LongBreakEvery == 0 ? PomodoroPhase.LongBreak : PomodoroPhase.ShortBreak;
    }

    private TimeSpan GetDuration(PomodoroPhase phase) => phase switch
    {
        PomodoroPhase.Focus => FocusDuration,
        PomodoroPhase.ShortBreak => ShortBreakDuration,
        PomodoroPhase.LongBreak => LongBreakDuration,
        _ => throw new ArgumentOutOfRangeException(nameof(phase))
    };
}
