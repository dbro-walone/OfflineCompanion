namespace Companion.Infrastructure.Config;

public sealed record AppSettings
{
    public int SchemaVersion { get; init; } = 1;
    public string CurrentCharacterId { get; init; } = "character.shadow-crow-ninja";
    public double PetScale { get; init; } = 1;
    public double? PetLeft { get; init; }
    public double? PetTop { get; init; }
    public bool Topmost { get; init; } = true;
    public bool IdleActionsEnabled { get; init; } = true;
    public bool EdgeActionsEnabled { get; init; } = true;
    public bool PauseAnimations { get; init; }
    public bool ReduceMotion { get; init; }
    public bool SilentWhenFullscreen { get; init; } = true;
    public string Theme { get; init; } = "dark";
    public int PomodoroFocusMinutes { get; init; } = 25;
    public int PomodoroShortBreakMinutes { get; init; } = 5;
    public int PomodoroLongBreakMinutes { get; init; } = 15;
    public int PomodoroLongBreakEvery { get; init; } = 4;
    public int SedentaryThresholdMinutes { get; init; } = 60;
    public int SedentaryBreakMinutes { get; init; } = 5;
    public int SedentarySnoozeMinutes { get; init; } = 10;
}
