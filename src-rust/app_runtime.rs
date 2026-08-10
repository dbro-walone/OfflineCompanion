use crate::{
    animation::{
        definition::AnimationDefinition,
        player::{AnimationPlayer, PlayerStatus},
        render_state::RenderState,
    },
    behavior::{
        director::BehaviorController,
        event::{EventNormalizer, PetEvent},
    },
    model::AppSettings,
    package_runtime::{
        catalog::PackageCatalog, manifest::PackageManifest, validator::load_manifest,
    },
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
}
pub struct AppRuntime {
    catalog: PackageCatalog,
    character_id: String,
    enabled_packs: Vec<String>,
    normalizer: EventNormalizer,
    pub behavior: BehaviorController,
    player: Option<AnimationPlayer>,
    render: Option<RenderState>,
    previous: Option<(AnimationPlayer, RenderState, String)>,
    current_action: String,
}
impl AppRuntime {
    pub fn new(characters: &Path, actions: &Path, settings: &AppSettings) -> Self {
        let mut behavior = BehaviorController::default();
        behavior.proactive_enabled = settings.allow_proactive_invitation;
        behavior.reduce_motion = settings.reduce_motion;
        behavior.interaction_cooldown_ms = settings.interaction_cooldown_seconds * 1000;
        Self {
            catalog: PackageCatalog::scan(characters, actions),
            character_id: settings.current_character_id.clone(),
            enabled_packs: settings.enabled_action_pack_ids.clone(),
            normalizer: EventNormalizer::default(),
            behavior,
            player: None,
            render: None,
            previous: None,
            current_action: String::new(),
        }
    }
    pub fn reload(&mut self, characters: &Path, actions: &Path) {
        self.catalog = PackageCatalog::scan(characters, actions);
        if self.catalog.resolve_character(&self.character_id).is_none() {
            self.character_id = self
                .catalog
                .characters
                .keys()
                .next()
                .cloned()
                .unwrap_or_default()
        }
        self.player = None;
        self.render = None;
        self.previous = None;
        self.current_action.clear();
    }
    pub fn dispatch(&mut self, event: PetEvent, now_ms: u64) -> Result<Option<RuntimeUpdate>> {
        let Some(event) = self.normalizer.normalize(event) else {
            return Ok(None);
        };
        let Some(request) = self.behavior.handle(event, now_ms) else {
            return Ok(None);
        };
        let update = self.play_action(&request.id, now_ms)?;
        self.behavior.complete_current();
        Ok(update)
    }
    pub fn dispatch_fact(
        &mut self,
        fact: BusinessFact,
        now_ms: u64,
    ) -> Result<Option<RuntimeUpdate>> {
        self.dispatch(fact.into_event(), now_ms)
    }
    pub fn tick(&mut self, now_ms: u64) -> Result<Option<RuntimeUpdate>> {
        if let Some(player) = self.player.as_mut()
            && let Some(frame) = player.tick(now_ms)
        {
            let mut render = self.render.clone().expect("player has render state");
            render.frame_index = frame;
            let completed = player.status() == PlayerStatus::Completed;
            let update = RuntimeUpdate {
                action_id: self
                    .behavior
                    .memory
                    .last_action_id
                    .clone()
                    .unwrap_or_default(),
                render,
                completed,
            };
            if completed && let Some((previous, render, action)) = self.previous.take() {
                self.player = Some(previous);
                self.render = Some(render);
                self.current_action = action;
            }
            return Ok(Some(update));
        }
        self.dispatch(PetEvent::Tick { now_ms }, now_ms)
    }
    fn play_action(&mut self, id: &str, now_ms: u64) -> Result<Option<RuntimeUpdate>> {
        let id = alias(id);
        let Some(path) = self
            .catalog
            .resolve_action(&self.character_id, &self.enabled_packs, id)
        else {
            return Ok(None);
        };
        let definition: AnimationDefinition = serde_json::from_str(&fs::read_to_string(&path)?)?;
        if let (Some(current), Some(render)) = (self.player.as_ref(), self.render.as_ref()) {
            use crate::animation::definition::ResumePolicy;
            if !current.interruptible() && id != "relax" {
                return Ok(None);
            }
            if matches!(
                current.resume_policy(),
                ResumePolicy::Resume | ResumePolicy::Previous
            ) {
                self.previous =
                    Some((current.clone(), render.clone(), self.current_action.clone()));
            }
        }
        let frame = definition
            .segments
            .entry
            .as_ref()
            .map(|x| x.start)
            .or_else(|| definition.segments.main_loop.as_ref().map(|x| x.start))
            .unwrap_or_default();
        let PackageManifest::Character(character) = load_manifest(
            &self
                .catalog
                .resolve_character(&self.character_id)
                .expect("resolved action has character")
                .root
                .join("manifest.json"),
        )?
        else {
            unreachable!()
        };
        let root = path.parent().unwrap();
        let atlas = root.join(&definition.atlas);
        let columns = character
            .render
            .as_ref()
            .and_then(|x| x.columns)
            .unwrap_or(4);
        let (fw, fh) = character
            .render
            .as_ref()
            .map(|x| (x.frame_width, x.frame_height))
            .unwrap_or((character.frame.width, character.frame.height));
        let render = RenderState {
            atlas,
            frame_index: frame,
            frame_width: fw,
            frame_height: fh,
            columns,
            mirror_x: definition.mirrorable && id.ends_with("right"),
        };
        self.player = Some(AnimationPlayer::new(definition, now_ms));
        self.render = Some(render.clone());
        self.current_action = id.into();
        Ok(Some(RuntimeUpdate {
            action_id: id.into(),
            render,
            completed: false,
        }))
    }
    pub fn catalog(&self) -> &PackageCatalog {
        &self.catalog
    }
}
fn alias(id: &str) -> &str {
    match id {
        "head-pat" | "clicked" => "clicked",
        "drag" => "dragged",
        "fall" => "relax",
        "reminder" => "reminder.default",
        "look" => "idle",
        x => x,
    }
}
