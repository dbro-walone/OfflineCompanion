#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod model;
mod packages;
mod platform;
mod storage;

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use anyhow::{Context, Result};
use chrono::{Local, NaiveDateTime, TimeZone, Timelike};
use model::{AppSettings, PomodoroPhase, PomodoroState};
use slint::{ComponentHandle, ModelRc, PhysicalPosition, SharedString, Timer, TimerMode, VecModel};
use storage::{AppPaths, Store};

slint::include_modules!();

fn main() {
    if let Err(error) = run() {
        rfd::MessageDialog::new()
            .set_title("离线桌面陪伴助手")
            .set_description(format!("鸦影启动失败：{error:#}"))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}

fn run() -> Result<()> {
    let paths = Rc::new(AppPaths::discover()?);
    let store = Rc::new(Store::open(&paths.database)?);
    let settings = Rc::new(RefCell::new(storage::load_settings(&paths.settings)));
    let pomodoro = Rc::new(RefCell::new(store.load_pomodoro()?.unwrap_or_default()));
    let pomodoro_five_minute_alerted = Rc::new(Cell::new(false));
    let active_todo = Rc::new(RefCell::new(String::new()));
    let notification_message = Rc::new(RefCell::new(String::new()));

    let pet = PetWindow::new()?;
    let todos = TodoWindow::new()?;
    let reminder = ReminderWindow::new()?;
    let timer_window = TimerWindow::new()?;
    let settings_window = SettingsWindow::new()?;
    let package_window = PackageWindow::new()?;
    let notification = NotificationWindow::new()?;

    apply_theme(
        settings.borrow().theme == "light",
        &pet,
        &todos,
        &reminder,
        &timer_window,
        &settings_window,
        &package_window,
        &notification,
    );
    apply_settings_to_pet(&pet, &settings.borrow());
    restore_pet_position(&pet, &settings.borrow());
    refresh_todos(&todos, &store, false);
    initialize_todo_datetime_pickers(&todos);
    initialize_reminder_datetime_pickers(&reminder);

    wire_basic_windows(
        &pet,
        &todos,
        &reminder,
        &timer_window,
        &settings_window,
        &package_window,
    );
    wire_todos(
        &todos,
        &timer_window,
        store.clone(),
        pomodoro.clone(),
        pomodoro_five_minute_alerted.clone(),
        active_todo.clone(),
    );
    wire_reminders(&reminder, store.clone());
    update_timer_view(&timer_window, &pomodoro.borrow(), "");
    wire_timer(
        &timer_window,
        &pet,
        pomodoro.clone(),
        pomodoro_five_minute_alerted.clone(),
        active_todo.clone(),
        store.clone(),
    );
    wire_settings(
        &settings_window,
        &pet,
        &todos,
        &reminder,
        &timer_window,
        &package_window,
        &notification,
        settings.clone(),
        paths.clone(),
    );
    wire_packages(&package_window, paths.clone());
    wire_notification(&notification, notification_message.clone(), store.clone());
    wire_pet_drag(&pet, settings.clone(), paths.clone());

    let animation_sequence = Rc::new(RefCell::new(Vec::<i32>::new()));
    let animation_index = Rc::new(RefCell::new(0usize));
    {
        let sequences = animation_sequence.clone();
        let index = animation_index.clone();
        pet.on_pet_clicked(move || {
            *sequences.borrow_mut() = vec![4, 5, 3];
            *index.borrow_mut() = 0;
        });
    }

    let animation_timer = Timer::default();
    {
        let weak_pet = pet.as_weak();
        let sequences = animation_sequence.clone();
        let index = animation_index.clone();
        animation_timer.start(TimerMode::Repeated, Duration::from_millis(500), move || {
            let Some(pet) = weak_pet.upgrade() else {
                return;
            };
            let sequence = sequences.borrow();
            if sequence.is_empty() {
                return;
            }
            let mut position = index.borrow_mut();
            pet.set_frame_index(sequence[*position]);
            *position += 1;
            if *position >= sequence.len() {
                drop(position);
                drop(sequence);
                sequences.borrow_mut().clear();
                *index.borrow_mut() = 0;
            }
        });
    }

    let idle_animation_timer = Timer::default();
    {
        let weak_pet = pet.as_weak();
        let sequences = animation_sequence.clone();
        let settings = settings.clone();
        let mut idle_frame = 0i32;
        idle_animation_timer.start(TimerMode::Repeated, Duration::from_secs(20), move || {
            if !settings.borrow().idle_actions_enabled || !sequences.borrow().is_empty() {
                return;
            }
            if let Some(pet) = weak_pet.upgrade() {
                idle_frame = (idle_frame + 1) % 4;
                pet.set_frame_index(idle_frame);
            }
        });
    }

    let scheduler_timer = Timer::default();
    {
        let store = store.clone();
        let weak_pet = pet.as_weak();
        let weak_notification = notification.as_weak();
        let message = notification_message.clone();
        let sequence = animation_sequence.clone();
        let store = store.clone();
        scheduler_timer.start(TimerMode::Repeated, Duration::from_secs(1), move || {
            let Ok(due) = store.take_due_reminders(Local::now()) else {
                return;
            };
            for item in due {
                let Some(pet) = weak_pet.upgrade() else {
                    return;
                };
                let Some(notification) = weak_notification.upgrade() else {
                    return;
                };
                *message.borrow_mut() = item.title.clone();
                notification.set_message(item.title.into());
                *sequence.borrow_mut() = vec![6, 6, 6, 3];
                center_window_on_active_monitor(
                    pet.window(),
                    pet.window().size().width,
                    pet.window().size().height,
                );
                position_notification(&notification, &pet);
                let _ = pet.show();
                let _ = notification.show();
            }
        });
    }

    let pomodoro_timer = Timer::default();
    {
        let state = pomodoro.clone();
        let weak_timer = timer_window.as_weak();
        let weak_pet = pet.as_weak();
        let weak_notification = notification.as_weak();
        let message = notification_message.clone();
        let sequence = animation_sequence.clone();
        let five_minute_alerted = pomodoro_five_minute_alerted.clone();
        pomodoro_timer.start(TimerMode::Repeated, Duration::from_secs(1), move || {
            let mut state = state.borrow_mut();
            if !state.running || state.paused {
                return;
            }
            state.remaining_seconds = state.remaining_seconds.saturating_sub(1);
            if let Some(timer) = weak_timer.upgrade() {
                update_timer_view(&timer, &state, "");
            }
            if state.phase == PomodoroPhase::Focus
                && state.remaining_seconds == 5 * 60
                && !five_minute_alerted.replace(true)
            {
                let text = "当前番茄时钟还剩5分钟";
                *message.borrow_mut() = text.into();
                *sequence.borrow_mut() = vec![7, 7, 7, 3];
                if let (Some(notification), Some(pet)) =
                    (weak_notification.upgrade(), weak_pet.upgrade())
                {
                    notification.set_message(text.into());
                    center_window_on_active_monitor(
                        pet.window(),
                        pet.window().size().width,
                        pet.window().size().height,
                    );
                    position_notification(&notification, &pet);
                    let _ = pet.show();
                    let _ = notification.show();
                }
            }
            if state.remaining_seconds > 0 {
                return;
            }
            let text = if state.phase == PomodoroPhase::Focus {
                "专注完成，休息一下吧"
            } else {
                "休息结束，可以开始下一轮了"
            };
            state.running = false;
            state.phase = if state.phase == PomodoroPhase::Focus {
                PomodoroPhase::ShortBreak
            } else {
                PomodoroPhase::Focus
            };
            state.remaining_seconds = if state.phase == PomodoroPhase::Focus {
                25 * 60
            } else {
                5 * 60
            };
            five_minute_alerted.set(false);
            let _ = store.save_pomodoro(&state);
            *message.borrow_mut() = text.into();
            *sequence.borrow_mut() = vec![7, 7, 7, 3];
            if let (Some(notification), Some(pet)) =
                (weak_notification.upgrade(), weak_pet.upgrade())
            {
                notification.set_message(text.into());
                center_window_on_active_monitor(
                    pet.window(),
                    pet.window().size().width,
                    pet.window().size().height,
                );
                position_notification(&notification, &pet);
                let _ = notification.show();
            }
        });
    }

    let sedentary_timer = Timer::default();
    {
        let settings = settings.clone();
        let weak_pet = pet.as_weak();
        let weak_notification = notification.as_weak();
        let message = notification_message.clone();
        let active_seconds = Rc::new(RefCell::new(0u64));
        sedentary_timer.start(TimerMode::Repeated, Duration::from_secs(60), move || {
            let idle = platform::idle_millis();
            let mut active = active_seconds.borrow_mut();
            if idle >= 5 * 60 * 1000 {
                *active = 0;
                return;
            }
            *active += 60;
            if *active < settings.borrow().sedentary_minutes as u64 * 60 {
                return;
            }
            *active = 0;
            let text = "已经专注很久了，起来活动一下吧";
            *message.borrow_mut() = text.into();
            if let (Some(notification), Some(pet)) =
                (weak_notification.upgrade(), weak_pet.upgrade())
            {
                notification.set_message(text.into());
                center_window_on_active_monitor(
                    pet.window(),
                    pet.window().size().width,
                    pet.window().size().height,
                );
                position_notification(&notification, &pet);
                let _ = notification.show();
            }
        });
    }

    pet.on_exit(|| {
        let _ = slint::quit_event_loop();
    });
    pet.show()?;
    slint::run_event_loop()?;
    Ok(())
}

fn wire_basic_windows(
    pet: &PetWindow,
    todos: &TodoWindow,
    reminder: &ReminderWindow,
    timer: &TimerWindow,
    settings: &SettingsWindow,
    packages: &PackageWindow,
) {
    macro_rules! open_window {
        ($callback:ident, $window:expr, $offset_x:expr, $offset_y:expr, $prepare:expr) => {{
            let weak = $window.as_weak();
            pet.$callback(move || {
                if let Some(window) = weak.upgrade() {
                    ($prepare)(&window);
                    position_window_on_active_monitor(
                        window.window(),
                        window.window().size().width,
                        window.window().size().height,
                        $offset_x,
                        $offset_y,
                    );
                    let _ = window.show();
                }
            });
        }};
    }
    open_window!(
        on_open_todos,
        todos,
        -140,
        -80,
        initialize_todo_datetime_pickers
    );
    open_window!(
        on_open_reminder,
        reminder,
        -50,
        -20,
        initialize_reminder_datetime_pickers
    );
    open_window!(on_open_timer, timer, 50, 20, |_| {});
    open_window!(on_open_settings, settings, 120, 60, |_| {});
    open_window!(on_open_packages, packages, 180, -60, |_| {});

    let weak = todos.as_weak();
    todos.on_dismiss_window(move || {
        if let Some(w) = weak.upgrade() {
            let _ = w.hide();
        }
    });
    let weak = reminder.as_weak();
    reminder.on_dismiss_window(move || {
        if let Some(w) = weak.upgrade() {
            let _ = w.hide();
        }
    });
    let weak = timer.as_weak();
    timer.on_dismiss_window(move || {
        if let Some(w) = weak.upgrade() {
            let _ = w.hide();
        }
    });
    let weak = settings.as_weak();
    settings.on_dismiss_window(move || {
        if let Some(w) = weak.upgrade() {
            let _ = w.hide();
        }
    });
    let weak = packages.as_weak();
    packages.on_dismiss_window(move || {
        if let Some(w) = weak.upgrade() {
            let _ = w.hide();
        }
    });

    macro_rules! enable_native_drag {
        ($window:expr) => {{
            let weak = $window.as_weak();
            $window.on_begin_drag(move || {
                if let Some(window) = weak.upgrade() {
                    platform::begin_window_drag(window.window());
                }
            });
        }};
    }
    enable_native_drag!(todos);
    enable_native_drag!(reminder);
    enable_native_drag!(timer);
    enable_native_drag!(settings);
    enable_native_drag!(packages);
}

fn wire_todos(
    todo_window: &TodoWindow,
    timer: &TimerWindow,
    store: Rc<Store>,
    state: Rc<RefCell<PomodoroState>>,
    five_minute_alerted: Rc<Cell<bool>>,
    active_todo: Rc<RefCell<String>>,
) {
    {
        let weak = todo_window.as_weak();
        let store = store.clone();
        todo_window.on_add_todo(move |title, date, time, pomodoros| {
            let Some(window) = weak.upgrade() else { return };
            let result = parse_local_datetime(&date, &time)
                .and_then(|due| store.add_todo(&title, due, pomodoros));
            match result {
                Ok(()) => {
                    window.set_status_text("".into());
                    refresh_todos(&window, &store, window.get_include_completed());
                }
                Err(error) => window.set_status_text(error.to_string().into()),
            }
        });
    }
    {
        let weak = todo_window.as_weak();
        let store = store.clone();
        todo_window.on_toggle_todo(move |id| {
            if let Some(window) = weak.upgrade()
                && let Ok(completed) = store.toggle_todo(&id)
            {
                if completed {
                    window.set_status_text("完成啦，鸦影在为你庆祝！".into());
                }
                refresh_todos(&window, &store, window.get_include_completed());
            }
        });
    }
    {
        let weak = todo_window.as_weak();
        let store = store.clone();
        todo_window.on_clear_completed(move || {
            if let Some(window) = weak.upgrade()
                && store.clear_completed().is_ok()
            {
                refresh_todos(&window, &store, window.get_include_completed());
            }
        });
    }
    {
        let weak = todo_window.as_weak();
        let store = store.clone();
        todo_window.on_refresh(move |include| {
            if let Some(window) = weak.upgrade() {
                refresh_todos(&window, &store, include);
            }
        });
    }
    {
        let weak_timer = timer.as_weak();
        todo_window.on_start_focus(move |title| {
            *active_todo.borrow_mut() = title.to_string();
            five_minute_alerted.set(false);
            let mut state = state.borrow_mut();
            *state = PomodoroState {
                running: true,
                ..PomodoroState::default()
            };
            let _ = store.save_pomodoro(&state);
            if let Some(timer) = weak_timer.upgrade() {
                timer.set_active_todo(title);
                update_timer_view(&timer, &state, "");
                let _ = timer.hide();
            }
        });
    }
}

fn wire_reminders(window: &ReminderWindow, store: Rc<Store>) {
    let weak = window.as_weak();
    window.on_save_reminder(move |title, date, time| {
        let Some(window) = weak.upgrade() else { return };
        let result = parse_local_datetime(&date, &time)
            .and_then(|value| value.context("请输入有效日期和时间，例如 2026-07-29 09:30"))
            .and_then(|value| store.add_reminder(&title, value));
        match result {
            Ok(()) => {
                window.set_status_text("提醒已保存".into());
                let _ = window.hide();
            }
            Err(error) => window.set_status_text(error.to_string().into()),
        }
    });
}

fn wire_timer(
    window: &TimerWindow,
    pet: &PetWindow,
    state: Rc<RefCell<PomodoroState>>,
    five_minute_alerted: Rc<Cell<bool>>,
    active_todo: Rc<RefCell<String>>,
    store: Rc<Store>,
) {
    {
        let weak = window.as_weak();
        let state = state.clone();
        let active_todo = active_todo.clone();
        let weak_pet = pet.as_weak();
        let store = store.clone();
        let five_minute_alerted = five_minute_alerted.clone();
        window.on_start(move || {
            let mut state = state.borrow_mut();
            state.running = true;
            state.paused = false;
            five_minute_alerted.set(false);
            let _ = store.save_pomodoro(&state);
            if let Some(window) = weak.upgrade() {
                update_timer_view(&window, &state, &active_todo.borrow());
                let _ = window.hide();
            }
            if let Some(pet) = weak_pet.upgrade() {
                pet.set_frame_index(1);
            }
        });
    }
    {
        let weak = window.as_weak();
        let state = state.clone();
        let active_todo = active_todo.clone();
        let store = store.clone();
        window.on_toggle_pause(move || {
            let mut state = state.borrow_mut();
            if state.running {
                state.paused = !state.paused;
            }
            let _ = store.save_pomodoro(&state);
            if let Some(window) = weak.upgrade() {
                update_timer_view(&window, &state, &active_todo.borrow());
            }
        });
    }
    {
        let weak = window.as_weak();
        let five_minute_alerted = five_minute_alerted.clone();
        window.on_stop(move || {
            *state.borrow_mut() = PomodoroState::default();
            five_minute_alerted.set(false);
            let _ = store.save_pomodoro(&state.borrow());
            active_todo.borrow_mut().clear();
            if let Some(window) = weak.upgrade() {
                window.set_active_todo("".into());
                update_timer_view(&window, &state.borrow(), "");
            }
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn wire_settings(
    window: &SettingsWindow,
    pet: &PetWindow,
    todos: &TodoWindow,
    reminder: &ReminderWindow,
    timer: &TimerWindow,
    packages: &PackageWindow,
    notification: &NotificationWindow,
    settings: Rc<RefCell<AppSettings>>,
    paths: Rc<AppPaths>,
) {
    let initial = settings.borrow().clone();
    window.set_scale_value(initial.pet_scale);
    window.set_topmost_value(initial.topmost);
    window.set_idle_value(initial.idle_actions_enabled);
    window.set_reduce_motion_value(initial.reduce_motion);
    window.set_theme_index(if initial.theme == "light" { 1 } else { 0 });
    window.set_sedentary_minutes(initial.sedentary_minutes as i32);
    let weak_window = window.as_weak();
    let weak_pet = pet.as_weak();
    let weak_todos = todos.as_weak();
    let weak_reminder = reminder.as_weak();
    let weak_timer = timer.as_weak();
    let weak_packages = packages.as_weak();
    let weak_notification = notification.as_weak();
    window.on_save(
        move |scale, topmost, idle, reduce_motion, theme_index, sedentary| {
            let mut value = settings.borrow_mut();
            value.pet_scale = scale.clamp(0.75, 1.4);
            value.topmost = topmost;
            value.idle_actions_enabled = idle;
            value.reduce_motion = reduce_motion;
            value.theme = if theme_index == 1 { "light" } else { "dark" }.into();
            value.sedentary_minutes = sedentary.clamp(30, 120) as u32;
            let _ = storage::save_settings(&paths.settings, &value);
            if let Some(pet) = weak_pet.upgrade() {
                apply_settings_to_pet(&pet, &value);
                pet.global::<Theme>().set_light(theme_index == 1);
            }
            if let Some(component) = weak_todos.upgrade() {
                component.global::<Theme>().set_light(theme_index == 1);
            }
            if let Some(component) = weak_reminder.upgrade() {
                component.global::<Theme>().set_light(theme_index == 1);
            }
            if let Some(component) = weak_timer.upgrade() {
                component.global::<Theme>().set_light(theme_index == 1);
            }
            if let Some(component) = weak_packages.upgrade() {
                component.global::<Theme>().set_light(theme_index == 1);
            }
            if let Some(component) = weak_notification.upgrade() {
                component.global::<Theme>().set_light(theme_index == 1);
            }
            if let Some(component) = weak_window.upgrade() {
                component.global::<Theme>().set_light(theme_index == 1);
            }
            if let Some(window) = weak_window.upgrade() {
                let _ = window.hide();
            }
        },
    );
}

fn wire_packages(window: &PackageWindow, paths: Rc<AppPaths>) {
    let weak = window.as_weak();
    window.on_import_package(move || {
        let Some(window) = weak.upgrade() else { return };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("扩展包", &["zip"])
            .pick_file()
        else {
            return;
        };
        match packages::install_package(&path, &paths.characters, &paths.actions) {
            Ok(status) => window.set_status_text(status.into()),
            Err(error) => window.set_status_text(format!("导入失败：{error}").into()),
        }
    });
}

fn wire_notification(window: &NotificationWindow, message: Rc<RefCell<String>>, store: Rc<Store>) {
    let weak = window.as_weak();
    window.on_dismiss(move || {
        if let Some(window) = weak.upgrade() {
            let _ = window.hide();
        }
    });
    let weak = window.as_weak();
    window.on_snooze(move || {
        let _ = store.add_reminder(
            &message.borrow(),
            Local::now() + chrono::Duration::minutes(10),
        );
        if let Some(window) = weak.upgrade() {
            let _ = window.hide();
        }
    });
}

fn wire_pet_drag(pet: &PetWindow, settings: Rc<RefCell<AppSettings>>, paths: Rc<AppPaths>) {
    #[derive(Default)]
    struct Drag {
        origin: Option<(i32, i32)>,
        pointer: (f32, f32),
        moved: bool,
    }
    let drag = Rc::new(RefCell::new(Drag::default()));
    {
        let weak = pet.as_weak();
        let drag = drag.clone();
        pet.on_drag_start(move |x, y| {
            if let Some(pet) = weak.upgrade() {
                let position = pet.window().position();
                *drag.borrow_mut() = Drag {
                    origin: Some((position.x, position.y)),
                    pointer: (x, y),
                    moved: false,
                };
            }
        });
    }
    {
        let weak = pet.as_weak();
        let drag = drag.clone();
        pet.on_drag_move(move |x, y| {
            let mut drag = drag.borrow_mut();
            let Some((left, top)) = drag.origin else {
                return;
            };
            if (x - drag.pointer.0).abs() + (y - drag.pointer.1).abs() < 4.0 {
                return;
            }
            drag.moved = true;
            if let Some(pet) = weak.upgrade() {
                pet.window().set_position(PhysicalPosition::new(
                    left + (x - drag.pointer.0) as i32,
                    top + (y - drag.pointer.1) as i32,
                ));
            }
        });
    }
    {
        let weak = pet.as_weak();
        pet.on_drag_end(move || {
            let moved = std::mem::take(&mut drag.borrow_mut().moved);
            drag.borrow_mut().origin = None;
            if !moved {
                return;
            }
            if let Some(pet) = weak.upgrade() {
                let position = pet.window().position();
                let mut value = settings.borrow_mut();
                value.pet_left = Some(position.x);
                value.pet_top = Some(position.y);
                let _ = storage::save_settings(&paths.settings, &value);
            }
        });
    }
}

fn refresh_todos(window: &TodoWindow, store: &Store, include_completed: bool) {
    let rows = store
        .list_todos(include_completed)
        .unwrap_or_default()
        .into_iter()
        .map(|item| TodoRow {
            id: item.id.into(),
            title: item.title.into(),
            completed: item.completed,
            meta: item
                .due_at
                .map(|x| {
                    format!(
                        "到期 {} · {} 个番茄",
                        x.format("%Y-%m-%d %H:%M"),
                        item.estimated_pomodoros
                    )
                })
                .unwrap_or_else(|| format!("未设置截止时间 · {} 个番茄", item.estimated_pomodoros))
                .into(),
        })
        .collect::<Vec<_>>();
    window.set_todos(ModelRc::from(Rc::new(VecModel::from(rows))));
}

fn initialize_todo_datetime_pickers(window: &TodoWindow) {
    let (dates, times, date_index, time_index) = datetime_picker_values();
    let mut optional_dates = Vec::with_capacity(dates.len() + 1);
    optional_dates.push(SharedString::from("不设置截止日期"));
    optional_dates.extend(dates);
    window.set_date_options(ModelRc::from(Rc::new(VecModel::from(optional_dates))));
    window.set_time_options(ModelRc::from(Rc::new(VecModel::from(times))));
    window.set_date_index(date_index + 1);
    window.set_time_index(time_index);
}

fn initialize_reminder_datetime_pickers(window: &ReminderWindow) {
    let (dates, times, date_index, time_index) = datetime_picker_values();
    window.set_date_options(ModelRc::from(Rc::new(VecModel::from(dates))));
    window.set_time_options(ModelRc::from(Rc::new(VecModel::from(times))));
    window.set_date_index(date_index);
    window.set_time_index(time_index);
}

fn datetime_picker_values() -> (Vec<SharedString>, Vec<SharedString>, i32, i32) {
    let now = Local::now();
    let target = now + chrono::Duration::minutes(10);
    let rounded_slot = (target.num_seconds_from_midnight() + 15 * 60 - 1) / (15 * 60);
    let rounded_day_offset = i64::from(rounded_slot / (24 * 4));
    let target_day_offset = (target.date_naive() - now.date_naive()).num_days();
    let date_index = (target_day_offset + rounded_day_offset).clamp(0, 29) as i32;
    let time_index = (rounded_slot % (24 * 4)) as i32;

    let dates = (0..30)
        .map(|day| {
            (now.date_naive() + chrono::Duration::days(day))
                .format("%Y-%m-%d")
                .to_string()
                .into()
        })
        .collect();
    let times = (0..24 * 4)
        .map(|slot| {
            let minutes = slot * 15;
            format!("{:02}:{:02}", minutes / 60, minutes % 60).into()
        })
        .collect();
    (dates, times, date_index, time_index)
}

fn parse_local_datetime(date: &str, time: &str) -> Result<Option<chrono::DateTime<Local>>> {
    if date.trim().is_empty() && time.trim().is_empty() {
        return Ok(None);
    }
    let raw = format!("{} {}", date.trim(), time.trim());
    let naive =
        NaiveDateTime::parse_from_str(&raw, "%Y-%m-%d %H:%M").context("日期或时间格式无效")?;
    Ok(Local.from_local_datetime(&naive).single())
}

fn update_timer_view(window: &TimerWindow, state: &PomodoroState, todo: &str) {
    let minutes = state.remaining_seconds / 60;
    let seconds = state.remaining_seconds % 60;
    window.set_time_text(format!("{minutes:02}:{seconds:02}").into());
    let phase = match (state.phase, state.paused) {
        (_, true) => "已暂停",
        (PomodoroPhase::Focus, false) => "专注",
        (PomodoroPhase::ShortBreak, false) => "短休息",
    };
    window.set_phase_text(phase.into());
    if !todo.is_empty() {
        window.set_active_todo(todo.into());
    }
}

fn apply_settings_to_pet(pet: &PetWindow, settings: &AppSettings) {
    pet.set_pet_scale(settings.pet_scale);
    pet.set_topmost_enabled(settings.topmost);
    pet.set_reduce_motion(settings.reduce_motion);
}

#[allow(clippy::too_many_arguments)]
fn apply_theme(
    light: bool,
    pet: &PetWindow,
    todos: &TodoWindow,
    reminder: &ReminderWindow,
    timer: &TimerWindow,
    settings: &SettingsWindow,
    packages: &PackageWindow,
    notification: &NotificationWindow,
) {
    pet.global::<Theme>().set_light(light);
    todos.global::<Theme>().set_light(light);
    reminder.global::<Theme>().set_light(light);
    timer.global::<Theme>().set_light(light);
    settings.global::<Theme>().set_light(light);
    packages.global::<Theme>().set_light(light);
    notification.global::<Theme>().set_light(light);
}

fn restore_pet_position(pet: &PetWindow, settings: &AppSettings) {
    if let (Some(x), Some(y)) = (settings.pet_left, settings.pet_top) {
        pet.window().set_position(PhysicalPosition::new(x, y));
    } else {
        let area = platform::active_work_area();
        pet.window()
            .set_position(PhysicalPosition::new(area.right - 290, area.bottom - 370));
    }
}

fn center_window_on_active_monitor(window: &slint::Window, width: u32, height: u32) {
    position_window_on_active_monitor(window, width, height, 0, 0);
}

fn position_window_on_active_monitor(
    window: &slint::Window,
    width: u32,
    height: u32,
    offset_x: i32,
    offset_y: i32,
) {
    let area = platform::active_work_area();
    let (x, y) = area.center(width, height);
    let max_x = (area.right - width as i32).max(area.left);
    let max_y = (area.bottom - height as i32).max(area.top);
    window.set_position(PhysicalPosition::new(
        (x + offset_x).clamp(area.left, max_x),
        (y + offset_y).clamp(area.top, max_y),
    ));
}

fn position_notification(notification: &NotificationWindow, pet: &PetWindow) {
    let pet_position = pet.window().position();
    let area = platform::active_work_area();
    let width = notification.window().size().width as i32;
    let height = notification.window().size().height as i32;
    let x = (pet_position.x - width - 12).clamp(area.left, area.right - width);
    let y = (pet_position.y + pet.window().size().height as i32 - height)
        .clamp(area.top, area.bottom - height);
    notification
        .window()
        .set_position(PhysicalPosition::new(x, y));
}
