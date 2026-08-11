//! Numeric companion state, independent of any character.
//!
//! [`PetStats`] models the slow-moving numerical dimensions of the pet — energy,
//! affinity and curiosity — as normalized intensities in `[0.0, 1.0]`. They drift
//! toward rest over time and are pushed around by events, but contain no
//! character-specific fields. Behavior systems read these numbers to pick a
//! response; they never live inside a particular character.

use super::event::{HitRegion, PetEvent};
use super::event_bus::EventSubscriber;
use super::locomotion::ReleasePath;

/// Passive drain applied to every stat, per second of wall-clock time.
const ENERGY_DRAIN_PER_S: f32 = 0.02;
const AFFINITY_DRAIN_PER_S: f32 = 0.005;
const CURIOSITY_DRAIN_PER_S: f32 = 0.03;

/// Default cap on the recorded change history (matches the decision-trace budget
/// used elsewhere in the runtime).
const DEFAULT_HISTORY_CAP: usize = 64;

/// A change too small to be worth recording or acting on.
const EPSILON: f32 = 1e-6;

/// Which stat a [`StatChangeEntry`] refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatKind {
    Energy,
    Affinity,
    Curiosity,
}

/// A single recorded mutation of a stat: its new value, when it happened and why.
#[derive(Debug, Clone, PartialEq)]
pub struct StatChangeEntry {
    pub stat: StatKind,
    pub value: f32,
    pub at_ms: u64,
    pub reason: String,
}

/// Character-independent numerical state with a bounded change history.
///
/// Each dimension is a normalized intensity in `[0.0, 1.0]`. Values passively
/// decay over time via [`PetStats::advance`] and react to events via
/// [`PetStats::apply`]. Only event-driven changes are recorded in the history;
/// passive decay is not, so the log reflects discrete interactions rather than
/// every idle tick.
#[derive(Debug, Clone, PartialEq)]
pub struct PetStats {
    pub energy: f32,
    pub affinity: f32,
    pub curiosity: f32,
    history: Vec<StatChangeEntry>,
    cap: usize,
    updated_ms: u64,
}

impl PetStats {
    /// Fresh state: fully rested, neutral bond, mildly curious.
    pub fn new() -> Self {
        Self {
            energy: 1.0,
            affinity: 0.5,
            curiosity: 0.4,
            history: Vec::new(),
            cap: DEFAULT_HISTORY_CAP,
            updated_ms: 0,
        }
    }

    /// Same as [`PetStats::new`] but with a custom history capacity (min 1).
    pub fn with_history_cap(cap: usize) -> Self {
        let mut stats = Self::new();
        stats.cap = cap.max(1);
        stats
    }

    /// Apply passive decay for the wall-clock elapsed since the last update.
    pub fn advance(&mut self, now_ms: u64) {
        let dt_s = now_ms.saturating_sub(self.updated_ms) as f32 / 1000.0;
        if dt_s <= 0.0 {
            return;
        }
        self.energy = (self.energy - ENERGY_DRAIN_PER_S * dt_s).max(0.0);
        self.affinity = (self.affinity - AFFINITY_DRAIN_PER_S * dt_s).max(0.0);
        self.curiosity = (self.curiosity - CURIOSITY_DRAIN_PER_S * dt_s).max(0.0);
        self.updated_ms = now_ms;
    }

    /// Advance time, then apply an event's effect. Tick events only advance time.
    pub fn apply(&mut self, event: &PetEvent, now_ms: u64) {
        self.advance(now_ms);
        self.apply_event(event);
    }

    fn apply_event(&mut self, event: &PetEvent) {
        match event {
            PetEvent::AppStarted => {
                self.adjust(
                    StatKind::Energy,
                    1.0 - self.energy,
                    "app started: refreshed",
                );
                self.adjust(StatKind::Curiosity, 0.05, "app started: looking around");
            }
            PetEvent::PointerEntered { .. } => {
                self.adjust(StatKind::Curiosity, 0.04, "pointer entered");
            }
            PetEvent::PointerNear { .. } => {
                self.adjust(StatKind::Curiosity, 0.08, "pointer approached");
                self.adjust(StatKind::Affinity, 0.01, "pointer approached");
            }
            PetEvent::PointerExited => {
                self.adjust(StatKind::Curiosity, -0.03, "pointer left");
            }
            PetEvent::PetClicked { region, .. } => match region {
                HitRegion::Head => {
                    self.adjust(StatKind::Affinity, 0.12, "head pat");
                    self.adjust(StatKind::Energy, -0.02, "head pat");
                    self.adjust(StatKind::Curiosity, 0.03, "head pat");
                }
                HitRegion::Body => {
                    self.adjust(StatKind::Affinity, 0.04, "body poke");
                    self.adjust(StatKind::Energy, -0.01, "body poke");
                }
            },
            PetEvent::DragStarted { .. } => {
                self.adjust(StatKind::Energy, -0.01, "drag started");
                self.adjust(StatKind::Curiosity, 0.02, "drag started");
            }
            PetEvent::DragMoved { .. } => {
                self.adjust(StatKind::Energy, -0.005, "drag moved");
            }
            PetEvent::DragReleased { path, .. } => match path {
                ReleasePath::Thrown => {
                    self.adjust(StatKind::Affinity, -0.15, "thrown release");
                    self.adjust(StatKind::Energy, -0.05, "thrown release");
                }
                ReleasePath::Drop => {
                    self.adjust(StatKind::Affinity, -0.02, "dropped");
                    self.adjust(StatKind::Energy, -0.03, "dropped");
                }
                ReleasePath::EdgeLeft | ReleasePath::EdgeRight => {
                    self.adjust(StatKind::Curiosity, 0.05, "parked on edge");
                }
            },
            PetEvent::Landing { .. } => {
                self.adjust(StatKind::Energy, -0.02, "landing");
            }
            PetEvent::ReminderRaised { .. } | PetEvent::SedentaryWarning => {
                self.adjust(StatKind::Curiosity, 0.06, "business reminder");
            }
            PetEvent::PomodoroStarted => {
                self.adjust(StatKind::Energy, -0.03, "focus session");
            }
            PetEvent::PomodoroCompleted => {
                self.adjust(StatKind::Affinity, 0.06, "pomodoro completed");
                self.adjust(StatKind::Energy, 0.05, "pomodoro completed");
            }
            PetEvent::TodoCompleted { .. } | PetEvent::ReminderCompleted { .. } => {
                self.adjust(StatKind::Affinity, 0.08, "task completed");
                self.adjust(StatKind::Energy, 0.04, "task completed");
            }
            PetEvent::UserActivityResumed => {
                self.adjust(StatKind::Energy, 0.05, "user returned");
                self.adjust(StatKind::Curiosity, 0.05, "user returned");
            }
            _ => {}
        }
    }

    /// Apply a delta to one stat, clamping to `[0, 1]` and recording the change.
    /// No-op entries (a delta that does not move the value) are skipped so the
    /// history reflects meaningful interactions only.
    fn adjust(&mut self, stat: StatKind, delta: f32, reason: &str) {
        let current = self.value_of(stat);
        let next = clamp01(current + delta);
        if (next - current).abs() < EPSILON {
            return;
        }
        self.set_value(stat, next);
        self.record(StatChangeEntry {
            stat,
            value: next,
            at_ms: self.updated_ms,
            reason: reason.to_owned(),
        });
    }

    fn value_of(&self, stat: StatKind) -> f32 {
        match stat {
            StatKind::Energy => self.energy,
            StatKind::Affinity => self.affinity,
            StatKind::Curiosity => self.curiosity,
        }
    }

    fn set_value(&mut self, stat: StatKind, value: f32) {
        match stat {
            StatKind::Energy => self.energy = value,
            StatKind::Affinity => self.affinity = value,
            StatKind::Curiosity => self.curiosity = value,
        }
    }

    fn record(&mut self, entry: StatChangeEntry) {
        self.history.push(entry);
        if self.history.len() > self.cap {
            self.history.remove(0);
        }
    }

    pub fn history(&self) -> &[StatChangeEntry] {
        &self.history
    }

    pub fn last_change(&self) -> Option<&StatChangeEntry> {
        self.history.last()
    }

    /// True when the pet is too tired to be proactive.
    pub fn is_exhausted(&self) -> bool {
        self.energy <= 0.15
    }

    /// True when enough positive interaction has accumulated to feel bonded.
    pub fn is_bonded(&self) -> bool {
        self.affinity >= 0.7
    }

    /// True when curiosity is running high enough to warrant exploration.
    pub fn is_inquisitive(&self) -> bool {
        self.curiosity >= 0.6
    }
}

impl Default for PetStats {
    fn default() -> Self {
        Self::new()
    }
}

/// A [`PetStats`] can plug straight into an [`EventBus`] as a state engine:
/// every published event advances its clock and applies the event's effect.
impl EventSubscriber for PetStats {
    fn on_event(&mut self, event: &PetEvent, now_ms: u64) {
        self.apply(event, now_ms);
    }
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head_pat(count: u8) -> PetEvent {
        PetEvent::PetClicked {
            region: HitRegion::Head,
            click_count: count,
        }
    }

    #[test]
    fn test_head_pat_raises_affinity_and_records_history() {
        let mut stats = PetStats::default();
        stats.apply(&head_pat(1), 0);

        assert!(stats.affinity > 0.5);
        assert!(stats.energy < 1.0);
        assert!(
            stats
                .history()
                .iter()
                .any(|entry| entry.stat == StatKind::Affinity && entry.reason == "head pat")
        );
    }

    #[test]
    fn test_stats_are_event_subscriber() {
        let mut stats = PetStats::default();
        // Drive the state engine purely through the subscriber trait method.
        stats.on_event(&head_pat(1), 0);
        assert!(stats.affinity > 0.5);
        assert!(stats.last_change().is_some());
    }

    #[test]
    fn test_app_started_refreshes_energy() {
        let mut stats = PetStats::default();
        stats.energy = 0.2;
        stats.apply(&PetEvent::AppStarted, 0);
        assert!((stats.energy - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_stats_decay_over_time() {
        let mut stats = PetStats::default();
        stats.advance(10_000);
        assert!((stats.energy - 0.8).abs() < 1e-4);
        assert!((stats.affinity - 0.45).abs() < 1e-4);
        assert!((stats.curiosity - 0.1).abs() < 1e-4);
    }

    #[test]
    fn test_decay_does_not_overshoot_when_clock_stalls() {
        let mut stats = PetStats::default();
        stats.advance(5_000);
        let energy_after_decay = stats.energy;
        // Re-advancing at the same timestamp applies no further decay.
        stats.advance(5_000);
        assert!((stats.energy - energy_after_decay).abs() < 1e-6);
    }

    #[test]
    fn test_stats_clamp_to_unit_range() {
        let mut stats = PetStats::default();
        for _ in 0..50 {
            stats.apply(&head_pat(1), 0);
        }
        assert!(stats.affinity <= 1.0);
        assert!(stats.energy >= 0.0);
        assert!(stats.curiosity <= 1.0);
    }

    #[test]
    fn test_history_is_capped() {
        let mut stats = PetStats::with_history_cap(5);
        for _ in 0..20 {
            stats.apply(&PetEvent::PointerNear { distance_px: 10.0 }, 0);
            stats.apply(&PetEvent::PointerExited, 0);
        }
        assert_eq!(stats.history().len(), 5);
    }

    #[test]
    fn test_thrown_release_costs_affinity_more_than_a_drop() {
        let mut thrown = PetStats::default();
        let mut dropped = PetStats::default();
        thrown.apply(
            &PetEvent::DragReleased {
                velocity_x: 0.0,
                velocity_y: 0.0,
                x: 0,
                y: 0,
                path: ReleasePath::Thrown,
            },
            0,
        );
        dropped.apply(
            &PetEvent::DragReleased {
                velocity_x: 0.0,
                velocity_y: 0.0,
                x: 0,
                y: 0,
                path: ReleasePath::Drop,
            },
            0,
        );
        assert!(thrown.affinity < dropped.affinity);
    }

    #[test]
    fn test_derived_stat_predicates() {
        let mut stats = PetStats::default();
        assert!(!stats.is_exhausted());
        assert!(!stats.is_bonded());

        stats.energy = 0.1;
        assert!(stats.is_exhausted());

        stats.affinity = 0.8;
        assert!(stats.is_bonded());

        stats.curiosity = 0.7;
        assert!(stats.is_inquisitive());
    }
}
