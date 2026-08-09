#[derive(Debug, Clone)]
pub enum ReminderKind {
    Default,
    Todo,
    Timer,
    Sedentary,
}

#[derive(Debug, Clone)]
pub enum PetEvent {
    PointerEntered,
    PointerExited,
    Clicked,
    DragStarted,
    DragMoved { x: i32, y: i32 },
    DragReleased { velocity_x: f32, velocity_y: f32 },
    ReachedLeftEdge,
    ReachedRightEdge,
    ReminderRaised { kind: ReminderKind, text: String },
    ReminderDismissed,
    PomodoroStarted,
    PomodoroPaused,
    PomodoroFinished,
    TodoCompleted,
    UserBecameActive,
    UserBecameInactive,
    IdleWindowElapsed,
    SleepWindowElapsed,
    PackageRuntimeReloaded,
}
