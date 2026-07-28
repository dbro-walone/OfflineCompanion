using Companion.Application.Abstractions;
using Companion.Application.Events;
using Companion.Domain.Entities;
using Companion.Domain.Scheduling;

namespace Companion.Application.Services;

public sealed class ReminderService(
    ICompanionStore store,
    IClock clock,
    IEventBus eventBus,
    ReminderCalculator calculator)
{
    public TimeSpan MissedReminderWindow { get; init; } = TimeSpan.FromHours(24);

    public async Task<Reminder> CreateOneTimeAsync(
        string title,
        DateTimeOffset triggerAt,
        Guid? todoId = null,
        CancellationToken cancellationToken = default)
    {
        if (string.IsNullOrWhiteSpace(title))
        {
            throw new ArgumentException("提醒标题不能为空。", nameof(title));
        }

        if (triggerAt <= clock.Now)
        {
            throw new ArgumentOutOfRangeException(nameof(triggerAt), "提醒时间必须晚于当前时间。");
        }

        var local = triggerAt.LocalDateTime;
        var reminder = new Reminder(
            Guid.NewGuid(),
            todoId,
            title.Trim(),
            ReminderScheduleType.Once,
            TimeOnly.FromDateTime(local),
            [],
            DateOnly.FromDateTime(local),
            DateOnly.FromDateTime(local),
            triggerAt,
            ReminderStatus.Active,
            clock.Now);
        await store.UpsertReminderAsync(reminder, cancellationToken);
        return reminder;
    }

    public async Task CheckDueAsync(CancellationToken cancellationToken = default)
    {
        var now = clock.Now;
        var reminders = await store.GetActiveRemindersAsync(cancellationToken);
        foreach (var reminder in reminders.Where(x => x.NextTriggerAt <= now))
        {
            var lateness = now - reminder.NextTriggerAt;
            if (lateness <= MissedReminderWindow)
            {
                eventBus.Publish(new ReminderDue(reminder.Id, reminder.Title, now));
                eventBus.Publish(new ActionRequested(
                    reminder.TodoId is null ? "reminder.timer" : "reminder.todo",
                    now));
            }

            var next = calculator.GetNext(reminder, now, TimeZoneInfo.Local);
            var updated = next is null
                ? reminder with { Status = ReminderStatus.Completed }
                : reminder with { NextTriggerAt = next.Value };
            await store.UpsertReminderAsync(updated, cancellationToken);
        }
    }

    public async Task SnoozeAsync(
        Reminder reminder,
        TimeSpan duration,
        CancellationToken cancellationToken = default)
    {
        if (duration <= TimeSpan.Zero)
        {
            throw new ArgumentOutOfRangeException(nameof(duration));
        }

        await store.UpsertReminderAsync(
            reminder with { NextTriggerAt = clock.Now.Add(duration), Status = ReminderStatus.Active },
            cancellationToken);
    }
}
