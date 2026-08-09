use std::time::Instant;

use crate::animation::definition::ResumePolicy;

#[derive(Debug, Clone)]
pub enum PetState {
    Idle,
    Focus,
    Relax,
    Attention,
    Dragging,
    OnEdge,
    Sleeping,
}

#[derive(Debug, Clone)]
pub enum ActionSource {
    UserInteraction,
    Reminder,
    Pomodoro,
    IdleScheduler,
    SystemEvent,
}

#[derive(Debug, Clone)]
pub enum InterruptPolicy {
    None,
    HigherOnly,
    Any,
}

#[derive(Debug, Clone)]
pub struct ActionRequest {
    pub action_id: String,
    pub source: ActionSource,
    pub priority: u8,
    pub interrupt: InterruptPolicy,
    pub resume: ResumePolicy,
    pub created_at: Instant,
    pub expires_at: Option<Instant>,
}
