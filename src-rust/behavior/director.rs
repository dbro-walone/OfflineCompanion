use super::{
    emotion::{EmotionState, Personality},
    event::{HitRegion, PetEvent},
    locomotion::ReleasePath,
    planner::{BehaviorContext, BehaviorIntent, BehaviorPlanner},
    scheduler::{Priority, ScheduledAction, Scheduler},
    session::InteractionSession,
    state::{Mood, MoodState, PetMemory, PetState},
    state_model::PetStats,
    tree::BehaviorTree,
};
use crate::package_runtime::manifest::BehaviorMapping;

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
    pub planner: BehaviorPlanner,
    pub tree: BehaviorTree,
    /// Latest snapshot of the brain state the planner reads. The runtime feeds
    /// the emotion-driven fields via [`BehaviorController::observe`]; the
    /// controller refreshes the live fields each turn before planning.
    pub brain: BehaviorContext,
    /// Per-intent action overrides for the active character, consulted after the
    /// (character-independent) tree resolves a default action id. Empty by
    /// default, so the tree's mapping is used verbatim until a character pack
    /// supplies its own. Populated by the runtime when a character loads.
    pub behavior_overrides: BehaviorMapping,
    pub proactive_enabled: bool,
    pub allow_pet_approach: bool,
    pub allow_mouse_follow: bool,
    pub interaction_level: String,
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
            planner: BehaviorPlanner,
            tree: BehaviorTree,
            brain: BehaviorContext::rested(),
            behavior_overrides: BehaviorMapping::default(),
            proactive_enabled: true,
            allow_pet_approach: false,
            allow_mouse_follow: false,
            interaction_level: "balanced".into(),
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
        let mut fallback = None;

        // Refresh the controller-owned portion of the behavior context, then
        // snapshot it. Emotion-driven fields are fed in via `observe`.
        self.brain.state = self.state;
        self.brain.last_interaction_ms = self.last_invite_ms;
        self.brain.has_active_session = self.session.is_some();
        self.brain.proactive_allowed = self.proactive_invite_allowed(now_ms);
        let ctx = self.brain;

        // 1. New intent-based layer: planner -> intent -> tree -> action.
        //    The planner declines most physical/system events, in which case we
        //    fall back to the verified event->action mapping below.
        let candidate = self.planner.plan(&event, now_ms, &ctx);
        let planned = candidate.and_then(|intent| {
            self.tree
                .resolve(intent, &event, &ctx)
                .map(|resolved| (intent, resolved))
        });

        let (request, reason) = if let Some((intent, resolved)) = planned {
            self.apply_intent(intent, now_ms);
            // Let the active character re-express the resolved intent: the tree
            // supplies the character-independent default, the override (if any)
            // swaps in this character's preferred action for that intent.
            let action_id = self
                .behavior_overrides
                .resolve(intent.label(), resolved.action_id);
            (
                Some(self.action(action_id, resolved.priority, now_ms)),
                resolved.reason,
            )
        } else {
            let (fallback_request, fallback_reason) = self.legacy_dispatch(&event, now_ms);
            (fallback_request, fallback_reason)
        };

        let had_request = request.is_some();
        let selected = request.filter(|request| self.scheduler.accepts(request));
        if had_request && selected.is_none() {
            fallback = Some("higher-priority action retained".into());
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

    /// Feed the latest emotion-driven brain state into the behavior context.
    ///
    /// Called by the runtime after advancing stats and emotion, so the planner
    /// decides from fresh numbers rather than stale defaults. Personality is
    /// synced here too; the controller-owned context fields are refreshed per
    /// decision inside [`BehaviorController::handle`].
    pub fn observe(&mut self, stats: &PetStats, emotion: &EmotionState, personality: Personality) {
        self.brain.energy = stats.energy;
        self.brain.affinity = stats.affinity;
        self.brain.curiosity = stats.curiosity;
        self.brain.emotion = *emotion;
        self.brain.personality = personality;
    }

    /// Whether a proactive invite may fire right now, mirroring the legacy
    /// PointerNear guard so the planner stays consistent with the fallback.
    fn proactive_invite_allowed(&self, now_ms: u64) -> bool {
        if self.interaction_level == "quiet" {
            return false;
        }
        if !(self.proactive_enabled || self.allow_pet_approach || self.allow_mouse_follow) {
            return false;
        }
        self.last_invite_ms
            .is_none_or(|last| now_ms.saturating_sub(last) >= self.interaction_cooldown_ms)
    }

    /// Apply the state-machine side effects of acting on a planned intent.
    ///
    /// Where an intent overlaps a legacy event (a proximity invite, a click) the
    /// effects mirror the verified arm so the new path stays behaviorally
    /// consistent with the fallback.
    fn apply_intent(&mut self, intent: BehaviorIntent, now_ms: u64) {
        match intent {
            BehaviorIntent::NoticeUser
            | BehaviorIntent::ApproachUser
            | BehaviorIntent::SeekAttention => {
                self.state = PetState::WaitingForResponse;
                self.mood.set(Mood::Curious, now_ms, 4000);
                if self.session.is_none() {
                    self.session = Some(InteractionSession::new(now_ms));
                }
                self.last_invite_ms = Some(now_ms);
            }
            BehaviorIntent::Play => {
                if let Some(session) = self.session.as_mut() {
                    session.click();
                }
                self.state = PetState::Playing;
                self.mood.set(Mood::Playful, now_ms, 1800);
            }
            BehaviorIntent::Avoid => {
                if let Some(session) = self.session.as_mut() {
                    session.click();
                }
                self.state = PetState::Playing;
                self.mood.set(Mood::Startled, now_ms, 1800);
            }
            BehaviorIntent::Rest => {
                self.state = PetState::Idle;
                self.mood.set(Mood::Calm, now_ms, 4000);
            }
            BehaviorIntent::Sleep => {
                self.state = PetState::Idle;
                self.mood.set(Mood::Sleepy, now_ms, 8000);
            }
            BehaviorIntent::Explore => {
                self.state = PetState::Observing;
                self.mood.set(Mood::Curious, now_ms, 3000);
            }
        }
    }

    /// The original event -> action director logic, kept as a fallback for any
    /// event the behavior planner declines to interpret. Preserved verbatim so
    /// the verified physical and system transitions stay intact.
    fn legacy_dispatch(
        &mut self,
        event: &PetEvent,
        now_ms: u64,
    ) -> (Option<ScheduledAction>, &'static str) {
        let mut reason = "event observed";
        let request = match event {
            PetEvent::AppStarted => {
                self.state = PetState::Idle;
                reason = "runtime started";
                Some(self.action("idle", Priority::IdleRandom, now_ms))
            }
            PetEvent::PointerNear { .. }
                if (self.proactive_enabled
                    || self.allow_pet_approach
                    || self.allow_mouse_follow)
                    && self.interaction_level != "quiet"
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
                Some(self.action("idle", Priority::ProactiveInvite, now_ms))
            }
            PetEvent::DragStarted { .. } => {
                self.state = PetState::Dragging;
                reason = "drag started";
                Some(self.action("drag", Priority::UserDirectInput, now_ms))
            }
            PetEvent::DragReleased { path, .. } => {
                reason = "classified drag release";
                match path {
                    ReleasePath::EdgeLeft => {
                        self.state = PetState::OnEdge;
                        self.memory.last_drag_direction = Some(-1);
                        Some(self.action("edge.left", Priority::SystemSafety, now_ms))
                    }
                    ReleasePath::EdgeRight => {
                        self.state = PetState::OnEdge;
                        self.memory.last_drag_direction = Some(1);
                        Some(self.action("edge.right", Priority::SystemSafety, now_ms))
                    }
                    ReleasePath::Drop | ReleasePath::Thrown => {
                        self.state = PetState::Falling;
                        if *path == ReleasePath::Thrown {
                            self.mood.set(Mood::Startled, now_ms, 3000);
                        }
                        Some(self.action("fall", Priority::SystemSafety, now_ms))
                    }
                }
            }
            PetEvent::Landing { path } => {
                self.state = PetState::Landing;
                reason = "release motion landed";
                Some(self.action(
                    if *path == ReleasePath::Thrown {
                        "startled"
                    } else {
                        "landing"
                    },
                    Priority::SystemSafety,
                    now_ms,
                ))
            }
            PetEvent::ActionCompleted { action_id } => {
                self.state = if action_id == "startled" {
                    PetState::Recovering
                } else if matches!(
                    self.state,
                    PetState::Playing
                        | PetState::Observing
                        | PetState::Landing
                        | PetState::OnEdge
                        | PetState::WaitingForResponse
                ) {
                    self.session = None;
                    PetState::Idle
                } else {
                    self.state
                };
                reason = "action lifecycle completed";
                None
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
                        Some(self.action("idle", Priority::ProactiveInvite, now_ms))
                    } else {
                        None
                    }
                } else if self.state == PetState::Recovering {
                    self.state = PetState::Idle;
                    reason = "startled recovery completed";
                    Some(self.action("idle", Priority::SystemSafety, now_ms))
                } else {
                    None
                }
            }
            _ => None,
        };
        (request, reason)
    }

    fn action(&self, id: &str, priority: Priority, now_ms: u64) -> ScheduledAction {
        ScheduledAction {
            id: id.into(),
            priority,
            not_before_ms: now_ms,
        }
    }
    pub fn complete_current(&mut self) -> Option<ScheduledAction> {
        self.scheduler.clear().map(|active| ScheduledAction {
            id: active.id,
            priority: active.priority,
            not_before_ms: active.started_at_ms,
        })
    }

    pub fn action_started(&mut self, action: &ScheduledAction, now_ms: u64) {
        self.scheduler.activate(action, now_ms);
        self.memory.record_action(&action.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(path: ReleasePath) -> PetEvent {
        PetEvent::DragReleased {
            velocity_x: 0.0,
            velocity_y: 0.0,
            x: 0,
            y: 0,
            path,
        }
    }

    #[test]
    fn test_drop_enters_landing_then_idle() {
        let mut behavior = BehaviorController::default();
        behavior.handle(release(ReleasePath::Drop), 0);
        assert_eq!(behavior.state, PetState::Falling);
        behavior.handle(
            PetEvent::Landing {
                path: ReleasePath::Drop,
            },
            1,
        );
        assert_eq!(behavior.state, PetState::Landing);
        behavior.handle(
            PetEvent::ActionCompleted {
                action_id: "landing".into(),
            },
            2,
        );
        assert_eq!(behavior.state, PetState::Idle);
    }

    #[test]
    fn test_throw_enters_startled_recovery() {
        let mut behavior = BehaviorController::default();
        behavior.handle(release(ReleasePath::Thrown), 0);
        assert_eq!(behavior.state, PetState::Falling);
        assert_eq!(behavior.mood.mood, Mood::Startled);
        behavior.handle(
            PetEvent::Landing {
                path: ReleasePath::Thrown,
            },
            1,
        );
        assert_eq!(behavior.state, PetState::Landing);
        behavior.handle(
            PetEvent::ActionCompleted {
                action_id: "startled".into(),
            },
            2,
        );
        assert_eq!(behavior.state, PetState::Recovering);
        behavior.handle(PetEvent::Tick { now_ms: 3 }, 3);
        assert_eq!(behavior.state, PetState::Idle);
    }

    #[test]
    fn test_landing_completes_falling_state() {
        let mut behavior = BehaviorController::default();
        behavior.handle(release(ReleasePath::Drop), 0);
        behavior.handle(
            PetEvent::Landing {
                path: ReleasePath::Drop,
            },
            1,
        );
        assert_ne!(behavior.state, PetState::Falling);
    }

    #[test]
    fn test_planner_supersedes_legacy_for_social_events() {
        // A default body click resolves through the planner as Play -> clicked,
        // never reaching the legacy arm.
        let mut behavior = BehaviorController::default();
        let action = behavior.handle(
            PetEvent::PetClicked {
                region: HitRegion::Body,
                click_count: 1,
            },
            0,
        );
        assert_eq!(behavior.state, PetState::Playing);
        assert_eq!(behavior.mood.mood, Mood::Playful);
        assert_eq!(action.as_ref().map(|a| a.id.as_str()), Some("clicked"));
    }

    #[test]
    fn test_falls_back_to_legacy_for_physical_events() {
        // The planner declines drag releases, so the verified falling/edge
        // transitions still come from the legacy mapping.
        let mut behavior = BehaviorController::default();
        let action = behavior.handle(
            PetEvent::DragReleased {
                velocity_x: 0.0,
                velocity_y: 0.0,
                x: 0,
                y: 0,
                path: ReleasePath::EdgeLeft,
            },
            0,
        );
        assert_eq!(behavior.state, PetState::OnEdge);
        assert_eq!(action.as_ref().map(|a| a.id.as_str()), Some("edge.left"));
    }

    #[test]
    fn test_exhausted_idle_tick_sleeps_via_planner() {
        let mut behavior = BehaviorController::default();
        behavior.brain.energy = 0.1;
        let action = behavior.handle(PetEvent::Tick { now_ms: 0 }, 0);
        assert_eq!(behavior.mood.mood, Mood::Sleepy);
        assert_eq!(action.as_ref().map(|a| a.id.as_str()), Some("relax"));
    }
}
