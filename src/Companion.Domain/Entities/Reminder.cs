namespace Companion.Domain.Entities;

public enum ReminderScheduleType
{
    Once,
    Daily,
    Weekdays,
    Weekly
}

public enum ReminderStatus
{
    Active,
    Completed,
    Dismissed
}

public sealed record Reminder(
    Guid Id,
    Guid? TodoId,
    string Title,
    ReminderScheduleType ScheduleType,
    TimeOnly LocalTime,
    DayOfWeek[] Weekdays,
    DateOnly? StartDate,
    DateOnly? EndDate,
    DateTimeOffset NextTriggerAt,
    ReminderStatus Status,
    DateTimeOffset CreatedAt);
