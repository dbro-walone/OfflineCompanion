use std::collections::BTreeMap;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub title: String,
    pub due_at: Option<DateTime<Local>>,
    pub estimated_pomodoros: i32,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
    pub id: String,
    pub title: String,
    pub trigger_at: DateTime<Local>,
    pub fired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub pet_left: Option<i32>,
    pub pet_top: Option<i32>,
    pub pet_scale: f32,
    pub topmost: bool,
    pub idle_actions_enabled: bool,
    pub reduce_motion: bool,
    pub theme: String,
    #[serde(rename = "sedentaryThresholdMinutes", alias = "sedentaryMinutes")]
    pub sedentary_minutes: u32,
    pub current_character_id: String,
    pub enabled_action_pack_ids: Vec<String>,
    pub pet_interaction_level: String,
    pub allow_pet_approach: bool,
    pub allow_mouse_follow: bool,
    pub allow_proactive_invitation: bool,
    pub interaction_cooldown_seconds: u64,
    pub pointer_near_distance_px: u32,
    pub reminder_follow_pet: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            pet_left: None,
            pet_top: None,
            pet_scale: 1.0,
            topmost: true,
            idle_actions_enabled: true,
            reduce_motion: false,
            theme: "dark".into(),
            sedentary_minutes: 60,
            current_character_id: "character.shadow-crow-ninja".into(),
            enabled_action_pack_ids: vec!["action.shadow-crow.office".into()],
            pet_interaction_level: "balanced".into(),
            allow_pet_approach: false,
            allow_mouse_follow: false,
            allow_proactive_invitation: true,
            interaction_cooldown_seconds: 30,
            pointer_near_distance_px: 120,
            reminder_follow_pet: true,
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PomodoroPhase {
    Focus,
    ShortBreak,
}

#[derive(Debug, Clone)]
pub struct PomodoroState {
    pub phase: PomodoroPhase,
    pub remaining_seconds: i64,
    pub running: bool,
    pub paused: bool,
    pub deadline_epoch_ms: Option<i64>,
    pub completion_emitted: bool,
}

impl Default for PomodoroState {
    fn default() -> Self {
        Self {
            phase: PomodoroPhase::Focus,
            remaining_seconds: 25 * 60,
            running: false,
            paused: false,
            deadline_epoch_ms: None,
            completion_emitted: false,
        }
    }
}

impl PomodoroState {
    pub fn start_at(&mut self, now_ms: i64) {
        self.running = true;
        self.paused = false;
        self.completion_emitted = false;
        self.deadline_epoch_ms = Some(now_ms + self.remaining_seconds * 1000);
    }
    pub fn pause_at(&mut self, now_ms: i64) {
        if self.running && !self.paused {
            self.remaining_seconds = self
                .deadline_epoch_ms
                .map(|x| ((x - now_ms) / 1000).max(0))
                .unwrap_or(self.remaining_seconds);
            self.paused = true;
            self.deadline_epoch_ms = None;
        }
    }
    pub fn resume_at(&mut self, now_ms: i64) {
        if self.running && self.paused {
            self.paused = false;
            self.deadline_epoch_ms = Some(now_ms + self.remaining_seconds * 1000);
        }
    }
    pub fn update_at(&mut self, now_ms: i64) -> bool {
        if !self.running || self.paused {
            return false;
        }
        self.remaining_seconds = self
            .deadline_epoch_ms
            .map(|x| ((x - now_ms + 999) / 1000).max(0))
            .unwrap_or(self.remaining_seconds);
        if self.remaining_seconds == 0 && !self.completion_emitted {
            self.completion_emitted = true;
            self.running = false;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_unknown_csharp_settings() {
        let json = r#"{"schemaVersion":1,"currentCharacterId":"character.custom","petScale":1.2,"sedentaryThresholdMinutes":90}"#;
        let settings: AppSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.pet_scale, 1.2);
        assert_eq!(settings.sedentary_minutes, 90);

        let saved = serde_json::to_value(settings).unwrap();
        assert_eq!(saved["currentCharacterId"], "character.custom");
        assert_eq!(saved["schemaVersion"], 1);
    }
    #[test]
    fn test_absolute_deadline_survives_sleep() {
        let mut p = PomodoroState {
            remaining_seconds: 600,
            ..Default::default()
        };
        p.start_at(1000);
        p.update_at(301_000);
        assert_eq!(p.remaining_seconds, 300);
        p.update_at(601_000);
        assert_eq!(p.remaining_seconds, 0);
    }
    #[test]
    fn test_pomodoro_completion_emits_once() {
        let mut p = PomodoroState {
            remaining_seconds: 1,
            ..Default::default()
        };
        p.start_at(0);
        assert!(p.update_at(1000));
        assert!(!p.update_at(2000));
    }
}
