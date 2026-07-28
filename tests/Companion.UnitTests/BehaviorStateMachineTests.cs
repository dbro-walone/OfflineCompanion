using Companion.Domain.Behavior;

namespace Companion.UnitTests;

public sealed class BehaviorStateMachineTests
{
    [Fact]
    public void HigherPriorityRequestInterruptsLowerPriorityState()
    {
        var machine = new BehaviorStateMachine();
        machine.Complete();
        machine.Request(new(
            BehaviorState.Idle,
            BehaviorPriority.P8Idle,
            "idle"));

        var transition = machine.Request(new(
            BehaviorState.Alerting,
            BehaviorPriority.P2DueAlert,
            "reminder.todo"));

        Assert.Equal(BehaviorState.Alerting, transition.Current);
        Assert.Equal("reminder.todo", machine.Current.ActionId);
    }

    [Fact]
    public void LowerPriorityRequestCannotInterruptDragging()
    {
        var machine = new BehaviorStateMachine();
        machine.Complete();
        machine.Request(new(
            BehaviorState.Dragging,
            BehaviorPriority.P1UserOperation,
            "dragged",
            Interruptible: false));

        var transition = machine.Request(new(
            BehaviorState.IdleAction,
            BehaviorPriority.P7IdleAction,
            "idle.thinking"));

        Assert.Equal(BehaviorState.Dragging, transition.Current);
        Assert.Equal("dragged", machine.Current.ActionId);
    }
}
