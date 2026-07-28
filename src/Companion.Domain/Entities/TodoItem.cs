namespace Companion.Domain.Entities;

public enum TodoPriority
{
    Low = 0,
    Normal = 1,
    High = 2
}

public sealed record TodoItem(
    Guid Id,
    string Title,
    string? Note,
    TodoPriority Priority,
    DateTimeOffset? DueAt,
    DateTimeOffset? ReminderAt,
    DateTimeOffset? CompletedAt,
    DateTimeOffset CreatedAt,
    DateTimeOffset UpdatedAt,
    int EstimatedPomodoros = 1,
    int CompletedPomodoros = 0,
    DateTimeOffset? DueTime = null)
{
    public bool IsCompleted => CompletedAt is not null;
    public bool CanFocus => !IsCompleted;
    public DateTimeOffset? EffectiveDueAt => DueTime ?? DueAt;
    public bool IsOverdue => !IsCompleted &&
                             EffectiveDueAt is { } due &&
                             due < DateTimeOffset.Now;

    public TodoItem Complete(DateTimeOffset now) => this with
    {
        CompletedAt = now,
        UpdatedAt = now
    };

    public TodoItem Restore(DateTimeOffset now) => this with
    {
        CompletedAt = null,
        UpdatedAt = now
    };

    public TodoItem CompletePomodoro(DateTimeOffset now) => this with
    {
        CompletedPomodoros = CompletedPomodoros + 1,
        UpdatedAt = now
    };
}
