namespace Companion.Domain.Behavior;

public sealed record ActionCandidate(
    string Id,
    string Trigger,
    int Priority,
    int PackPriority,
    int Weight,
    DateTimeOffset? CooldownUntil,
    bool ResourceAvailable,
    bool ExplicitOverride = false);

public sealed class ActionSelector
{
    private readonly Random _random;

    public ActionSelector(Random? random = null)
    {
        _random = random ?? Random.Shared;
    }

    public string Select(
        IEnumerable<ActionCandidate> candidates,
        string trigger,
        DateTimeOffset now,
        string defaultAction)
    {
        var available = candidates
            .Where(x => x.Trigger == trigger)
            .Where(x => x.ResourceAvailable)
            .Where(x => x.CooldownUntil is null || x.CooldownUntil <= now)
            .OrderByDescending(x => x.Priority)
            .ThenByDescending(x => x.ExplicitOverride)
            .ThenByDescending(x => x.PackPriority)
            .ToArray();

        if (available.Length == 0)
        {
            return defaultAction;
        }

        var top = available[0];
        var peers = available
            .Where(x => x.Priority == top.Priority)
            .Where(x => x.ExplicitOverride == top.ExplicitOverride)
            .Where(x => x.PackPriority == top.PackPriority)
            .ToArray();

        var totalWeight = peers.Sum(x => Math.Max(1, x.Weight));
        var roll = _random.Next(totalWeight);
        foreach (var candidate in peers)
        {
            roll -= Math.Max(1, candidate.Weight);
            if (roll < 0)
            {
                return candidate.Id;
            }
        }

        return peers[0].Id;
    }
}
