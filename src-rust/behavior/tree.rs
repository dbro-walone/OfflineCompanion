//! Behavior Tree: maps a [`BehaviorIntent`] to a concrete action.
//!
//! The tree is the execution layer between the planner and the character /
//! animation layer. It groups intents into four behavior categories —
//! [`BehaviorCategory::Reactive`], [`BehaviorCategory::Life`],
//! [`BehaviorCategory::Companion`] and [`BehaviorCategory::Exploration`] — and
//! resolves each intent to an action id drawn from the existing action system
//! (`idle` / `look` / `clicked` / `head-pat` / `relax` / `startled` / ... ).
//!
//! The mapping holds no character-specific logic. Reactive intents carry the
//! highest scheduling priority so direct user input can interrupt any
//! autonomous behavior; autonomous intents (Life, Companion, Exploration) yield
//! to an open interaction session. Priorities reuse the existing [`Priority`]
//! ladder so the legacy [`super::scheduler::Scheduler`] keeps deciding what may
//! interrupt what.

use super::event::{HitRegion, PetEvent};
use super::planner::{BehaviorContext, BehaviorIntent};
use super::scheduler::Priority;

/// The four families of behavior the tree distinguishes.
///
/// Only [`BehaviorCategory::Reactive`] is treated as user-driven and allowed to
/// interrupt an in-progress interaction; the others are autonomous and yield.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorCategory {
    /// Direct responses to user input — clicks, pokes, drags. High priority.
    Reactive,
    /// Self-driven upkeep: resting and sleeping, driven by energy.
    Life,
    /// Social behaviors toward the user: noticing, approaching, inviting.
    Companion,
    /// Self-driven curiosity: exploring and observing the environment.
    Exploration,
}

/// A resolved action: what to play, at what priority, classified and explained.
///
/// `action_id` and `reason` are `&'static str` because every mapping is a fixed
/// literal — the tree never synthesizes dynamic action ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolved {
    pub action_id: &'static str,
    pub priority: Priority,
    pub category: BehaviorCategory,
    pub reason: &'static str,
}

impl BehaviorIntent {
    /// Which behavior family this intent belongs to.
    pub fn category(self) -> BehaviorCategory {
        match self {
            BehaviorIntent::Play | BehaviorIntent::Avoid => BehaviorCategory::Reactive,
            BehaviorIntent::Rest | BehaviorIntent::Sleep => BehaviorCategory::Life,
            BehaviorIntent::NoticeUser
            | BehaviorIntent::ApproachUser
            | BehaviorIntent::SeekAttention => BehaviorCategory::Companion,
            BehaviorIntent::Explore => BehaviorCategory::Exploration,
        }
    }

    /// Reactive intents respond to direct user input; all others are autonomous.
    fn is_reactive(self) -> bool {
        self.category() == BehaviorCategory::Reactive
    }
}

/// Maps intents to actions. Stateless and character-independent.
#[derive(Debug, Clone, Default)]
pub struct BehaviorTree;

impl BehaviorTree {
    /// Resolve `intent` to an action, consulting `event` for click region and
    /// `ctx` for whether an autonomous behavior may fire.
    ///
    /// Returns `None` only when an autonomous intent is asked to fire while an
    /// interaction session is open — reactive intents always resolve.
    pub fn resolve(
        &self,
        intent: BehaviorIntent,
        event: &PetEvent,
        ctx: &BehaviorContext,
    ) -> Option<Resolved> {
        // Autonomous behaviors yield to any open interaction session; reactive
        // responses to direct user input always go through.
        if !intent.is_reactive() && ctx.has_active_session {
            return None;
        }
        let category = intent.category();
        let resolved = match intent {
            BehaviorIntent::NoticeUser => Resolved {
                action_id: "look",
                priority: Priority::ProactiveInvite,
                category,
                reason: "noticed the user nearby",
            },
            BehaviorIntent::ApproachUser => Resolved {
                action_id: "look",
                priority: Priority::ProactiveInvite,
                category,
                reason: "approaching the user",
            },
            BehaviorIntent::SeekAttention => Resolved {
                action_id: "look",
                priority: Priority::ProactiveInvite,
                category,
                reason: "inviting the user back",
            },
            BehaviorIntent::Play => Resolved {
                action_id: play_action_id(event),
                priority: Priority::UserDirectInput,
                category,
                reason: "playing with the user",
            },
            BehaviorIntent::Avoid => Resolved {
                action_id: "startled",
                priority: Priority::UserDirectInput,
                category,
                reason: "flinching from overstimulation",
            },
            BehaviorIntent::Rest => Resolved {
                action_id: "relax",
                priority: Priority::IdleRandom,
                category,
                reason: "resting in place",
            },
            BehaviorIntent::Sleep => Resolved {
                action_id: "relax",
                priority: Priority::IdleRandom,
                category,
                reason: "drifting to sleep",
            },
            BehaviorIntent::Explore => Resolved {
                action_id: "look",
                priority: Priority::ProactiveInvite,
                category,
                reason: "looking the environment over",
            },
        };
        Some(resolved)
    }
}

/// A head pat plays the `head-pat` action; any other play interaction plays `clicked`.
fn play_action_id(event: &PetEvent) -> &'static str {
    match event {
        PetEvent::PetClicked { region: HitRegion::Head, .. } => "head-pat",
        _ => "clicked",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use BehaviorIntent::*;

    fn ctx() -> BehaviorContext {
        BehaviorContext::rested()
    }

    fn head() -> PetEvent {
        PetEvent::PetClicked {
            region: HitRegion::Head,
            click_count: 1,
        }
    }

    fn body() -> PetEvent {
        PetEvent::PetClicked {
            region: HitRegion::Body,
            click_count: 1,
        }
    }

    fn near() -> PetEvent {
        PetEvent::PointerNear { distance_px: 10.0 }
    }

    fn tick() -> PetEvent {
        PetEvent::Tick { now_ms: 0 }
    }

    #[test]
    fn play_resolves_to_head_pat_or_clicked_by_region() {
        let tree = BehaviorTree;
        assert_eq!(tree.resolve(Play, &head(), &ctx()).unwrap().action_id, "head-pat");
        assert_eq!(tree.resolve(Play, &body(), &ctx()).unwrap().action_id, "clicked");
    }

    #[test]
    fn avoid_resolves_to_a_reactive_startle() {
        let tree = BehaviorTree;
        let resolved = tree.resolve(Avoid, &body(), &ctx()).unwrap();
        assert_eq!(resolved.action_id, "startled");
        assert_eq!(resolved.category, BehaviorCategory::Reactive);
        assert_eq!(resolved.priority, Priority::UserDirectInput);
    }

    #[test]
    fn companion_intents_resolve_to_look_at_proactive_priority() {
        let tree = BehaviorTree;
        for intent in [NoticeUser, ApproachUser, SeekAttention] {
            let resolved = tree.resolve(intent, &near(), &ctx()).unwrap();
            assert_eq!(resolved.action_id, "look");
            assert_eq!(resolved.category, BehaviorCategory::Companion);
            assert_eq!(resolved.priority, Priority::ProactiveInvite);
        }
    }

    #[test]
    fn life_intents_resolve_to_relax_at_idle_priority() {
        let tree = BehaviorTree;
        let rest = tree.resolve(Rest, &tick(), &ctx()).unwrap();
        let sleep = tree.resolve(Sleep, &tick(), &ctx()).unwrap();
        assert_eq!(rest.action_id, "relax");
        assert_eq!(sleep.action_id, "relax");
        assert_eq!(rest.category, BehaviorCategory::Life);
        assert_eq!(sleep.category, BehaviorCategory::Life);
        assert_eq!(rest.priority, Priority::IdleRandom);
    }

    #[test]
    fn explore_resolves_to_look_under_exploration() {
        let tree = BehaviorTree;
        let resolved = tree.resolve(Explore, &tick(), &ctx()).unwrap();
        assert_eq!(resolved.action_id, "look");
        assert_eq!(resolved.category, BehaviorCategory::Exploration);
    }

    #[test]
    fn reactive_intents_fire_during_an_open_session() {
        let tree = BehaviorTree;
        let busy = BehaviorContext {
            has_active_session: true,
            ..ctx()
        };
        assert!(tree.resolve(Play, &body(), &busy).is_some());
        assert!(tree.resolve(Avoid, &body(), &busy).is_some());
    }

    #[test]
    fn autonomous_intents_yield_to_an_open_session() {
        let tree = BehaviorTree;
        let busy = BehaviorContext {
            has_active_session: true,
            ..ctx()
        };
        assert!(tree.resolve(Rest, &tick(), &busy).is_none());
        assert!(tree.resolve(Sleep, &tick(), &busy).is_none());
        assert!(tree.resolve(Explore, &tick(), &busy).is_none());
        assert!(tree.resolve(SeekAttention, &tick(), &busy).is_none());
    }

    #[test]
    fn categories_partition_the_intents() {
        assert_eq!(Play.category(), BehaviorCategory::Reactive);
        assert_eq!(Avoid.category(), BehaviorCategory::Reactive);
        assert_eq!(Rest.category(), BehaviorCategory::Life);
        assert_eq!(Sleep.category(), BehaviorCategory::Life);
        assert_eq!(NoticeUser.category(), BehaviorCategory::Companion);
        assert_eq!(ApproachUser.category(), BehaviorCategory::Companion);
        assert_eq!(SeekAttention.category(), BehaviorCategory::Companion);
        assert_eq!(Explore.category(), BehaviorCategory::Exploration);
    }
}
