//! Behavior Planner + Intent model.
//!
//! Sits between Emotion and the action layer. The pipeline is
//! `Event -> State -> Emotion -> Behavior -> Action`; this module owns the
//! `Behavior` step's *intent*: an [`EmotionEngine`] has already turned the
//! event into an [`EmotionState`], and the planner now reads the event plus the
//! pet's [`PetStats`], [`EmotionState`], [`Personality`] and a snapshot of live
//! state to produce an optional [`BehaviorIntent`] — a character-independent
//! statement of *what the pet wants to do*. The [`super::tree::BehaviorTree`]
//! then turns an intent into a concrete action id.
//!
//! Every decision is a pure function of the event and the supplied
//! [`BehaviorContext`]; nothing here references a specific character, so the
//! same intent can be expressed by any character pack.

use super::emotion::{EmotionState, Personality};
use super::event::{HitRegion, PetEvent};
use super::state::PetState;

// Decision thresholds -------------------------------------------------------

/// Energy at or below which the pet must sleep rather than do anything social.
const SLEEP_ENERGY: f32 = 0.15;
/// Energy at or below which the pet rests instead of engaging or exploring.
const REST_ENERGY: f32 = 0.3;
/// Personality sensitivity above which a body poke reads as overstimulation.
const SENSITIVE: f32 = 0.65;
/// Personality activity above which long idleness drives attention-seeking.
const ACTIVE: f32 = 0.65;
/// Personality curiosity above which the pet explores its environment.
const CURIOUS: f32 = 0.65;
/// Stat curiosity above which the pet explores regardless of personality.
const CURIOUS_STAT: f32 = 0.7;
/// Attachment above which a nearby user is approached rather than just noticed.
const ATTACHED: f32 = 0.6;
/// Affinity at which the pet feels bonded enough to approach.
const BONDED: f32 = 0.7;
/// Annoyance intensity that counts as a salient negative emotion.
const ANNOYED_SALIENT: f32 = 0.3;
/// Repeated pokes past this count annoy even before annoyance is otherwise salient.
const RAPID_POKE_COUNT: u8 = 3;
/// How long the pet can be idle before an active personality seeks attention.
const ATTENTION_IDLE_MS: u64 = 60_000;

/// A character-independent statement of what the pet wants to do next.
///
/// Intents deliberately say nothing about *how* to do something — that is the
/// [`super::tree::BehaviorTree`]'s job — so two different characters can express
/// the same [`BehaviorIntent`] with different animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorIntent {
    /// The pet has noticed the user nearby.
    NoticeUser,
    /// The pet wants to move toward the user.
    ApproachUser,
    /// The pet wants to retreat from overstimulation.
    Avoid,
    /// The pet wants to play with the user.
    Play,
    /// The pet wants to rest in place.
    Rest,
    /// The pet wants to sleep.
    Sleep,
    /// The pet wants the user's attention after being left alone.
    SeekAttention,
    /// The pet wants to explore or observe its environment.
    Explore,
}

impl BehaviorIntent {
    /// Stable lowercase label, useful for tracing and tests.
    pub fn label(self) -> &'static str {
        match self {
            BehaviorIntent::NoticeUser => "notice-user",
            BehaviorIntent::ApproachUser => "approach-user",
            BehaviorIntent::Avoid => "avoid",
            BehaviorIntent::Play => "play",
            BehaviorIntent::Rest => "rest",
            BehaviorIntent::Sleep => "sleep",
            BehaviorIntent::SeekAttention => "seek-attention",
            BehaviorIntent::Explore => "explore",
        }
    }
}

/// Read-only snapshot of everything the planner needs beyond the event itself.
///
/// All fields are [`Copy`], so the controller can refresh the live portions
/// (state-machine state, timers, flags) on each turn and hand the planner an
/// owned snapshot without fighting the borrow checker. The emotion-driven fields
/// (`energy`, `affinity`, `curiosity`, `emotion`, `personality`) are fed in by
/// the runtime; the controller-owned fields are refreshed per decision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BehaviorContext {
    /// Current energy in `[0, 1]`.
    pub energy: f32,
    /// Current affinity in `[0, 1]`.
    pub affinity: f32,
    /// Current curiosity in `[0, 1]`.
    pub curiosity: f32,
    /// Current emotion space.
    pub emotion: EmotionState,
    /// Fixed personality profile.
    pub personality: Personality,
    /// Current state-machine state.
    pub state: PetState,
    /// When the user last interacted with the pet, if ever.
    pub last_interaction_ms: Option<u64>,
    /// Whether an interaction session is currently open.
    pub has_active_session: bool,
    /// Whether proactive invites are allowed right now (flags + quiet + cooldown).
    pub proactive_allowed: bool,
}

impl BehaviorContext {
    /// A rested, neutral baseline used by default and by tests.
    pub fn rested() -> Self {
        Self {
            energy: 1.0,
            affinity: 0.5,
            curiosity: 0.4,
            emotion: EmotionState::baseline(),
            personality: Personality::BALANCED,
            state: PetState::Idle,
            last_interaction_ms: None,
            has_active_session: false,
            proactive_allowed: true,
        }
    }
}

/// States where the legacy director owns the action (physics, focus, recovery).
/// The planner stays out of these so the verified transitions in `director.rs`
/// are preserved byte-for-byte.
fn owned_by_legacy(state: PetState) -> bool {
    matches!(
        state,
        PetState::Dragging
            | PetState::Falling
            | PetState::Landing
            | PetState::OnEdge
            | PetState::Recovering
            | PetState::FocusCompanion
            | PetState::Sleeping
    )
}

/// Turns `Event + State + Emotion + Personality` into an optional intent.
///
/// The planner is stateless: every decision is a pure function of the event and
/// the supplied [`BehaviorContext`], which makes the behavior rules trivial to
/// unit-test in isolation.
#[derive(Debug, Clone, Default)]
pub struct BehaviorPlanner;

impl BehaviorPlanner {
    /// Decide what the pet wants to do for `event`, given `ctx`.
    ///
    /// Returns `None` when no behavior-layer intent applies — either because the
    /// event is physical/system-level or because the pet is in a legacy-owned
    /// state — which lets the caller fall back to the original event-to-action
    /// mapping.
    pub fn plan(
        &self,
        event: &PetEvent,
        now_ms: u64,
        ctx: &BehaviorContext,
    ) -> Option<BehaviorIntent> {
        if owned_by_legacy(ctx.state) {
            return None;
        }
        match event {
            PetEvent::PointerNear { .. } => self.plan_for_proximity(ctx),
            PetEvent::PetClicked { region, click_count } => {
                Some(self.plan_for_click(*region, *click_count, ctx))
            }
            PetEvent::Tick { .. } => self.plan_for_life(now_ms, ctx),
            _ => None,
        }
    }

    /// A nearby pointer: approach a bonded user, notice a stranger, or rest when tired.
    fn plan_for_proximity(&self, ctx: &BehaviorContext) -> Option<BehaviorIntent> {
        if !ctx.proactive_allowed {
            return None;
        }
        if ctx.energy <= REST_ENERGY {
            return Some(BehaviorIntent::Rest);
        }
        let wants_closeness = ctx.personality.attachment >= ATTACHED || ctx.affinity >= BONDED;
        Some(if wants_closeness {
            BehaviorIntent::ApproachUser
        } else {
            BehaviorIntent::NoticeUser
        })
    }

    /// A click: head pats are always play; body pokes play, or startle a
    /// sensitive pet that is already annoyed.
    fn plan_for_click(
        &self,
        region: HitRegion,
        click_count: u8,
        ctx: &BehaviorContext,
    ) -> BehaviorIntent {
        if region == HitRegion::Head {
            return BehaviorIntent::Play;
        }
        if self.is_overstimulated(click_count, ctx) {
            return BehaviorIntent::Avoid;
        }
        BehaviorIntent::Play
    }

    /// A sensitive pet flinches from pokes once annoyance dominates happiness,
    /// or once the pokes come in a rapid barrage.
    fn is_overstimulated(&self, click_count: u8, ctx: &BehaviorContext) -> bool {
        let sensitive = ctx.personality.sensitivity > SENSITIVE;
        let annoyed_dominant = ctx.emotion.annoyed >= ANNOYED_SALIENT
            && ctx.emotion.annoyed > ctx.emotion.happy;
        sensitive && (annoyed_dominant || click_count >= RAPID_POKE_COUNT)
    }

    /// An idle tick drives the autonomous life loop: sleep when exhausted, seek
    /// attention when ignored and restless, explore when curious, otherwise rest
    /// when low on energy. Returns `None` when nothing is salient, leaving the
    /// idle scheduler free to pick a random animation.
    fn plan_for_life(&self, now_ms: u64, ctx: &BehaviorContext) -> Option<BehaviorIntent> {
        if ctx.has_active_session || ctx.state != PetState::Idle {
            return None;
        }
        if ctx.energy <= SLEEP_ENERGY {
            return Some(BehaviorIntent::Sleep);
        }
        let idle_ms = ctx.last_interaction_ms.map_or(now_ms, |last| now_ms.saturating_sub(last));
        if idle_ms >= ATTENTION_IDLE_MS && ctx.personality.activity > ACTIVE {
            return Some(BehaviorIntent::SeekAttention);
        }
        if ctx.personality.curiosity > CURIOUS || ctx.curiosity >= CURIOUS_STAT {
            return Some(BehaviorIntent::Explore);
        }
        if ctx.energy <= REST_ENERGY {
            return Some(BehaviorIntent::Rest);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavior::locomotion::ReleasePath;

    fn ctx() -> BehaviorContext {
        BehaviorContext::rested()
    }

    /// An emotion space dominated by the given annoyance level.
    fn annoyed(level: f32) -> EmotionState {
        EmotionState {
            annoyed: level,
            ..EmotionState::baseline()
        }
    }

    /// An emotion space dominated by the given happiness level.
    fn happy(level: f32) -> EmotionState {
        EmotionState {
            happy: level,
            ..EmotionState::baseline()
        }
    }

    fn near() -> PetEvent {
        PetEvent::PointerNear { distance_px: 10.0 }
    }

    fn body(count: u8) -> PetEvent {
        PetEvent::PetClicked {
            region: HitRegion::Body,
            click_count: count,
        }
    }

    fn head() -> PetEvent {
        PetEvent::PetClicked {
            region: HitRegion::Head,
            click_count: 1,
        }
    }

    fn tick(now_ms: u64) -> PetEvent {
        PetEvent::Tick { now_ms }
    }

    #[test]
    fn proximity_approaches_a_bonded_energetic_pet() {
        let planner = BehaviorPlanner;
        let bonded = BehaviorContext {
            affinity: 0.8,
            ..ctx()
        };
        let attached = BehaviorContext {
            personality: Personality::new(0.5, 0.8, 0.5, 0.5, 0.5),
            ..ctx()
        };
        assert_eq!(planner.plan(&near(), 0, &bonded), Some(BehaviorIntent::ApproachUser));
        assert_eq!(planner.plan(&near(), 0, &attached), Some(BehaviorIntent::ApproachUser));
    }

    #[test]
    fn proximity_notices_a_stranger() {
        let planner = BehaviorPlanner;
        let stranger = BehaviorContext {
            affinity: 0.3,
            ..ctx()
        };
        assert_eq!(planner.plan(&near(), 0, &stranger), Some(BehaviorIntent::NoticeUser));
    }

    #[test]
    fn proximity_rests_when_tired() {
        let planner = BehaviorPlanner;
        let tired = BehaviorContext {
            energy: 0.2,
            ..ctx()
        };
        assert_eq!(planner.plan(&near(), 0, &tired), Some(BehaviorIntent::Rest));
    }

    #[test]
    fn proximity_is_ignored_when_proactive_invites_are_disabled() {
        let planner = BehaviorPlanner;
        let muted = BehaviorContext {
            proactive_allowed: false,
            ..ctx()
        };
        assert_eq!(planner.plan(&near(), 0, &muted), None);
    }

    #[test]
    fn head_click_always_plays() {
        let planner = BehaviorPlanner;
        let grumpy = BehaviorContext {
            personality: Personality::sensitive(),
            emotion: annoyed(0.9),
            ..ctx()
        };
        assert_eq!(planner.plan(&head(), 0, &grumpy), Some(BehaviorIntent::Play));
    }

    #[test]
    fn body_click_plays_for_a_playful_pet() {
        let planner = BehaviorPlanner;
        let playful = BehaviorContext {
            personality: Personality::playful(),
            emotion: happy(0.7),
            ..ctx()
        };
        assert_eq!(planner.plan(&body(1), 0, &playful), Some(BehaviorIntent::Play));
    }

    #[test]
    fn body_click_avoids_for_an_annoyed_sensitive_pet() {
        let planner = BehaviorPlanner;
        let irked = BehaviorContext {
            personality: Personality::sensitive(),
            emotion: annoyed(0.6),
            ..ctx()
        };
        assert_eq!(planner.plan(&body(2), 0, &irked), Some(BehaviorIntent::Avoid));
    }

    #[test]
    fn body_click_avoids_after_a_rapid_barrage_even_when_annoyance_is_low() {
        let planner = BehaviorPlanner;
        let sensitive = BehaviorContext {
            personality: Personality::sensitive(),
            ..ctx()
        };
        assert_eq!(
            planner.plan(&body(RAPID_POKE_COUNT), 0, &sensitive),
            Some(BehaviorIntent::Avoid)
        );
    }

    #[test]
    fn life_sleeps_when_exhausted() {
        let planner = BehaviorPlanner;
        let exhausted = BehaviorContext {
            energy: 0.1,
            ..ctx()
        };
        assert_eq!(planner.plan(&tick(0), 0, &exhausted), Some(BehaviorIntent::Sleep));
    }

    #[test]
    fn life_seeks_attention_when_idle_long_enough_and_active() {
        let planner = BehaviorPlanner;
        let restless = BehaviorContext {
            personality: Personality::new(0.8, 0.5, 0.5, 0.5, 0.5),
            last_interaction_ms: Some(0),
            ..ctx()
        };
        assert_eq!(
            planner.plan(&tick(70_000), 70_000, &restless),
            Some(BehaviorIntent::SeekAttention)
        );
    }

    #[test]
    fn life_explores_when_curious() {
        let planner = BehaviorPlanner;
        let curious = BehaviorContext {
            personality: Personality::new(0.5, 0.5, 0.8, 0.5, 0.5),
            ..ctx()
        };
        assert_eq!(planner.plan(&tick(0), 0, &curious), Some(BehaviorIntent::Explore));
    }

    #[test]
    fn life_rests_when_low_but_not_exhausted() {
        let planner = BehaviorPlanner;
        let drowsy = BehaviorContext {
            energy: 0.2,
            ..ctx()
        };
        assert_eq!(planner.plan(&tick(0), 0, &drowsy), Some(BehaviorIntent::Rest));
    }

    #[test]
    fn life_does_nothing_on_a_default_idle_tick() {
        let planner = BehaviorPlanner;
        assert_eq!(planner.plan(&tick(0), 0, &ctx()), None);
    }

    #[test]
    fn life_does_not_fire_during_an_open_session() {
        let planner = BehaviorPlanner;
        let exhausted = BehaviorContext {
            energy: 0.1,
            has_active_session: true,
            ..ctx()
        };
        assert_eq!(planner.plan(&tick(0), 0, &exhausted), None);
    }

    #[test]
    fn physical_events_produce_no_intent() {
        let planner = BehaviorPlanner;
        let drag = PetEvent::DragReleased {
            velocity_x: 0.0,
            velocity_y: 0.0,
            x: 0,
            y: 0,
            path: ReleasePath::Drop,
        };
        let landing = PetEvent::Landing {
            path: ReleasePath::Drop,
        };
        assert_eq!(planner.plan(&drag, 0, &ctx()), None);
        assert_eq!(planner.plan(&landing, 0, &ctx()), None);
    }

    #[test]
    fn no_intent_while_in_a_legacy_owned_state() {
        let planner = BehaviorPlanner;
        let recovering = BehaviorContext {
            state: PetState::Recovering,
            ..ctx()
        };
        assert_eq!(planner.plan(&body(1), 0, &recovering), None);
        assert_eq!(planner.plan(&tick(0), 0, &recovering), None);
    }
}
