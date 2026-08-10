use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PetState {
    #[default]
    Idle,
    Observing,
    WaitingForResponse,
    Playing,
    FocusCompanion,
    Dragging,
    Falling,
    Landing,
    OnEdge,
    Sleeping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mood {
    #[default]
    Calm,
    Curious,
    Playful,
    Startled,
    Focused,
    Sleepy,
    Happy,
}

#[derive(Debug, Clone, Default)]
pub struct PetMemory {
    pub last_event: Option<String>,
    pub last_action_id: Option<String>,
    pub recent_actions: VecDeque<String>,
    pub last_drag_direction: Option<i8>,
    pub session_interactions: u32,
}

impl PetMemory {
    pub fn record_action(&mut self, id: impl Into<String>) {
        let id = id.into();
        self.last_action_id = Some(id.clone());
        self.recent_actions.push_back(id);
        while self.recent_actions.len() > 8 {
            self.recent_actions.pop_front();
        }
    }
    pub fn can_repeat(&self, id: &str) -> bool {
        self.recent_actions.iter().rev().take(2).any(|x| x != id) || self.recent_actions.len() < 2
    }
}

#[derive(Debug, Clone)]
pub struct MoodState {
    pub mood: Mood,
    expires_at_ms: Option<u64>,
}
impl Default for MoodState {
    fn default() -> Self {
        Self {
            mood: Mood::Calm,
            expires_at_ms: None,
        }
    }
}
impl MoodState {
    pub fn set(&mut self, mood: Mood, now_ms: u64, duration_ms: u64) {
        self.mood = mood;
        self.expires_at_ms = Some(now_ms.saturating_add(duration_ms));
    }
    pub fn tick(&mut self, now_ms: u64) {
        if self.expires_at_ms.is_some_and(|x| now_ms >= x) {
            self.mood = Mood::Calm;
            self.expires_at_ms = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_mood_expires_and_returns_to_calm() {
        let mut m = MoodState::default();
        m.set(Mood::Happy, 10, 100);
        m.tick(109);
        assert_eq!(m.mood, Mood::Happy);
        m.tick(110);
        assert_eq!(m.mood, Mood::Calm)
    }
    #[test]
    fn test_recent_actions_prevent_three_time_repetition() {
        let mut m = PetMemory::default();
        m.record_action("idle");
        m.record_action("idle");
        assert!(!m.can_repeat("idle"));
        assert!(m.can_repeat("look"));
    }
}
