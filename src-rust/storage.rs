use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use directories::ProjectDirs;
use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::model::{AppSettings, PomodoroPhase, PomodoroState, Reminder, TodoItem};

pub struct AppPaths {
    pub root: PathBuf,
    pub database: PathBuf,
    pub settings: PathBuf,
    pub characters: PathBuf,
    pub actions: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let root = if cfg!(windows) {
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .context("无法确定 %LocalAppData% 目录")?
                .join("OfflineCompanion")
        } else {
            ProjectDirs::from("", "Local", "OfflineCompanion")
                .context("无法确定本地数据目录")?
                .data_local_dir()
                .to_path_buf()
        };
        let result = Self {
            database: root.join("data/companion.db"),
            settings: root.join("config/settings.json"),
            characters: root.join("packages/characters"),
            actions: root.join("packages/actions"),
            root,
        };
        for path in [
            result.database.parent().unwrap(),
            result.settings.parent().unwrap(),
            result.characters.as_path(),
            result.actions.as_path(),
            result.root.join("logs").as_path(),
            result.root.join("backups").as_path(),
        ] {
            fs::create_dir_all(path)?;
        }
        Ok(result)
    }
}

pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS todo_items (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                note TEXT,
                priority INTEGER NOT NULL DEFAULT 1,
                due_at TEXT,
                reminder_at TEXT,
                completed_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                estimated_pomodoros INTEGER NOT NULL DEFAULT 1,
                completed_pomodoros INTEGER NOT NULL DEFAULT 0,
                due_time TEXT
            );
            CREATE TABLE IF NOT EXISTS reminders (
                id TEXT PRIMARY KEY,
                todo_id TEXT,
                title TEXT NOT NULL,
                schedule_type INTEGER NOT NULL,
                local_time TEXT NOT NULL,
                weekdays TEXT NOT NULL,
                start_date TEXT,
                end_date TEXT,
                next_trigger_at TEXT NOT NULL,
                status INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY(todo_id) REFERENCES todo_items(id) ON DELETE CASCADE
            );
            CREATE TABLE IF NOT EXISTS pomodoro_sessions (
                id TEXT PRIMARY KEY,
                phase INTEGER NOT NULL,
                started_at TEXT NOT NULL,
                expected_end_at TEXT NOT NULL,
                paused_at TEXT,
                remaining_seconds INTEGER NOT NULL,
                completed_focus_rounds INTEGER NOT NULL,
                status INTEGER NOT NULL
            );",
        )?;
        Ok(Self { connection })
    }

    pub fn add_todo(
        &self,
        title: &str,
        due_at: Option<DateTime<Local>>,
        pomodoros: i32,
    ) -> Result<()> {
        let title = title.trim();
        anyhow::ensure!(!title.is_empty(), "待办标题不能为空");
        self.connection.execute(
            "INSERT INTO todo_items (id,title,note,priority,due_at,reminder_at,completed_at,created_at,updated_at,estimated_pomodoros,completed_pomodoros,due_time)
             VALUES (?1,?2,NULL,1,?3,NULL,NULL,?4,?4,?5,0,?3)",
            params![Uuid::new_v4().to_string(), title, due_at.map(|x| x.to_rfc3339()), Local::now().to_rfc3339(), pomodoros.clamp(1, 8)],
        )?;
        Ok(())
    }

    pub fn list_todos(&self, include_completed: bool) -> Result<Vec<TodoItem>> {
        let sql = if include_completed {
            "SELECT id,title,COALESCE(due_time,due_at),estimated_pomodoros,completed_at IS NOT NULL FROM todo_items ORDER BY completed_at IS NOT NULL,due_at IS NULL,due_at,created_at DESC"
        } else {
            "SELECT id,title,COALESCE(due_time,due_at),estimated_pomodoros,completed_at IS NOT NULL FROM todo_items WHERE completed_at IS NULL ORDER BY due_at IS NULL,due_at,created_at DESC"
        };
        let mut statement = self.connection.prepare(sql)?;
        let items = statement
            .query_map([], |row| {
                let due: Option<String> = row.get(2)?;
                Ok(TodoItem {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    due_at: due
                        .and_then(|x| DateTime::parse_from_rfc3339(&x).ok())
                        .map(|x| x.with_timezone(&Local)),
                    estimated_pomodoros: row.get(3)?,
                    completed: row.get::<_, i32>(4)? != 0,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(items)
    }

    pub fn toggle_todo(&self, id: &str) -> Result<bool> {
        self.connection.execute(
            "UPDATE todo_items SET completed_at = CASE WHEN completed_at IS NULL THEN ?2 ELSE NULL END, updated_at=?2 WHERE id=?1",
            params![id, Local::now().to_rfc3339()],
        )?;
        let completed: i32 = self.connection.query_row(
            "SELECT completed_at IS NOT NULL FROM todo_items WHERE id=?1",
            [id],
            |row| row.get(0),
        )?;
        Ok(completed != 0)
    }

    pub fn clear_completed(&self) -> Result<()> {
        self.connection
            .execute("DELETE FROM todo_items WHERE completed_at IS NOT NULL", [])?;
        Ok(())
    }

    pub fn add_reminder(&self, title: &str, trigger_at: DateTime<Local>) -> Result<()> {
        let title = title.trim();
        anyhow::ensure!(!title.is_empty(), "提醒标题不能为空");
        anyhow::ensure!(trigger_at > Local::now(), "提醒时间必须晚于当前时间");
        self.connection.execute(
            "INSERT INTO reminders (id,todo_id,title,schedule_type,local_time,weekdays,start_date,end_date,next_trigger_at,status,created_at)
             VALUES (?1,NULL,?2,0,?3,'[]',NULL,NULL,?4,0,?5)",
            params![
                Uuid::new_v4().to_string(),
                title,
                trigger_at.format("%H:%M:%S").to_string(),
                trigger_at.to_rfc3339(),
                Local::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn take_due_reminders(&self, now: DateTime<Local>) -> Result<Vec<Reminder>> {
        let cutoff = now - chrono::Duration::hours(24);
        let mut statement = self.connection.prepare(
            "SELECT id,title,next_trigger_at,status FROM reminders WHERE status=0 AND next_trigger_at<=?1 AND next_trigger_at>=?2 ORDER BY next_trigger_at"
        )?;
        let reminders = statement
            .query_map(params![now.to_rfc3339(), cutoff.to_rfc3339()], |row| {
                let raw: String = row.get(2)?;
                Ok(Reminder {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    trigger_at: DateTime::parse_from_rfc3339(&raw)
                        .unwrap()
                        .with_timezone(&Local),
                    fired: row.get::<_, i32>(3)? != 0,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for reminder in &reminders {
            self.connection
                .execute("UPDATE reminders SET status=1 WHERE id=?1", [&reminder.id])?;
        }
        Ok(reminders)
    }

    pub fn load_pomodoro(&self) -> Result<Option<PomodoroState>> {
        let mut statement = self.connection.prepare(
            "SELECT phase,expected_end_at,paused_at,remaining_seconds,status FROM pomodoro_sessions ORDER BY started_at DESC LIMIT 1"
        )?;
        let result = statement.query_row([], |row| {
            let phase = if row.get::<_, i32>(0)? == 0 {
                PomodoroPhase::Focus
            } else {
                PomodoroPhase::ShortBreak
            };
            let expected: String = row.get(1)?;
            let status: i32 = row.get(4)?;
            let mut remaining: i64 = row.get(3)?;
            if status == 0
                && let Ok(end) = DateTime::parse_from_rfc3339(&expected)
            {
                remaining = (end.with_timezone(&Local) - Local::now())
                    .num_seconds()
                    .max(0);
            }
            Ok(PomodoroState {
                phase,
                remaining_seconds: remaining,
                running: status == 0 || status == 1,
                paused: status == 1,
            })
        });
        match result {
            Ok(state) => Ok(Some(state)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn save_pomodoro(&self, state: &PomodoroState) -> Result<()> {
        let now = Local::now();
        let expected = now + chrono::Duration::seconds(state.remaining_seconds);
        let phase = if state.phase == PomodoroPhase::Focus {
            0
        } else {
            1
        };
        let status = if !state.running {
            3
        } else if state.paused {
            1
        } else {
            0
        };
        self.connection.execute(
            "INSERT INTO pomodoro_sessions (id,phase,started_at,expected_end_at,paused_at,remaining_seconds,completed_focus_rounds,status)
             VALUES ('rust-current',?1,?2,?3,?4,?5,0,?6)
             ON CONFLICT(id) DO UPDATE SET phase=excluded.phase,expected_end_at=excluded.expected_end_at,paused_at=excluded.paused_at,remaining_seconds=excluded.remaining_seconds,status=excluded.status",
            params![phase, now.to_rfc3339(), expected.to_rfc3339(), state.paused.then(|| now.to_rfc3339()), state.remaining_seconds, status],
        )?;
        Ok(())
    }
}

pub fn load_settings(path: &Path) -> AppSettings {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_settings(path: &Path, settings: &AppSettings) -> Result<()> {
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(settings)?)?;
    if path.exists() {
        let _ = fs::copy(path, path.with_extension("json.bak"));
        fs::remove_file(path)?;
    }
    fs::rename(temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_round_trip() {
        let store = Store {
            connection: Connection::open_in_memory().unwrap(),
        };
        store.connection.execute_batch(
            "CREATE TABLE todo_items (id TEXT PRIMARY KEY,title TEXT NOT NULL,note TEXT,priority INTEGER NOT NULL DEFAULT 1,due_at TEXT,reminder_at TEXT,completed_at TEXT,created_at TEXT NOT NULL,updated_at TEXT NOT NULL,estimated_pomodoros INTEGER NOT NULL,completed_pomodoros INTEGER NOT NULL DEFAULT 0,due_time TEXT);"
        ).unwrap();
        store.add_todo("测试任务", None, 2).unwrap();
        let items = store.list_todos(false).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "测试任务");
        assert!(store.toggle_todo(&items[0].id).unwrap());
        assert!(store.list_todos(false).unwrap().is_empty());
    }
}
