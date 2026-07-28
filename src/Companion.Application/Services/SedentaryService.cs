using Companion.Application.Abstractions;
using Companion.Application.Events;
using Companion.Domain.Entities;

namespace Companion.Application.Services;

public sealed class SedentaryService(
    IUserActivitySource activity,
    IClock clock,
    IEventBus eventBus)
{
    public TimeSpan Threshold { get; set; } = TimeSpan.FromMinutes(60);
    public TimeSpan EffectiveBreak { get; set; } = TimeSpan.FromMinutes(5);
    public int MaximumSnoozes { get; set; } = 3;

    public SedentaryState Evaluate(SedentaryState state)
    {
        var now = clock.Now;
        if (activity.IsSessionLocked || activity.GetIdleDuration() >= EffectiveBreak)
        {
            return new SedentaryState(now, now, 0, state.MutedUntil);
        }

        if (state.MutedUntil > now)
        {
            return state;
        }

        if (now - state.ActiveSince >= Threshold)
        {
            eventBus.Publish(new SedentaryThresholdReached(now));
            eventBus.Publish(new ActionRequested("reminder.sedentary", now));
        }

        return state;
    }

    public SedentaryState Snooze(SedentaryState state, TimeSpan duration)
    {
        if (state.SnoozeCount >= MaximumSnoozes)
        {
            return state;
        }

        return state with
        {
            SnoozeCount = state.SnoozeCount + 1,
            MutedUntil = clock.Now.Add(duration)
        };
    }

    public SedentaryState MuteForToday(SedentaryState state)
    {
        var tomorrow = DateOnly.FromDateTime(clock.Now.LocalDateTime).AddDays(1);
        return state with
        {
            MutedUntil = new DateTimeOffset(tomorrow.ToDateTime(TimeOnly.MinValue), clock.Now.Offset)
        };
    }
}
