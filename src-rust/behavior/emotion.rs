//! Emotion and personality: how the pet *feels*, derived from events, state,
//! personality and memory.
//!
//! The pipeline is `Event → State → Emotion → Behavior`. This module owns the
//! `Emotion` step: an [`EmotionEngine`] consumes an event plus the current
//! [`PetStats`], a fixed [`Personality`] and the [`PetMemory`], and produces a
//! new [`EmotionState`]. The same event yields different emotions for different
//! personalities — a poke amuses a playful pet but irritates a sensitive one —
//! and low energy drifts the pet toward sleepiness. Behavior systems read the
//! resulting [`EmotionKind`] to pick a response; no character hard-codes `if`.

use std::cmp::Ordering;

use super::event::{HitRegion, PetEvent};
use super::locomotion::ReleasePath;
use super::state::{Mood, PetMemory};
use super::state_model::PetStats;

/// Five fixed personality dimensions, each a normalized weight in `[0.0, 1.0]`.
/// A character pack provides one set; these never change at runtime.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Personality {
    /// How active/restless the pet tends to be.
    pub activity: f32,
    /// How strongly the pet bonds with the user.
    pub attachment: f32,
    /// How eager the pet is to investigate new things.
    pub curiosity: f32,
    /// How readily the pet treats interaction as play.
    pub playfulness: f32,
    /// How strongly the pet reacts to stimulation (positive or negative).
    pub sensitivity: f32,
}

impl Personality {
    /// A neutral baseline used when no character-specific profile is supplied.
    pub const BALANCED: Personality = Personality {
        activity: 0.5,
        attachment: 0.5,
        curiosity: 0.5,
        playfulness: 0.5,
        sensitivity: 0.5,
    };

    /// Build a personality, clamping each dimension into `[0.0, 1.0]`.
    pub fn new(
        activity: f32,
        attachment: f32,
        curiosity: f32,
        playfulness: f32,
        sensitivity: f32,
    ) -> Self {
        Self {
            activity: activity.clamp(0.0, 1.0),
            attachment: attachment.clamp(0.0, 1.0),
            curiosity: curiosity.clamp(0.0, 1.0),
            playfulness: playfulness.clamp(0.0, 1.0),
            sensitivity: sensitivity.clamp(0.0, 1.0),
        }
    }

    /// A playful profile: loves interaction, not easily rattled.
    pub fn playful() -> Self {
        Self::new(0.7, 0.6, 0.7, 0.9, 0.1)
    }

    /// A sensitive profile: reacts strongly, takes pokes personally.
    pub fn sensitive() -> Self {
        Self::new(0.3, 0.6, 0.5, 0.1, 0.9)
    }
}

impl Default for Personality {
    fn default() -> Self {
        Self::BALANCED
    }
}

/// The six emotion dimensions, each an intensity in `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmotionState {
    pub happy: f32,
    pub confused: f32,
    pub shy: f32,
    pub annoyed: f32,
    pub sleepy: f32,
    pub relaxed: f32,
}

impl EmotionState {
    /// Calm baseline: a little happy and drowsy, otherwise settled.
    pub const fn baseline() -> Self {
        Self {
            happy: 0.1,
            confused: 0.0,
            shy: 0.0,
            annoyed: 0.0,
            sleepy: 0.1,
            relaxed: 0.3,
        }
    }

    /// The strongest emotion above a small threshold, or [`EmotionKind::Neutral`]
    /// when nothing is salient enough to drive behavior.
    pub fn dominant(&self) -> EmotionKind {
        const THRESHOLD: f32 = 0.2;
        let ranking = [
            (EmotionKind::Happy, self.happy),
            (EmotionKind::Annoyed, self.annoyed),
            (EmotionKind::Sleepy, self.sleepy),
            (EmotionKind::Confused, self.confused),
            (EmotionKind::Shy, self.shy),
            (EmotionKind::Relaxed, self.relaxed),
        ];
        ranking
            .iter()
            .copied()
            .filter(|(_, value)| *value >= THRESHOLD)
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
            .map(|(kind, _)| kind)
            .unwrap_or(EmotionKind::Neutral)
    }

    /// Bridge to the legacy single-mood scheduler, so the emotion space can feed
    /// existing behavior code without forcing a rewrite.
    pub fn to_mood(&self) -> Mood {
        match self.dominant() {
            EmotionKind::Happy => Mood::Happy,
            EmotionKind::Annoyed | EmotionKind::Confused => Mood::Startled,
            EmotionKind::Sleepy => Mood::Sleepy,
            _ => Mood::Calm,
        }
    }
}

impl Default for EmotionState {
    fn default() -> Self {
        Self::baseline()
    }
}

/// A labeled emotion, for behavior systems that want a discrete signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmotionKind {
    Happy,
    Confused,
    Shy,
    Annoyed,
    Sleepy,
    Relaxed,
    Neutral,
}

/// Read-only inputs the engine needs beyond the event itself. Borrowed for the
/// duration of one [`EmotionEngine::apply`] call.
#[derive(Debug, Clone, Copy)]
pub struct EmotionContext<'a> {
    pub personality: &'a Personality,
    pub stats: &'a PetStats,
    pub memory: &'a PetMemory,
}

/// Computes the pet's current [`EmotionState`] from the event stream and the
/// pet's personality, stats and memory.
#[derive(Debug, Clone)]
pub struct EmotionEngine {
    pub state: EmotionState,
    last_ms: u64,
}

impl EmotionEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance decay for elapsed time, then fold in the event's effect.
    pub fn apply(&mut self, event: &PetEvent, now_ms: u64, ctx: EmotionContext) {
        self.decay(now_ms, ctx.personality, ctx.stats);
        self.apply_event(event, ctx);
        self.last_ms = now_ms;
    }

    fn decay(&mut self, now_ms: u64, personality: &Personality, stats: &PetStats) {
        let dt_s = now_ms.saturating_sub(self.last_ms) as f32 / 1000.0;
        if dt_s <= 0.0 {
            return;
        }
        let s = &mut self.state;
        // Transient emotions fade toward zero, lingering longer for pets that
        // hold on to feelings (playful keep happiness, sensitive keep annoyance).
        s.happy = fade(s.happy, 0.06 * dt_s * (1.0 - 0.4 * personality.playfulness));
        s.annoyed = fade(s.annoyed, 0.07 * dt_s * (1.0 - 0.6 * personality.sensitivity));
        s.shy = fade(s.shy, 0.06 * dt_s * (1.0 - 0.5 * personality.sensitivity));
        s.confused = fade(s.confused, 0.12 * dt_s);
        // Sleepiness tracks the pet's energy level; calm contentment returns to a baseline.
        s.sleepy = drift(s.sleepy, 1.0 - stats.energy, 0.05 * dt_s);
        s.relaxed = drift(s.relaxed, 0.5, 0.04 * dt_s);
    }

    fn apply_event(&mut self, event: &PetEvent, ctx: EmotionContext) {
        let personality = ctx.personality;
        let s = &mut self.state;
        match event {
            PetEvent::PetClicked { region, click_count } => match region {
                HitRegion::Head => {
                    // A pat is comforting, more so for playful/attached pets.
                    s.happy = clamp01(s.happy + 0.30 * (0.6 + 0.4 * personality.playfulness));
                    s.annoyed = fade(s.annoyed, 0.05);
                    s.shy = fade(s.shy, 0.05);
                }
                HitRegion::Body => {
                    // A poke: playful pets enjoy it, sensitive pets get irritated.
                    // The headline personality fork.
                    s.happy = clamp01(s.happy + 0.30 * personality.playfulness);
                    s.annoyed = clamp01(s.annoyed + 0.40 * personality.sensitivity);
                    // Rapid repeated pokes overstimulate sensitive pets further.
                    let jabs = (f32::from(*click_count) - 1.0).clamp(0.0, 3.0);
                    s.annoyed = clamp01(s.annoyed + 0.05 * jabs * personality.sensitivity);
                }
            },
            PetEvent::PointerNear { .. } => {
                s.confused = clamp01(s.confused + 0.10);
                s.sleepy = fade(s.sleepy, 0.05);
            }
            PetEvent::DragReleased { path, .. } => {
                if matches!(path, ReleasePath::Thrown) {
                    s.annoyed = clamp01(s.annoyed + 0.30 * (0.5 + 0.5 * personality.sensitivity));
                    s.happy = (s.happy - 0.10).max(0.0);
                    s.confused = clamp01(s.confused + 0.10);
                    // A recent fall makes the next upset land harder (memory).
                    if recently_shaken(ctx.memory) {
                        s.annoyed = clamp01(s.annoyed + 0.05);
                    }
                } else {
                    s.annoyed = clamp01(s.annoyed + 0.05 * personality.sensitivity);
                }
            }
            PetEvent::PomodoroCompleted
            | PetEvent::TodoCompleted { .. }
            | PetEvent::ReminderCompleted { .. } => {
                s.happy = clamp01(s.happy + 0.40);
                s.relaxed = clamp01(s.relaxed + 0.10);
            }
            PetEvent::PomodoroStarted => {
                s.sleepy = fade(s.sleepy, 0.10);
                s.confused = fade(s.confused, 0.05);
            }
            PetEvent::SedentaryWarning => {
                s.confused = clamp01(s.confused + 0.05);
            }
            PetEvent::UserActivityResumed => {
                s.happy = clamp01(s.happy + 0.05);
                s.sleepy = fade(s.sleepy, 0.10);
            }
            PetEvent::AppStarted => {
                s.relaxed = clamp01(s.relaxed + 0.10);
            }
            _ => {}
        }
    }
}

impl Default for EmotionEngine {
    fn default() -> Self {
        Self {
            state: EmotionState::baseline(),
            last_ms: 0,
        }
    }
}

/// True if the pet was recently dropped or startled — emotion amplifies that.
fn recently_shaken(memory: &PetMemory) -> bool {
    memory
        .recent_actions
        .iter()
        .any(|action| action == "fall" || action == "startled")
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

/// Subtract `amount` from `value`, floored at zero.
fn fade(value: f32, amount: f32) -> f32 {
    (value - amount).max(0.0)
}

/// Move `value` toward `target` by `rate` (clamped to `[0, 1]`), then clamp.
fn drift(value: f32, target: f32, rate: f32) -> f32 {
    clamp01(value + (target - value) * rate.min(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(
        personality: &Personality,
        stats: &PetStats,
        memory: &PetMemory,
    ) -> EmotionContext<'_> {
        EmotionContext {
            personality,
            stats,
            memory,
        }
    }

    fn poke(count: u8) -> PetEvent {
        PetEvent::PetClicked {
            region: HitRegion::Body,
            click_count: count,
        }
    }

    #[test]
    fn test_same_poke_yields_different_emotion_by_personality() {
        let stats = PetStats::default();
        let memory = PetMemory::default();
        let playful = Personality::playful();
        let sensitive = Personality::sensitive();

        let mut playful_engine = EmotionEngine::default();
        playful_engine.apply(&poke(1), 0, ctx(&playful, &stats, &memory));

        let mut sensitive_engine = EmotionEngine::default();
        sensitive_engine.apply(&poke(1), 0, ctx(&sensitive, &stats, &memory));

        assert_eq!(playful_engine.state.dominant(), EmotionKind::Happy);
        assert_eq!(sensitive_engine.state.dominant(), EmotionKind::Annoyed);
        assert!(playful_engine.state.happy > sensitive_engine.state.happy);
        assert!(sensitive_engine.state.annoyed > playful_engine.state.annoyed);
    }

    #[test]
    fn test_head_pat_makes_playful_pet_happy() {
        let stats = PetStats::default();
        let memory = PetMemory::default();
        let playful = Personality::playful();
        let mut engine = EmotionEngine::default();
        engine.apply(
            &PetEvent::PetClicked {
                region: HitRegion::Head,
                click_count: 1,
            },
            0,
            ctx(&playful, &stats, &memory),
        );
        assert_eq!(engine.state.dominant(), EmotionKind::Happy);
    }

    #[test]
    fn test_repeated_pokes_overstimulate_sensitive_pet() {
        let stats = PetStats::default();
        let memory = PetMemory::default();
        let sensitive = Personality::sensitive();
        let mut single = EmotionEngine::default();
        let mut barrage = EmotionEngine::default();
        single.apply(&poke(1), 0, ctx(&sensitive, &stats, &memory));
        barrage.apply(&poke(4), 0, ctx(&sensitive, &stats, &memory));
        assert!(barrage.state.annoyed > single.state.annoyed);
    }

    #[test]
    fn test_low_energy_drives_sleepiness() {
        let personality = Personality::BALANCED;
        let memory = PetMemory::default();

        let mut tired = PetStats::default();
        tired.energy = 0.1;
        let rested = PetStats::default();

        let mut tired_engine = EmotionEngine::default();
        tired_engine.apply(
            &PetEvent::Tick { now_ms: 10_000 },
            10_000,
            ctx(&personality, &tired, &memory),
        );

        let mut rested_engine = EmotionEngine::default();
        rested_engine.apply(
            &PetEvent::Tick { now_ms: 10_000 },
            10_000,
            ctx(&personality, &rested, &memory),
        );

        assert!(tired_engine.state.sleepy > rested_engine.state.sleepy);
        assert_eq!(tired_engine.state.dominant(), EmotionKind::Sleepy);
    }

    #[test]
    fn test_recent_fall_memory_amplifies_annoyance() {
        let sensitive = Personality::sensitive();
        let stats = PetStats::default();

        let mut shaken_memory = PetMemory::default();
        shaken_memory.record_action("fall");
        let calm_memory = PetMemory::default();

        let thrown = PetEvent::DragReleased {
            velocity_x: 0.0,
            velocity_y: 0.0,
            x: 0,
            y: 0,
            path: ReleasePath::Thrown,
        };

        let mut shaken = EmotionEngine::default();
        shaken.apply(&thrown, 0, ctx(&sensitive, &stats, &shaken_memory));

        let mut calm = EmotionEngine::default();
        calm.apply(&thrown, 0, ctx(&sensitive, &stats, &calm_memory));

        assert!(shaken.state.annoyed > calm.state.annoyed);
    }

    #[test]
    fn test_annoyance_decays_over_time() {
        let balanced = Personality::BALANCED;
        let sensitive = Personality::sensitive();
        let stats = PetStats::default();
        let memory = PetMemory::default();

        let mut engine = EmotionEngine::default();
        engine.apply(&poke(1), 0, ctx(&sensitive, &stats, &memory));
        let peak = engine.state.annoyed;

        engine.apply(
            &PetEvent::Tick { now_ms: 20_000 },
            20_000,
            ctx(&balanced, &stats, &memory),
        );
        assert!(engine.state.annoyed < peak);
    }

    #[test]
    fn test_completion_events_raise_happiness() {
        let balanced = Personality::BALANCED;
        let stats = PetStats::default();
        let memory = PetMemory::default();
        let mut engine = EmotionEngine::default();
        engine.apply(&PetEvent::PomodoroCompleted, 0, ctx(&balanced, &stats, &memory));
        assert_eq!(engine.state.dominant(), EmotionKind::Happy);
    }

    #[test]
    fn test_default_emotion_is_relaxed() {
        let engine = EmotionEngine::default();
        assert_eq!(engine.state.dominant(), EmotionKind::Relaxed);
    }

    #[test]
    fn test_emotion_state_bridges_to_legacy_mood() {
        let balanced = Personality::BALANCED;
        let stats = PetStats::default();
        let memory = PetMemory::default();

        let mut happy_engine = EmotionEngine::default();
        happy_engine.apply(&PetEvent::PomodoroCompleted, 0, ctx(&balanced, &stats, &memory));
        assert_eq!(happy_engine.state.to_mood(), Mood::Happy);

        assert_eq!(EmotionState::default().to_mood(), Mood::Calm);
    }

    #[test]
    fn test_personality_is_clamped_to_unit_range() {
        let p = Personality::new(-1.0, 2.0, 0.3, -0.5, 1.5);
        assert!(p.activity.abs() < 1e-6);
        assert!((p.attachment - 1.0).abs() < 1e-6);
        assert!((p.curiosity - 0.3).abs() < 1e-6);
        assert!(p.playfulness.abs() < 1e-6);
        assert!((p.sensitivity - 1.0).abs() < 1e-6);
    }
}
