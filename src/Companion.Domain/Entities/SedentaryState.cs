namespace Companion.Domain.Entities;

public sealed record SedentaryState(
    DateTimeOffset ActiveSince,
    DateTimeOffset? LastBreakAt,
    int SnoozeCount,
    DateTimeOffset? MutedUntil)
{
    public static SedentaryState New(DateTimeOffset now) => new(now, null, 0, null);
}
