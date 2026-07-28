namespace Companion.Domain.Behavior;

public sealed class BehaviorStateMachine
{
    private BehaviorRequest _current = new(
        BehaviorState.Starting,
        BehaviorPriority.P0System,
        "startup",
        Interruptible: true);

    private BehaviorRequest? _suspended;

    public BehaviorRequest Current => _current;

    public BehaviorTransition Request(BehaviorRequest request)
    {
        var previous = _current;
        if (!CanInterrupt(previous, request))
        {
            return new(previous.Target, previous.Target, previous.ActionId, request);
        }

        _suspended = previous.ResumePolicy == ResumePolicy.Resume ? previous : null;
        _current = request;
        return new(previous.Target, request.Target, request.ActionId, previous);
    }

    public BehaviorTransition Complete()
    {
        var previous = _current;
        _current = _suspended ?? new(
            BehaviorState.Idle,
            BehaviorPriority.P8Idle,
            "idle",
            Interruptible: true);
        _suspended = null;
        return new(previous.Target, _current.Target, _current.ActionId, null);
    }

    private static bool CanInterrupt(BehaviorRequest current, BehaviorRequest incoming)
    {
        if (incoming.Priority < current.Priority)
        {
            return true;
        }

        return current.Interruptible && incoming.Priority == current.Priority;
    }
}
