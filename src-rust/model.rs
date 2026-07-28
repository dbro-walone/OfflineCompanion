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
pub struct AppSettings {
    pub pet_left: Option<i32>,
    pub pet_top: Option<i32>,
    pub pet_scale: f32,
    pub topmost: bool,
    pub idle_actions_enabled: bool,
    pub reduce_motion: bool,
    pub theme: String,
    pub sedentary_minutes: u32,
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
