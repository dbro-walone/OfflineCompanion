use crate::{
    animation::{
        definition::{AnimationDefinition, ResumePolicy},
        player::{AnimationPlayer, PlayerStatus},
        render_state::RenderState,
    },
    behavior::{
        director::BehaviorController,
        emotion::{EmotionContext, EmotionEngine, EmotionState, Personality},
        event::{EventNormalizer, PetEvent},
        scheduler::{IdleScheduler, Priority, ScheduledAction, SeededRng},
        state::PetState,
        state_model::PetStats,
    },
    model::AppSettings,
    package_runtime::catalog::PackageCatalog,
};
use anyhow::Result;
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusinessFact {
    ReminderDue {
        kind: crate::behavior::event::ReminderKind,
        id: String,
    },
    ReminderHandled {
        kind: crate::behavior::event::ReminderKind,
        id: String,
    },
    PomodoroStarted,
    PomodoroPaused,
    PomodoroCompleted,
    TodoCompleted {
        id: String,
    },
    SedentaryWarning,
    UserActivityResumed,
}

impl BusinessFact {
    pub fn into_event(self) -> PetEvent {
        match self {
            Self::ReminderDue { kind, id } => PetEvent::ReminderRaised { kind, id },
            Self::ReminderHandled { kind, id } => PetEvent::ReminderCompleted { kind, id },
            Self::PomodoroStarted => PetEvent::PomodoroStarted,
            Self::PomodoroPaused => PetEvent::PomodoroPaused,
            Self::PomodoroCompleted => PetEvent::PomodoroCompleted,
            Self::TodoCompleted { id } => PetEvent::TodoCompleted { id },
            Self::SedentaryWarning => PetEvent::SedentaryWarning,
            Self::UserActivityResumed => PetEvent::UserActivityResumed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeUpdate {
    pub action_id: String,
    pub render: RenderState,
    pub completed: bool,
    pub completed_action_id: Option<String>,
}

#[derive(Clone)]
struct SavedAction {
    player: AnimationPlayer,
    render: RenderState,
    id: String,
    priority: Priority,
}

pub struct AppRuntime {
    catalog: PackageCatalog,
    character_id: String,
    enabled_packs: Vec<String>,
    idle_actions_enabled: bool,
    normalizer: EventNormalizer,
    pub behavior: BehaviorController,
    pub stats: PetStats,
    pub personality: Personality,
    pub emotion: EmotionEngine,
    idle_scheduler: IdleScheduler<SeededRng>,
    player: Option<AnimationPlayer>,
    render: Option<RenderState>,
    previous: Option<SavedAction>,
    current_action: String,
}

impl AppRuntime {
    pub fn new(characters: &Path, actions: &Path, settings: &AppSettings) -> Self {
        let mut behavior = BehaviorController::default();
        apply_behavior_settings(&mut behavior, settings);
        Self {
            catalog: PackageCatalog::scan(characters, actions),
            character_id: settings.current_character_id.clone(),
            enabled_packs: settings.enabled_action_pack_ids.clone(),
            idle_actions_enabled: settings.idle_actions_enabled,
            normalizer: EventNormalizer::default(),
            behavior,
            stats: PetStats::new(),
            personality: Personality::BALANCED,
            emotion: EmotionEngine::default(),
            idle_scheduler: IdleScheduler::default(),
            player: None,
            render: None,
            previous: None,
            current_action: String::new(),
        }
    }

    pub fn apply_settings(&mut self, settings: &AppSettings, now_ms: u64) {
        apply_behavior_settings(&mut self.behavior, settings);
        if self.idle_actions_enabled != settings.idle_actions_enabled {
            self.idle_scheduler.reset(now_ms);
        }
        self.idle_actions_enabled = settings.idle_actions_enabled;
        self.character_id = settings.current_character_id.clone();
        self.enabled_packs = settings.enabled_action_pack_ids.clone();
    }

    pub fn reload(&mut self, characters: &Path, actions: &Path) {
        self.catalog = PackageCatalog::scan(characters, actions);
        self.enabled_packs
            .retain(|id| self.catalog.actions.contains_key(id));
        if self.catalog.resolve_character(&self.character_id).is_none() {
            self.character_id = self
                .catalog
                .characters
                .keys()
                .next()
                .cloned()
                .unwrap_or_default();
        }
        self.stop_current();
    }

    pub fn set_character(&mut self, id: &str) -> bool {
        if !self.catalog.characters.contains_key(id) {
            return false;
        }
        self.character_id = id.into();
        self.stop_current();
        true
    }

    pub fn set_action_pack_enabled(&mut self, id: &str, enabled: bool) -> bool {
        if !self.catalog.actions.contains_key(id) {
            return false;
        }
        if enabled {
            if !self.enabled_packs.iter().any(|value| value == id) {
                self.enabled_packs.push(id.into());
            }
        } else {
            self.enabled_packs.retain(|value| value != id);
            if self.catalog.actions.get(id).is_some_and(|package| {
                package
                    .manifest
                    .actions
                    .iter()
                    .any(|action| action.id == self.current_action)
            }) {
                self.stop_current();
            }
        }
        true
    }

    pub fn current_character_id(&self) -> &str {
        &self.character_id
    }

    pub fn enabled_action_pack_ids(&self) -> &[String] {
        &self.enabled_packs
    }

    pub fn activate_default(&mut self, now_ms: u64) -> Result<Option<RuntimeUpdate>> {
        self.stop_current();
        self.dispatch(PetEvent::AppStarted, now_ms)
    }

    pub fn preview_action_pack(
        &mut self,
        package_id: &str,
        now_ms: u64,
    ) -> Result<Option<RuntimeUpdate>> {
        let Some(id) = self
            .catalog
            .actions
            .get(package_id)
            .and_then(|package| package.manifest.actions.first())
            .map(|action| action.id.clone())
        else {
            return Ok(None);
        };
        self.start_scheduled(
            ScheduledAction {
                id,
                priority: Priority::UserDirectInput,
                not_before_ms: now_ms,
            },
            now_ms,
        )
    }

    pub fn dispatch(&mut self, event: PetEvent, now_ms: u64) -> Result<Option<RuntimeUpdate>> {
        let Some(event) = self.normalizer.normalize(event) else {
            return Ok(None);
        };
        // Advance the brain (state, then emotion) before behavior decides what
        // to do. Events only describe what happened; the brain turns them into
        // numeric state and emotion that the behavior layer may read.
        self.stats.apply(&event, now_ms);
        self.emotion.apply(
            &event,
            now_ms,
            EmotionContext {
                personality: &self.personality,
                stats: &self.stats,
                memory: &self.behavior.memory,
            },
        );
        // Feed the freshly advanced stats/emotion into the behavior context so
        // the planner decides from current numbers, then run the decision layer.
        self.behavior
            .observe(&self.stats, &self.emotion.state, self.personality);
        let Some(request) = self.behavior.handle(event, now_ms) else {
            return Ok(None);
        };
        self.start_scheduled(request, now_ms)
    }

    pub fn dispatch_fact(
        &mut self,
        fact: BusinessFact,
        now_ms: u64,
    ) -> Result<Option<RuntimeUpdate>> {
        self.dispatch(fact.into_event(), now_ms)
    }

    /// Read-only view of the pet's numeric state, for inspection or UI.
    pub fn stats(&self) -> &PetStats {
        &self.stats
    }

    /// The current emotion space resulting from the event stream.
    pub fn emotion(&self) -> &EmotionState {
        &self.emotion.state
    }

    /// The personality profile currently driving emotional responses.
    pub fn personality(&self) -> &Personality {
        &self.personality
    }

    /// Replace the personality profile, e.g. when a different character loads.
    pub fn set_personality(&mut self, personality: Personality) {
        self.personality = personality;
    }

    pub fn tick(&mut self, now_ms: u64) -> Result<Option<RuntimeUpdate>> {
        if let Some(player) = self.player.as_mut()
            && let Some(frame) = player.tick(now_ms)
        {
            if let Some(render) = self.render.as_mut() {
                render.frame_index = frame;
            }
            if player.status() == PlayerStatus::Completed {
                return self.complete_action(now_ms);
            }
            if player.safe_to_interrupt()
                && let Some(pending) = self.behavior.scheduler.take_pending()
            {
                return self.start_scheduled(pending, now_ms);
            }
            return Ok(self.current_update(false, None));
        }

        if self
            .player
            .as_ref()
            .is_some_and(AnimationPlayer::safe_to_interrupt)
            && let Some(pending) = self.behavior.scheduler.take_pending()
        {
            return self.start_scheduled(pending, now_ms);
        }

        let blocked = self.behavior.state != PetState::Idle
            || self.behavior.session.is_some()
            || self
                .behavior
                .scheduler
                .current()
                .is_some_and(|action| action.priority > Priority::IdleRandom);
        let actions = self
            .catalog
            .merged_actions(&self.character_id, &self.enabled_packs)
            .into_values()
            .collect::<Vec<_>>();
        let default_action = self
            .catalog
            .resolve_character(&self.character_id)
            .map(|character| character.manifest.default_action.as_str())
            .unwrap_or("idle");
        if let Some(id) = self.idle_scheduler.select_or_default(
            now_ms,
            self.idle_actions_enabled,
            blocked,
            &actions,
            default_action,
        ) {
            return self.start_scheduled(
                ScheduledAction {
                    id,
                    priority: Priority::IdleRandom,
                    not_before_ms: now_ms,
                },
                now_ms,
            );
        }

        if self.player.is_none() {
            return self.dispatch(PetEvent::Tick { now_ms }, now_ms);
        }
        Ok(None)
    }

    fn start_scheduled(
        &mut self,
        request: ScheduledAction,
        now_ms: u64,
    ) -> Result<Option<RuntimeUpdate>> {
        if now_ms < request.not_before_ms || !self.behavior.scheduler.accepts(&request) {
            return Ok(None);
        }
        let requested = alias(&request.id);
        let Some(action) =
            self.catalog
                .resolve_action(&self.character_id, &self.enabled_packs, requested)
        else {
            return Ok(None);
        };
        let definition: AnimationDefinition =
            serde_json::from_str(&fs::read_to_string(&action.animation_path)?)?;

        if let Some(current) = self.player.as_ref() {
            if !current.interruptible() && !current.safe_to_interrupt() {
                self.behavior.scheduler.queue(request);
                return Ok(None);
            }
            if matches!(
                definition.resume_policy,
                ResumePolicy::Previous | ResumePolicy::Resume
            ) && let Some(render) = self.render.as_ref()
                && let Some(active) = self.behavior.scheduler.current()
            {
                self.previous = Some(SavedAction {
                    player: current.clone(),
                    render: render.clone(),
                    id: self.current_action.clone(),
                    priority: active.priority,
                });
            }
        }

        let frame = definition
            .segments
            .entry
            .as_ref()
            .map(|segment| segment.start)
            .or_else(|| {
                definition
                    .segments
                    .main_loop
                    .as_ref()
                    .map(|segment| segment.start)
            })
            .unwrap_or_default();
        let render = RenderState {
            atlas: action.render.atlas_path,
            frame_index: frame,
            frame_width: action.render.frame_width,
            frame_height: action.render.frame_height,
            columns: action.render.columns,
            mirror_x: definition.mirrorable && requested.ends_with("right"),
        };
        let actual = ScheduledAction {
            id: action.id,
            ..request
        };
        self.player = Some(AnimationPlayer::new(definition, now_ms));
        self.render = Some(render.clone());
        self.current_action = actual.id.clone();
        self.behavior.action_started(&actual, now_ms);
        Ok(Some(RuntimeUpdate {
            action_id: actual.id,
            render,
            completed: false,
            completed_action_id: None,
        }))
    }

    fn complete_action(&mut self, now_ms: u64) -> Result<Option<RuntimeUpdate>> {
        let completed_id = self.current_action.clone();
        let emit = self
            .behavior
            .scheduler
            .current_mut()
            .is_some_and(|active| active.mark_completed_once());
        if !emit {
            return Ok(None);
        }
        let policy = self
            .player
            .as_ref()
            .map(|player| player.resume_policy().clone());
        self.behavior.complete_current();
        let _ = self.behavior.handle(
            PetEvent::ActionCompleted {
                action_id: completed_id.clone(),
            },
            now_ms,
        );

        if matches!(policy, Some(ResumePolicy::Previous | ResumePolicy::Resume))
            && let Some(previous) = self.previous.take()
        {
            let request = ScheduledAction {
                id: previous.id.clone(),
                priority: previous.priority,
                not_before_ms: now_ms,
            };
            self.player = Some(previous.player);
            self.render = Some(previous.render);
            self.current_action = previous.id;
            self.behavior.action_started(&request, now_ms);
            return Ok(self.current_update(true, Some(completed_id)));
        }

        self.previous = None;
        let default_id = self
            .catalog
            .resolve_character(&self.character_id)
            .map(|character| character.manifest.default_action.clone())
            .unwrap_or_else(|| "idle".into());
        self.player = None;
        self.render = None;
        self.current_action.clear();
        let mut update = self.start_scheduled(
            ScheduledAction {
                id: default_id,
                priority: Priority::IdleRandom,
                not_before_ms: now_ms,
            },
            now_ms,
        )?;
        if let Some(update) = update.as_mut() {
            update.completed = true;
            update.completed_action_id = Some(completed_id);
        }
        Ok(update)
    }

    fn current_update(
        &self,
        completed: bool,
        completed_action_id: Option<String>,
    ) -> Option<RuntimeUpdate> {
        Some(RuntimeUpdate {
            action_id: self.current_action.clone(),
            render: self.render.clone()?,
            completed,
            completed_action_id,
        })
    }

    fn stop_current(&mut self) {
        self.player = None;
        self.render = None;
        self.previous = None;
        self.current_action.clear();
        self.behavior.complete_current();
    }

    pub fn catalog(&self) -> &PackageCatalog {
        &self.catalog
    }
}

fn apply_behavior_settings(behavior: &mut BehaviorController, settings: &AppSettings) {
    behavior.proactive_enabled = settings.allow_proactive_invitation;
    behavior.reduce_motion = settings.reduce_motion;
    behavior.interaction_cooldown_ms = settings.interaction_cooldown_seconds * 1000;
    behavior.allow_pet_approach = settings.allow_pet_approach;
    behavior.allow_mouse_follow = settings.allow_mouse_follow;
    behavior.interaction_level = settings.pet_interaction_level.clone();
}

fn alias(id: &str) -> &str {
    match id {
        "clicked" => "clicked",
        "drag" => "dragged",
        "reminder" => "reminder.default",
        value => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime() -> AppRuntime {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        AppRuntime::new(
            &root.join("packages/characters"),
            &root.join("packages/actions"),
            &AppSettings::default(),
        )
    }

    #[test]
    fn enabled_action_pack_plays_with_its_own_atlas_geometry() {
        let mut runtime = runtime();
        let update = runtime
            .start_scheduled(
                ScheduledAction {
                    id: "idle.thinking".into(),
                    priority: Priority::IdleRandom,
                    not_before_ms: 0,
                },
                0,
            )
            .unwrap()
            .expect("default action pack must play");
        assert!(update.render.atlas.ends_with("atlases/office.png"));
        assert_eq!(
            (update.render.frame_width, update.render.frame_height),
            (384, 512)
        );
        assert_eq!(update.render.columns, 4);
        assert_eq!(update.action_id, "idle.thinking");
    }

    #[test]
    fn test_user_input_interrupts_idle() {
        let mut runtime = runtime();
        runtime.dispatch(PetEvent::AppStarted, 0).unwrap();
        let update = runtime
            .dispatch(
                PetEvent::PetClicked {
                    region: crate::behavior::event::HitRegion::Body,
                    click_count: 1,
                },
                1,
            )
            .unwrap()
            .unwrap();
        assert_eq!(update.action_id, "clicked");
    }

    #[test]
    fn head_and_body_clicks_play_distinct_actions() {
        let mut head = runtime();
        let head_update = head
            .dispatch(
                PetEvent::PetClicked {
                    region: crate::behavior::event::HitRegion::Head,
                    click_count: 1,
                },
                0,
            )
            .unwrap()
            .unwrap();
        let mut body = runtime();
        let body_update = body
            .dispatch(
                PetEvent::PetClicked {
                    region: crate::behavior::event::HitRegion::Body,
                    click_count: 1,
                },
                0,
            )
            .unwrap()
            .unwrap();
        assert_eq!(head_update.action_id, "head-pat");
        assert_eq!(body_update.action_id, "clicked");
    }

    #[test]
    fn test_runtime_update_reports_current_action() {
        let mut runtime = runtime();
        runtime.dispatch(PetEvent::AppStarted, 0).unwrap();
        runtime.behavior.memory.last_action_id = Some("stale".into());
        let update = runtime.tick(500).unwrap().unwrap();
        assert_eq!(update.action_id, "idle");
    }

    #[test]
    fn test_action_completed_is_emitted_once() {
        let mut runtime = runtime();
        runtime
            .dispatch(
                PetEvent::PetClicked {
                    region: crate::behavior::event::HitRegion::Body,
                    click_count: 1,
                },
                0,
            )
            .unwrap();
        let update = runtime.tick(10_000).unwrap().unwrap();
        assert_eq!(update.completed_action_id.as_deref(), Some("clicked"));
        assert!(update.completed);
        let next = runtime.tick(10_001).unwrap();
        assert!(
            next.as_ref()
                .is_none_or(|item| item.completed_action_id.is_none())
        );
    }

    #[test]
    fn test_previous_action_is_restored_after_once_action() {
        let mut runtime = runtime();
        runtime.dispatch(PetEvent::AppStarted, 0).unwrap();
        runtime
            .dispatch(
                PetEvent::PetClicked {
                    region: crate::behavior::event::HitRegion::Body,
                    click_count: 1,
                },
                1,
            )
            .unwrap();
        let update = runtime.tick(10_000).unwrap().unwrap();
        assert_eq!(update.completed_action_id.as_deref(), Some("clicked"));
        assert_eq!(update.action_id, "idle");
        assert!(update.render.atlas.ends_with("atlases/base.png"));
    }

    #[test]
    fn idle_tick_schedules_the_enabled_action_pack() {
        let mut runtime = runtime();
        runtime.dispatch(PetEvent::AppStarted, 0).unwrap();
        runtime.tick(10_000).unwrap();
        let update = runtime.tick(10_001).unwrap().unwrap();
        assert_eq!(update.action_id, "idle.thinking");
        assert!(update.render.atlas.ends_with("atlases/office.png"));
    }

    #[test]
    fn settings_reload_disables_idle_actions_immediately() {
        let mut runtime = runtime();
        runtime.dispatch(PetEvent::AppStarted, 0).unwrap();
        let settings = AppSettings {
            idle_actions_enabled: false,
            ..AppSettings::default()
        };
        runtime.apply_settings(&settings, 0);
        runtime.tick(10_000).unwrap();
        let update = runtime.tick(10_001).unwrap();
        assert!(update.is_none());
        assert_eq!(runtime.current_action, "idle");
    }

    #[test]
    fn action_pack_toggle_applies_without_restart_and_falls_back_safely() {
        let mut runtime = runtime();
        assert!(runtime.set_action_pack_enabled("action.shadow-crow.office", true));
        let preview = runtime
            .preview_action_pack("action.shadow-crow.office", 0)
            .unwrap()
            .unwrap();
        assert_eq!(preview.action_id, "idle.thinking");
        assert!(runtime.set_action_pack_enabled("action.shadow-crow.office", false));
        let fallback = runtime.activate_default(1).unwrap().unwrap();
        assert_eq!(fallback.action_id, "idle");
    }
}
