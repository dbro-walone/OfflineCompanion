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

#[derive(Debug, Default)]
pub struct Scheduler {
    current: Option<ScheduledAction>,
    last_idle_ms: Option<u64>,
}
impl Scheduler {
    pub fn submit(&mut self, action: ScheduledAction) -> bool {
        if self
            .current
            .as_ref()
            .is_none_or(|x| action.priority >= x.priority)
        {
            self.current = Some(action);
            true
        } else {
            false
        }
    }
    pub fn current(&self) -> Option<&ScheduledAction> {
        self.current.as_ref()
    }
    pub fn take(&mut self) -> Option<ScheduledAction> {
        self.current.take()
    }
    pub fn idle_due(&mut self, now_ms: u64, cooldown_ms: u64) -> bool {
        if self
            .last_idle_ms
            .is_some_and(|x| now_ms.saturating_sub(x) < cooldown_ms)
        {
            false
        } else {
            self.last_idle_ms = Some(now_ms);
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_idle_scheduler_respects_cooldown() {
        let mut s = Scheduler::default();
        assert!(s.idle_due(100, 1000));
        assert!(!s.idle_due(500, 1000));
        assert!(s.idle_due(1100, 1000));
    }
    #[test]
    fn test_priority_prevents_idle_from_overriding_reminder() {
        let mut s = Scheduler::default();
        s.submit(ScheduledAction {
            id: "reminder".into(),
            priority: Priority::ReminderAttention,
            not_before_ms: 0,
        });
        assert!(!s.submit(ScheduledAction {
            id: "idle".into(),
            priority: Priority::IdleRandom,
            not_before_ms: 0
        }));
        assert_eq!(s.current().unwrap().id, "reminder");
    }
}
