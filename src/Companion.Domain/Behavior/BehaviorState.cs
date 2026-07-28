namespace Companion.Domain.Behavior;

public enum BehaviorState
{
    Starting,
    Idle,
    IdleAction,
    Interacting,
    Dragging,
    Edge,
    Focus,
    Rest,
    Alerting,
    Celebrating,
    Faulted
}

public enum BehaviorPriority
{
    P0System = 0,
    P1UserOperation = 1,
    P2DueAlert = 2,
    P3UserFeedback = 3,
    P4Pomodoro = 4,
    P5Sedentary = 5,
    P6Edge = 6,
    P7IdleAction = 7,
    P8Idle = 8
}

public enum ResumePolicy
{
    Restart,
    Resume,
    Idle
}

public sealed record BehaviorRequest(
    BehaviorState Target,
    BehaviorPriority Priority,
    string ActionId,
    bool Interruptible = true,
    ResumePolicy ResumePolicy = ResumePolicy.Idle);

public sealed record BehaviorTransition(
    BehaviorState Previous,
    BehaviorState Current,
    string ActionId,
    BehaviorRequest? Interrupted);
