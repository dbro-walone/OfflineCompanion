use super::{
    event::{HitRegion, PetEvent},
    scheduler::{Priority, ScheduledAction, Scheduler},
    session::InteractionSession,
    state::{Mood, MoodState, PetMemory, PetState},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionTrace {
    pub event: String,
    pub state: PetState,
    pub mood: Mood,
    pub selected_action: Option<String>,
    pub reason: String,
    pub fallback: Option<String>,
}

#[derive(Debug)]
pub struct BehaviorController {
    pub state: PetState,
    pub mood: MoodState,
    pub memory: PetMemory,
    pub scheduler: Scheduler,
    pub session: Option<InteractionSession>,
    pub proactive_enabled: bool,
    pub reduce_motion: bool,
    pub interaction_cooldown_ms: u64,
    last_invite_ms: Option<u64>,
    pub traces: Vec<DecisionTrace>,
}
impl Default for BehaviorController {
    fn default() -> Self {
        Self {
            state: PetState::Idle,
            mood: MoodState::default(),
            memory: PetMemory::default(),
            scheduler: Scheduler::default(),
            session: None,
            proactive_enabled: true,
            reduce_motion: false,
            interaction_cooldown_ms: 30_000,
            last_invite_ms: None,
            traces: vec![],
        }
    }
}
impl BehaviorController {
    pub fn handle(&mut self, event: PetEvent, now_ms: u64) -> Option<ScheduledAction> {
        self.memory.last_event = Some(format!("{event:?}"));
        self.mood.tick(now_ms);
        let mut reason = "event observed";
        let mut fallback = None;
        let request = match &event {
            PetEvent::AppStarted => {
                self.state = PetState::Idle;
                reason = "runtime started";
                Some(self.action("idle", Priority::IdleRandom, now_ms))
            }
            PetEvent::PointerNear { .. }
                if self.proactive_enabled
                    && self.last_invite_ms.is_none_or(|x| {
                        now_ms.saturating_sub(x) >= self.interaction_cooldown_ms
                    }) =>
            {
                self.state = PetState::WaitingForResponse;
                self.mood.set(Mood::Curious, now_ms, 4000);
                self.session = Some(InteractionSession::new(now_ms));
                self.last_invite_ms = Some(now_ms);
                reason = if self.reduce_motion {
                    "reduced-motion in-place invite"
                } else {
                    "pointer-near invite"
                };
                Some(self.action("look", Priority::ProactiveInvite, now_ms))
            }
            PetEvent::PetClicked { region, .. } => {
                if let Some(s) = self.session.as_mut() {
                    s.click();
                }
                self.state = PetState::Playing;
                self.mood.set(Mood::Playful, now_ms, 1800);
                reason = "direct click";
                Some(self.action(
                    match region {
                        HitRegion::Head => "head-pat",
                        HitRegion::Body => "clicked",
                    },
                    Priority::UserDirectInput,
                    now_ms,
                ))
            }
            PetEvent::PointerExited => {
                if let Some(s) = self.session.as_mut() {
                    s.pointer_left();
                }
                self.state = PetState::Idle;
                reason = "pointer left";
                Some(self.action("idle", Priority::IdleRandom, now_ms))
            }
            PetEvent::DragStarted { .. } => {
                self.state = PetState::Dragging;
                reason = "drag started";
                Some(self.action("drag", Priority::UserDirectInput, now_ms))
            }
            PetEvent::DragReleased { .. } => {
                self.state = if self.reduce_motion {
                    PetState::Idle
                } else {
                    PetState::Falling
                };
                reason = "drag released";
                Some(self.action(
                    if self.reduce_motion { "idle" } else { "fall" },
                    Priority::SystemSafety,
                    now_ms,
                ))
            }
            PetEvent::ReminderRaised { .. } | PetEvent::SedentaryWarning => {
                self.state = PetState::Observing;
                self.mood.set(Mood::Curious, now_ms, 4000);
                reason = "business attention";
                Some(self.action("reminder", Priority::ReminderAttention, now_ms))
            }
            PetEvent::PomodoroStarted => {
                self.state = PetState::FocusCompanion;
                self.mood.set(Mood::Focused, now_ms, u64::MAX - now_ms);
                reason = "focus companion";
                Some(self.action("focus", Priority::FocusCompanion, now_ms))
            }
            PetEvent::PomodoroPaused => {
                self.state = PetState::Idle;
                reason = "focus paused";
                Some(self.action("relax", Priority::BusinessFeedback, now_ms))
            }
            PetEvent::PomodoroCompleted => {
                self.state = PetState::Playing;
                self.mood.set(Mood::Happy, now_ms, 3000);
                reason = "focus completed";
                Some(self.action("celebrate", Priority::BusinessFeedback, now_ms))
            }
            PetEvent::TodoCompleted { .. } | PetEvent::ReminderCompleted { .. } => {
                self.mood.set(Mood::Happy, now_ms, 2200);
                reason = "task completed";
                Some(self.action("celebrate", Priority::BusinessFeedback, now_ms))
            }
            PetEvent::Tick { .. } => {
                if let Some(s) = self.session.as_mut() {
                    s.tick(now_ms);
                    if s.end().is_some() {
                        self.session = None;
                        self.state = PetState::Idle;
                        reason = "interaction session ended";
                        Some(self.action("idle", Priority::IdleRandom, now_ms))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        };
        let had_request = request.is_some();
        let selected = request.filter(|r| self.scheduler.submit(r.clone()));
        if had_request && selected.is_none() {
            fallback = Some("higher-priority action retained".into());
        }
        if let Some(x) = selected.as_ref() {
            self.memory.record_action(&x.id)
        }
        self.traces.push(DecisionTrace {
            event: format!("{event:?}"),
            state: self.state,
            mood: self.mood.mood,
            selected_action: selected.as_ref().map(|x| x.id.clone()),
            reason: reason.into(),
            fallback,
        });
        if self.traces.len() > 128 {
            self.traces.remove(0);
        }
        selected
    }
    fn action(&self, id: &str, priority: Priority, now_ms: u64) -> ScheduledAction {
        ScheduledAction {
            id: id.into(),
            priority,
            not_before_ms: now_ms,
        }
    }
    pub fn complete_current(&mut self) -> Option<ScheduledAction> {
        self.scheduler.take()
    }
}
