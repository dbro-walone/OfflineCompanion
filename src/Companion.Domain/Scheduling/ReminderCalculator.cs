using Companion.Domain.Entities;

namespace Companion.Domain.Scheduling;

public sealed class ReminderCalculator
{
    public DateTimeOffset? GetNext(Reminder reminder, DateTimeOffset after, TimeZoneInfo zone)
    {
        if (reminder.Status != ReminderStatus.Active)
        {
            return null;
        }

        var localAfter = TimeZoneInfo.ConvertTime(after, zone);
        var start = reminder.StartDate ?? DateOnly.FromDateTime(localAfter.DateTime);
        var candidateDate = DateOnly.FromDateTime(localAfter.DateTime);
        if (candidateDate < start)
        {
            candidateDate = start;
        }

        for (var offset = 0; offset <= 370; offset++)
        {
            var date = candidateDate.AddDays(offset);
            if (reminder.EndDate is not null && date > reminder.EndDate)
            {
                return null;
            }

            if (!MatchesSchedule(reminder, date))
            {
                continue;
            }

            var localDateTime = date.ToDateTime(reminder.LocalTime, DateTimeKind.Unspecified);
            var utc = TimeZoneInfo.ConvertTimeToUtc(localDateTime, zone);
            var result = new DateTimeOffset(utc);
            if (result > after)
            {
                return result;
            }
        }

        return null;
    }

    private static bool MatchesSchedule(Reminder reminder, DateOnly date) =>
        reminder.ScheduleType switch
        {
            ReminderScheduleType.Once => date == (reminder.StartDate ?? date),
            ReminderScheduleType.Daily => true,
            ReminderScheduleType.Weekdays => date.DayOfWeek is not DayOfWeek.Saturday and not DayOfWeek.Sunday,
            ReminderScheduleType.Weekly => reminder.Weekdays.Contains(date.DayOfWeek),
            _ => false
        };
}
