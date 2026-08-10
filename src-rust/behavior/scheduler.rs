use crate::package_runtime::catalog::RuntimeAction;
use std::collections::{HashMap, VecDeque};

pub const FIRST_IDLE_DELAY_MS: u64 = 10_000;
pub const MIN_IDLE_INTERVAL_MS: u64 = 20_000;
pub const MAX_IDLE_INTERVAL_MS: u64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Priority {
    IdleRandom = 10,
    ProactiveInvite = 30,
    FocusCompanion = 50,
    BusinessFeedback = 70,
    ReminderAttention = 80,
    UserDirectInput = 90,
    SystemSafety = 100,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledAction {
    pub id: String,
    pub priority: Priority,
    pub not_before_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveAction {
    pub id: String,
    pub priority: Priority,
    pub started_at_ms: u64,
    completion_emitted: bool,
}

impl ActiveAction {
    pub fn new(action: &ScheduledAction, now_ms: u64) -> Self {
        Self {
            id: action.id.clone(),
            priority: action.priority,
            started_at_ms: now_ms,
            completion_emitted: false,
        }
    }

    pub fn mark_completed_once(&mut self) -> bool {
        if self.completion_emitted {
            false
        } else {
            self.completion_emitted = true;
            true
        }
    }
}

#[derive(Debug, Default)]
pub struct Scheduler {
    current: Option<ActiveAction>,
    pending: Option<ScheduledAction>,
}

impl Scheduler {
    pub fn accepts(&self, action: &ScheduledAction) -> bool {
        self.current
            .as_ref()
            .is_none_or(|current| action.priority >= current.priority)
    }

    pub fn activate(&mut self, action: &ScheduledAction, now_ms: u64) {
        self.current = Some(ActiveAction::new(action, now_ms));
        if self
            .pending
            .as_ref()
            .is_some_and(|item| item.id == action.id)
        {
            self.pending = None;
        }
    }

    pub fn current(&self) -> Option<&ActiveAction> {
        self.current.as_ref()
    }

    pub fn current_mut(&mut self) -> Option<&mut ActiveAction> {
        self.current.as_mut()
    }

    pub fn queue(&mut self, action: ScheduledAction) {
        if self
            .pending
            .as_ref()
            .is_none_or(|pending| action.priority >= pending.priority)
        {
            self.pending = Some(action);
        }
    }

    pub fn take_pending(&mut self) -> Option<ScheduledAction> {
        self.pending.take()
    }

    pub fn clear(&mut self) -> Option<ActiveAction> {
        self.current.take()
    }
}

pub trait RandomSource: std::fmt::Debug {
    fn next_u64(&mut self) -> u64;
}

#[derive(Debug, Clone)]
pub struct SeededRng {
    state: u64,
}

impl SeededRng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }
}

impl RandomSource for SeededRng {
    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }
}

#[derive(Debug)]
pub struct IdleScheduler<R: RandomSource = SeededRng> {
    rng: R,
    first_due_ms: u64,
    next_due_ms: u64,
    recent: VecDeque<String>,
    last_played: HashMap<String, u64>,
}

impl Default for IdleScheduler<SeededRng> {
    fn default() -> Self {
        Self::new(0, SeededRng::new(0xC0FFEE))
    }
}

impl<R: RandomSource> IdleScheduler<R> {
    pub fn new(started_at_ms: u64, rng: R) -> Self {
        let first_due_ms = started_at_ms.saturating_add(FIRST_IDLE_DELAY_MS);
        Self {
            rng,
            first_due_ms,
            next_due_ms: first_due_ms,
            recent: VecDeque::new(),
            last_played: HashMap::new(),
        }
    }

    pub fn reset(&mut self, now_ms: u64) {
        self.first_due_ms = now_ms.saturating_add(FIRST_IDLE_DELAY_MS);
        self.next_due_ms = self.first_due_ms;
        self.recent.clear();
        self.last_played.clear();
    }

    pub fn select(
        &mut self,
        now_ms: u64,
        enabled: bool,
        blocked: bool,
        actions: &[RuntimeAction],
    ) -> Option<String> {
        if !enabled || blocked || now_ms < self.next_due_ms {
            return None;
        }

        let candidates = actions
            .iter()
            .filter(|action| action.trigger == "idle-random")
            .filter(|action| {
                self.last_played
                    .get(&action.id)
                    .is_none_or(|last| now_ms.saturating_sub(*last) >= action.cooldown_ms)
            })
            .filter(|action| !self.recent.iter().any(|id| id == &action.id))
            .collect::<Vec<_>>();

        self.schedule_next(now_ms);
        let total = candidates
            .iter()
            .map(|action| u64::from(action.weight.max(1)))
            .sum::<u64>();
        if total == 0 {
            return None;
        }
        let mut ticket = self.rng.next_u64() % total;
        let selected = candidates.into_iter().find(|action| {
            let weight = u64::from(action.weight.max(1));
            if ticket < weight {
                true
            } else {
                ticket -= weight;
                false
            }
        })?;
        self.last_played.insert(selected.id.clone(), now_ms);
        self.recent.push_back(selected.id.clone());
        while self.recent.len() > 3 {
            self.recent.pop_front();
        }
        Some(selected.id.clone())
    }

    pub fn select_or_default(
        &mut self,
        now_ms: u64,
        enabled: bool,
        blocked: bool,
        actions: &[RuntimeAction],
        default_action: &str,
    ) -> Option<String> {
        let due = enabled && !blocked && now_ms >= self.next_due_ms;
        self.select(now_ms, enabled, blocked, actions)
            .or_else(|| due.then(|| default_action.to_owned()))
    }

    fn schedule_next(&mut self, now_ms: u64) {
        let range = MAX_IDLE_INTERVAL_MS - MIN_IDLE_INTERVAL_MS + 1;
        self.next_due_ms = now_ms
            .saturating_add(MIN_IDLE_INTERVAL_MS)
            .saturating_add(self.rng.next_u64() % range);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package_runtime::catalog::RenderSource;
    use std::path::PathBuf;

    #[derive(Debug)]
    struct FixedRng(VecDeque<u64>);

    impl RandomSource for FixedRng {
        fn next_u64(&mut self) -> u64 {
            self.0.pop_front().unwrap_or(0)
        }
    }

    fn action(id: &str, weight: u32, cooldown_ms: u64) -> RuntimeAction {
        RuntimeAction {
            id: id.into(),
            semantic: id.into(),
            trigger: "idle-random".into(),
            weight,
            cooldown_ms,
            priority: 10,
            animation_path: PathBuf::from("animation.json"),
            render: RenderSource {
                atlas_path: PathBuf::from("atlas.png"),
                frame_width: 1,
                frame_height: 1,
                columns: 1,
                rows: 1,
            },
        }
    }

    #[test]
    fn test_idle_action_is_selected_from_enabled_action_pack() {
        let mut scheduler = IdleScheduler::new(0, FixedRng(VecDeque::from([0, 0])));
        assert_eq!(
            scheduler.select(10_000, true, false, &[action("idle.thinking", 20, 0)]),
            Some("idle.thinking".into())
        );
    }

    #[test]
    fn test_idle_action_respects_weight_and_cooldown() {
        let mut scheduler = IdleScheduler::new(0, FixedRng(VecDeque::from([0, 9, 0, 0])));
        let actions = [action("light", 1, 0), action("heavy", 9, 60_000)];
        assert_eq!(
            scheduler.select(10_000, true, false, &actions),
            Some("heavy".into())
        );
        assert_eq!(
            scheduler.select(70_000, true, false, &actions),
            Some("light".into())
        );
    }

    #[test]
    fn test_idle_action_respects_first_idle_delay() {
        let mut scheduler = IdleScheduler::new(500, FixedRng(VecDeque::from([0, 0])));
        assert_eq!(
            scheduler.select(10_499, true, false, &[action("idle", 1, 0)]),
            None
        );
        assert_eq!(
            scheduler.select(10_500, true, false, &[action("idle", 1, 0)]),
            Some("idle".into())
        );
    }

    #[test]
    fn test_idle_action_is_disabled_by_setting() {
        let mut scheduler = IdleScheduler::new(0, FixedRng(VecDeque::from([0, 0])));
        assert_eq!(
            scheduler.select(10_000, false, false, &[action("idle", 1, 0)]),
            None
        );
    }

    #[test]
    fn test_recent_three_actions_are_not_repeated() {
        let mut scheduler = IdleScheduler::new(0, FixedRng(VecDeque::from([0; 16])));
        let actions = [
            action("a", 1, 0),
            action("b", 1, 0),
            action("c", 1, 0),
            action("d", 1, 0),
        ];
        let a = scheduler.select(10_000, true, false, &actions).unwrap();
        let b = scheduler.select(70_000, true, false, &actions).unwrap();
        let c = scheduler.select(130_000, true, false, &actions).unwrap();
        let d = scheduler.select(190_000, true, false, &actions).unwrap();
        assert_eq!(
            [a.as_str(), b.as_str(), c.as_str(), d.as_str()],
            ["a", "b", "c", "d"]
        );
    }

    #[test]
    fn test_focus_and_dragging_block_idle_action() {
        let mut scheduler = IdleScheduler::new(0, FixedRng(VecDeque::from([0, 0])));
        assert_eq!(
            scheduler.select(10_000, true, true, &[action("idle", 1, 0)]),
            None
        );
    }

    #[test]
    fn test_idle_cannot_override_active_reminder() {
        let mut scheduler = Scheduler::default();
        scheduler.activate(
            &ScheduledAction {
                id: "reminder".into(),
                priority: Priority::ReminderAttention,
                not_before_ms: 0,
            },
            0,
        );
        assert!(!scheduler.accepts(&ScheduledAction {
            id: "idle".into(),
            priority: Priority::IdleRandom,
            not_before_ms: 0
        }));
        assert_eq!(scheduler.current().unwrap().id, "reminder");
    }
}
