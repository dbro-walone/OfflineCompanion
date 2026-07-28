namespace Companion.Domain.Entities;

public enum PomodoroPhase
{
    Focus,
    ShortBreak,
    LongBreak
}

public enum PomodoroStatus
{
    Running,
    Paused,
    Completed,
    Stopped
}

public sealed record PomodoroSession(
    Guid Id,
    PomodoroPhase Phase,
    DateTimeOffset StartedAt,
    DateTimeOffset ExpectedEndAt,
    DateTimeOffset? PausedAt,
    int RemainingSeconds,
    int CompletedFocusRounds,
    PomodoroStatus Status);
