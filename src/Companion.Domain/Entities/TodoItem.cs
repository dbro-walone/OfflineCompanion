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
    DateTimeOffset UpdatedAt)
{
    public bool IsCompleted => CompletedAt is not null;

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
}
