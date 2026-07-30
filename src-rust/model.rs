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
    pub has_seen_exit_cry: bool,
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
            has_seen_exit_cry: false,
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
}

impl Default for PomodoroState {
    fn default() -> Self {
        Self {
            phase: PomodoroPhase::Focus,
            remaining_seconds: 25 * 60,
            running: false,
            paused: false,
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
    fn exit_cry_setting_is_backward_compatible_and_camel_case() {
        let settings: AppSettings = serde_json::from_str("{}").unwrap();
        assert!(!settings.has_seen_exit_cry);
        let saved = serde_json::to_value(settings).unwrap();
        assert_eq!(saved["hasSeenExitCry"], false);
    }
}
