namespace Companion.Application.Events;

public abstract record CompanionEvent(DateTimeOffset OccurredAt);

public sealed record ActionRequested(
    string ActionId,
    DateTimeOffset OccurredAt) : CompanionEvent(OccurredAt);

public sealed record ReminderDue(
    Guid ReminderId,
    string Title,
    DateTimeOffset OccurredAt) : CompanionEvent(OccurredAt);

public sealed record PomodoroPhaseEnded(
    string Phase,
    DateTimeOffset OccurredAt) : CompanionEvent(OccurredAt);

public sealed record SedentaryThresholdReached(
    DateTimeOffset OccurredAt) : CompanionEvent(OccurredAt);
