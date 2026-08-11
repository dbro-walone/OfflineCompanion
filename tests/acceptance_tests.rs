//! Acceptance baseline for the Desktop Companion MVP.
//!
//! These integration tests prove the pet behaves like a stateful, personality-
//! driven companion, not just a sprite that plays animations. Each test drives
//! a [`PetEvent`] through the full decision pipeline
//! (`Event -> State -> Emotion -> Behavior -> Action`) and asserts on the
//! resulting action, state-machine state, mood and emotion.
//!
//! They are pure Rust: no window, atlas or package catalog is required. The
//! [`Pet`] harness below mirrors [`AppRuntime::dispatch`] — it advances the
//! numeric state engine, folds the event into the emotion engine, feeds the
//! fresh numbers into the behavior context, then asks the behavior controller
//! to decide. That is the exact sequence the real runtime runs every event.
//!
//! The scenarios map onto issues #63-#66:
//!   * A, B - interaction demo: clicks and the personality fork.
//!   * C     - physical interaction: drag, drop, throw and recovery.
//!   * D     - social proximity: approach vs. rest.
//!   * E     - life simulation: autonomous attention-seeking and exploration.
//!   * F     - life simulation: the sleep / wake energy loop.
//!   * G     - proactive companion: celebrating a finished focus session.
//!   * H     - architecture: the same input branches purely by personality.
//!   * I     - architecture: data-driven per-character behavior overrides.

use offline_companion::behavior::{
    director::BehaviorController,
    emotion::{EmotionContext, EmotionEngine, EmotionKind, Personality},
    event::{HitRegion, PetEvent},
    locomotion::ReleasePath,
    scheduler::ScheduledAction,
    state::{Mood, PetState},
    state_model::PetStats,
};

// --- Test harness -----------------------------------------------------------

/// A minimal, GUI-free replica of the runtime's decision pipeline.
///
/// [`AppRuntime::dispatch`] advances the brain in a fixed order - numeric
/// state, then emotion, then the behavior context - before the behavior layer
/// decides what to do. This harness reproduces that exact sequence on
/// standalone components, so acceptance tests exercise the real decision chain
/// with no window, atlas or package catalog.
struct Pet {
    stats: PetStats,
    emotion: EmotionEngine,
    behavior: BehaviorController,
    personality: Personality,
}

impl Pet {
    fn new(personality: Personality) -> Self {
        Self {
            stats: PetStats::new(),
            emotion: EmotionEngine::default(),
            behavior: BehaviorController::default(),
            personality,
        }
    }

    fn balanced() -> Self {
        Self::new(Personality::BALANCED)
    }

    /// Feed one event through the full brain pipeline and return any action the
    /// behavior layer schedules. Identical to `AppRuntime::dispatch` minus the
    /// animation and catalog layer.
    fn interact(&mut self, event: PetEvent, now_ms: u64) -> Option<ScheduledAction> {
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
        self.behavior
            .observe(&self.stats, &self.emotion.state, self.personality);
        self.behavior.handle(event, now_ms)
    }
}

fn click(region: HitRegion, click_count: u8) -> PetEvent {
    PetEvent::PetClicked {
        region,
        click_count,
    }
}

fn released(path: ReleasePath) -> PetEvent {
    PetEvent::DragReleased {
        velocity_x: 0.0,
        velocity_y: 0.0,
        x: 0,
        y: 0,
        path,
    }
}

fn action_id(action: &Option<ScheduledAction>) -> Option<&str> {
    action.as_ref().map(|a| a.id.as_str())
}

// --- A. Click interaction ---------------------------------------------------

#[test]
fn single_head_pat_plays_and_bonds() {
    let mut pet = Pet::balanced();
    let action = pet.interact(click(HitRegion::Head, 1), 0);

    // Head -> Play intent -> head-pat action, playful mood, Playing state.
    assert_eq!(action_id(&action), Some("head-pat"));
    assert_eq!(pet.behavior.state, PetState::Playing);
    assert_eq!(pet.behavior.mood.mood, Mood::Playful);
    // A head pat is the warmest interaction; it bonds the pet and reads as joy.
    assert!(pet.stats.affinity > 0.5);
    assert_eq!(pet.emotion.state.dominant(), EmotionKind::Happy);
}

#[test]
fn single_body_click_plays_clicked_action() {
    let mut pet = Pet::new(Personality::playful());
    let action = pet.interact(click(HitRegion::Body, 1), 0);

    assert_eq!(action_id(&action), Some("clicked"));
    assert_eq!(pet.behavior.state, PetState::Playing);
    assert_eq!(pet.behavior.mood.mood, Mood::Playful);
}

// --- B. Repeated clicks diverge by personality ------------------------------

#[test]
fn repeated_pokes_startle_a_sensitive_pet() {
    let mut pet = Pet::new(Personality::sensitive());
    let mut last = None;
    for poke in 1u8..=3 {
        last = pet.interact(click(HitRegion::Body, poke), u64::from(poke) * 1000);
    }

    // A sensitive pet reads a barrage of pokes as overstimulation -> Avoid.
    assert_eq!(action_id(&last), Some("startled"));
    assert_eq!(pet.behavior.mood.mood, Mood::Startled);
    assert_eq!(pet.emotion.state.dominant(), EmotionKind::Annoyed);
}

#[test]
fn repeated_pokes_keep_a_playful_pet_happy() {
    let mut pet = Pet::new(Personality::playful());
    let mut last = None;
    for poke in 1u8..=3 {
        last = pet.interact(click(HitRegion::Body, poke), u64::from(poke) * 1000);
    }

    // The same input amuses a playful pet -> Play, never Avoid.
    assert_eq!(action_id(&last), Some("clicked"));
    assert_eq!(pet.behavior.mood.mood, Mood::Playful);
    assert_eq!(pet.emotion.state.dominant(), EmotionKind::Happy);
}

// --- C. Drag, drop and throw ------------------------------------------------

#[test]
fn drag_starts_and_releases_into_falling() {
    let mut pet = Pet::balanced();
    let drag = pet.interact(
        PetEvent::DragStarted {
            pointer_x: 0.0,
            pointer_y: 0.0,
        },
        0,
    );

    assert_eq!(action_id(&drag), Some("drag"));
    assert_eq!(pet.behavior.state, PetState::Dragging);

    let fall = pet.interact(released(ReleasePath::Drop), 1);
    assert_eq!(action_id(&fall), Some("fall"));
    assert_eq!(pet.behavior.state, PetState::Falling);
}

#[test]
fn gentle_drop_lands_and_returns_to_idle() {
    let mut pet = Pet::balanced();
    pet.interact(released(ReleasePath::Drop), 0);
    assert_eq!(pet.behavior.state, PetState::Falling);

    let land = pet.interact(
        PetEvent::Landing {
            path: ReleasePath::Drop,
        },
        1,
    );
    assert_eq!(action_id(&land), Some("landing"));
    assert_eq!(pet.behavior.state, PetState::Landing);

    let after = pet.interact(
        PetEvent::ActionCompleted {
            action_id: "landing".into(),
        },
        2,
    );
    assert!(after.is_none());
    assert_eq!(pet.behavior.state, PetState::Idle);
}

#[test]
fn thrown_release_startles_then_recovers() {
    let mut pet = Pet::new(Personality::sensitive());

    let fall = pet.interact(released(ReleasePath::Thrown), 0);
    assert_eq!(action_id(&fall), Some("fall"));
    assert_eq!(pet.behavior.state, PetState::Falling);
    // A throw is alarming: the mood startles and annoyance becomes salient.
    assert_eq!(pet.behavior.mood.mood, Mood::Startled);
    assert!(pet.emotion.state.annoyed > 0.2);

    let land = pet.interact(
        PetEvent::Landing {
            path: ReleasePath::Thrown,
        },
        1,
    );
    assert_eq!(action_id(&land), Some("startled"));

    pet.interact(
        PetEvent::ActionCompleted {
            action_id: "startled".into(),
        },
        2,
    );
    assert_eq!(pet.behavior.state, PetState::Recovering);

    // A subsequent tick walks the pet out of recovery and back to idle.
    let recovered = pet.interact(PetEvent::Tick { now_ms: 3 }, 3);
    assert_eq!(pet.behavior.state, PetState::Idle);
    assert_eq!(action_id(&recovered), Some("idle"));
}

// --- D. Pointer proximity ---------------------------------------------------

#[test]
fn nearby_pointer_with_high_attachment_is_approached() {
    // playful has attachment 0.6, past the approach threshold.
    let mut pet = Pet::new(Personality::playful());
    let action = pet.interact(PetEvent::PointerNear { distance_px: 10.0 }, 0);

    assert_eq!(action_id(&action), Some("look"));
    assert_eq!(pet.behavior.state, PetState::WaitingForResponse);
    assert_eq!(pet.behavior.mood.mood, Mood::Curious);
}

#[test]
fn nearby_pointer_when_tired_lets_pet_rest() {
    let mut pet = Pet::balanced();
    pet.stats.energy = 0.2;
    let action = pet.interact(PetEvent::PointerNear { distance_px: 10.0 }, 0);

    // Low energy overrides sociability: rest in place instead of approaching.
    assert_eq!(action_id(&action), Some("relax"));
    assert_eq!(pet.behavior.mood.mood, Mood::Calm);
}

// --- E. Idle life simulation ------------------------------------------------

#[test]
fn restless_active_pet_seeks_attention_when_ignored() {
    // Active but not curious, so SeekAttention is reachable before Explore.
    let mut pet = Pet::new(Personality::new(0.8, 0.5, 0.4, 0.5, 0.5));
    // The pet has been left alone for over a minute; keep it rested so energy
    // does not collapse to sleep first (a real runtime restores energy on naps
    // and completions, which is simulated here by topping up after the idle).
    pet.stats.advance(70_000);
    pet.stats.energy = 0.8;

    let action = pet.interact(PetEvent::Tick { now_ms: 70_000 }, 70_000);

    assert_eq!(action_id(&action), Some("look"));
    assert_eq!(pet.behavior.state, PetState::WaitingForResponse);
    assert_eq!(pet.behavior.mood.mood, Mood::Curious);
}

#[test]
fn curious_pet_explores_when_idle() {
    // Curious but calm, so the life loop picks Explore rather than SeekAttention.
    let mut pet = Pet::new(Personality::new(0.4, 0.5, 0.8, 0.5, 0.5));
    let action = pet.interact(PetEvent::Tick { now_ms: 0 }, 0);

    assert_eq!(action_id(&action), Some("look"));
    assert_eq!(pet.behavior.state, PetState::Observing);
    assert_eq!(pet.behavior.mood.mood, Mood::Curious);
}

// --- F. Sleep / wake energy loop --------------------------------------------

#[test]
fn exhausted_pet_sleeps_and_wakes_when_restored() {
    let mut pet = Pet::balanced();
    pet.stats.energy = 0.1;

    let sleeping = pet.interact(PetEvent::Tick { now_ms: 0 }, 0);
    assert_eq!(action_id(&sleeping), Some("relax"));
    assert_eq!(pet.behavior.mood.mood, Mood::Sleepy);

    // Energy restored: the pet stops sleeping and the sleepy mood clears.
    pet.stats.energy = 0.9;
    let awake = pet.interact(PetEvent::Tick { now_ms: 9_000 }, 9_000);
    assert!(awake.is_none());
    assert_ne!(pet.behavior.mood.mood, Mood::Sleepy);
}

// --- G. Proactive companion -------------------------------------------------

#[test]
fn pomodoro_completion_celebrates() {
    let mut pet = Pet::balanced();
    let action = pet.interact(PetEvent::PomodoroCompleted, 0);

    assert_eq!(action_id(&action), Some("celebrate"));
    assert_eq!(pet.behavior.state, PetState::Playing);
    assert_eq!(pet.behavior.mood.mood, Mood::Happy);
    assert_eq!(pet.emotion.state.dominant(), EmotionKind::Happy);
}

// --- H. Architecture: personality-driven decoupling -------------------------

#[test]
fn identical_poke_branches_by_personality() {
    // The same body poke yields opposite behavior purely from personality.
    // The engine never branches on a character id; the fork lives in data.
    let poke = PetEvent::PetClicked {
        region: HitRegion::Body,
        click_count: 1,
    };
    let mut playful = Pet::new(Personality::playful());
    let mut sensitive = Pet::new(Personality::sensitive());

    let playful_action = playful.interact(poke.clone(), 0);
    let sensitive_action = sensitive.interact(poke, 0);

    assert_eq!(action_id(&playful_action), Some("clicked"));
    assert_eq!(action_id(&sensitive_action), Some("startled"));
    assert!(playful.emotion.state.happy > sensitive.emotion.state.happy);
    assert!(sensitive.emotion.state.annoyed > playful.emotion.state.annoyed);
}

// --- I. Architecture: data-driven behavior overrides ------------------------

#[test]
fn behavior_mapping_override_re_expresses_an_intent() {
    let mut pet = Pet::balanced();
    // This character expresses Play with its own action instead of head-pat.
    pet.behavior.behavior_overrides.play = Some("happy-dance".into());
    assert!(!pet.behavior.behavior_overrides.is_empty());

    let action = pet.interact(click(HitRegion::Head, 1), 0);
    assert_eq!(action_id(&action), Some("happy-dance"));
    assert_eq!(pet.behavior.state, PetState::Playing);
}

#[test]
fn empty_behavior_mapping_keeps_engine_defaults() {
    let mut pet = Pet::balanced();
    let action = pet.interact(click(HitRegion::Head, 1), 0);
    // No overrides -> the tree's default head-pat mapping is used verbatim.
    assert!(pet.behavior.behavior_overrides.is_empty());
    assert_eq!(action_id(&action), Some("head-pat"));
}

// --- Observability ----------------------------------------------------------

#[test]
fn decision_trace_records_the_pipeline() {
    let mut pet = Pet::balanced();
    pet.interact(click(HitRegion::Head, 1), 0);

    let trace = pet
        .behavior
        .traces
        .last()
        .expect("a decision trace is recorded for every event");
    assert_eq!(trace.state, PetState::Playing);
    assert_eq!(trace.mood, Mood::Playful);
    assert_eq!(trace.selected_action.as_deref(), Some("head-pat"));
}
