using Companion.Domain.Entities;
using Companion.Domain.Scheduling;

namespace Companion.UnitTests;

public sealed class ReminderCalculatorTests
{
    [Fact]
    public void WeekdayScheduleSkipsWeekend()
    {
        var zone = TimeZoneInfo.Utc;
        var fridayEvening = new DateTimeOffset(2026, 7, 24, 18, 0, 0, TimeSpan.Zero);
        var reminder = new Reminder(
            Guid.NewGuid(),
            null,
            "工作日提醒",
            ReminderScheduleType.Weekdays,
            new TimeOnly(9, 0),
            [],
            new DateOnly(2026, 7, 24),
            null,
            fridayEvening,
            ReminderStatus.Active,
            fridayEvening);

        var next = new ReminderCalculator().GetNext(reminder, fridayEvening, zone);

        Assert.Equal(
            new DateTimeOffset(2026, 7, 27, 9, 0, 0, TimeSpan.Zero),
            next);
    }
}
