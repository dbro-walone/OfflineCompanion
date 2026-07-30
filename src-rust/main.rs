#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod model;
mod packages;
mod platform;
mod storage;

use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use chrono::{Local, NaiveDateTime, TimeZone, Timelike};
use model::{AppSettings, PomodoroPhase, PomodoroState};
use slint::{ComponentHandle, ModelRc, PhysicalPosition, Timer, TimerMode, VecModel};
use storage::{AppPaths, Store};

slint::include_modules!();

struct PetMotion {
    start_x: i32,
    start_y: i32,
    target_x: i32,
    target_y: i32,
    started_at: Instant,
    duration: Duration,
    message: String,
}

type AlertPresenter = Rc<dyn Fn(String)>;
type CancelMotion = Rc<dyn Fn()>;

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
    let active_todo = Rc::new(RefCell::new(String::new()));
    let notification_message = Rc::new(RefCell::new(String::new()));
    let pomodoro_five_minute_notified = Rc::new(RefCell::new(false));
    let suppress_next_pet_click = Rc::new(RefCell::new(false));

    let pet = PetWindow::new()?;
    let todos = TodoWindow::new()?;
    let reminder = ReminderWindow::new()?;
    let timer_window = TimerWindow::new()?;
    let settings_window = SettingsWindow::new()?;
    let package_window = PackageWindow::new()?;
    let notification = NotificationWindow::new()?;
    let menu = MenuWindow::new()?;

    apply_theme(
        settings.borrow().theme == "light",
        &pet,
        &menu,
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

    wire_basic_windows(
        &pet,
        &menu,
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
        active_todo.clone(),
        pomodoro_five_minute_notified.clone(),
    );
    configure_reminder_defaults(&reminder);
    wire_reminders(&reminder, store.clone());
    update_timer_view(&timer_window, &pomodoro.borrow(), "");
    wire_timer(
        &timer_window,
        &pet,
        pomodoro.clone(),
        active_todo.clone(),
        store.clone(),
        pomodoro_five_minute_notified.clone(),
    );
    wire_settings(
        &settings_window,
        &pet,
        &todos,
        &reminder,
        &timer_window,
        &package_window,
        &menu,
        &notification,
        settings.clone(),
        paths.clone(),
    );
    wire_packages(&package_window, paths.clone());
    wire_notification(&notification, notification_message.clone(), store.clone());
    let animation_sequence = Rc::new(RefCell::new(Vec::<i32>::new()));
    let animation_index = Rc::new(RefCell::new(0usize));
    {
        let sequences = animation_sequence.clone();
        let index = animation_index.clone();
        let suppress_next_pet_click = suppress_next_pet_click.clone();
        pet.on_pet_clicked(move || {
            if std::mem::take(&mut *suppress_next_pet_click.borrow_mut()) {
                return;
            }
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

    let (present_alert, cancel_pet_motion) = create_alert_presenter(
        &pet,
        &notification,
        settings.clone(),
        notification_message.clone(),
        animation_sequence.clone(),
    );
    wire_pet_drag(
        &pet,
        settings.clone(),
        paths.clone(),
        suppress_next_pet_click.clone(),
        cancel_pet_motion,
    );

    let scheduler_timer = Timer::default();
    {
        let store = store.clone();
        let present_alert = present_alert.clone();
        let weak_reminder = reminder.as_weak();
        scheduler_timer.start(TimerMode::Repeated, Duration::from_secs(1), move || {
            let Ok(due) = store.take_due_reminders(Local::now()) else {
                return;
            };
            if due.is_empty() {
                return;
            }
            let titles = due
                .iter()
                .map(|item| item.title.clone())
                .collect::<Vec<_>>();
            let Some(text) = format_reminder_alert(&titles) else {
                return;
            };
            if let Some(window) = weak_reminder.upgrade() {
                refresh_reminders(&window, &store);
            }
            present_alert(text);
        });
    }

    let pomodoro_timer = Timer::default();
    {
        let state = pomodoro.clone();
        let weak_timer = timer_window.as_weak();
        let five_minute_notified = pomodoro_five_minute_notified.clone();
        let present_alert = present_alert.clone();
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
                && state.remaining_seconds > 0
                && state.remaining_seconds <= 5 * 60
                && !*five_minute_notified.borrow()
            {
                *five_minute_notified.borrow_mut() = true;
                let text = "当前番茄时钟还剩5分钟";
                present_alert(text.into());
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
            *five_minute_notified.borrow_mut() = false;
            let _ = store.save_pomodoro(&state);
            present_alert(text.into());
        });
    }

    let sedentary_timer = Timer::default();
    {
        let settings = settings.clone();
        let present_alert = present_alert.clone();
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
            present_alert(text.into());
        });
    }

    wire_menu(&menu, &pet);
    let menu_dismiss_timer = Timer::default();
    {
        let weak_menu = menu.as_weak();
        let weak_pet = pet.as_weak();
        let menu_was_visible = Rc::new(RefCell::new(false));
        menu_dismiss_timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
            let (Some(menu), Some(pet)) = (weak_menu.upgrade(), weak_pet.upgrade()) else {
                return;
            };
            if !pet.get_menu_visible() {
                *menu_was_visible.borrow_mut() = false;
                return;
            }
            if !*menu_was_visible.borrow() {
                *menu_was_visible.borrow_mut() = true;
                return;
            }
            if platform::window_has_focus(menu.window()) {
                return;
            }
            pet.set_menu_visible(false);
            *menu_was_visible.borrow_mut() = false;
            let _ = menu.hide();
        });
    }

    let weak_menu = menu.as_weak();
    pet.on_exit(move || {
        if let Some(menu) = weak_menu.upgrade() {
            let _ = menu.hide();
        }
        let _ = slint::quit_event_loop();
    });
    pet.show()?;
    slint::run_event_loop()?;
    Ok(())
}

fn wire_basic_windows(
    pet: &PetWindow,
    menu: &MenuWindow,
    todos: &TodoWindow,
    reminder: &ReminderWindow,
    timer: &TimerWindow,
    settings: &SettingsWindow,
    packages: &PackageWindow,
) {
    macro_rules! open_window {
        ($callback:ident, $window:expr) => {{
            let weak = $window.as_weak();
            let weak_pet = pet.as_weak();
            pet.$callback(move || {
                if let (Some(window), Some(pet)) = (weak.upgrade(), weak_pet.upgrade()) {
                    center_window_on_active_monitor(
                        window.window(),
                        pet.window(),
                        window.window().size().width,
                        window.window().size().height,
                    );
                    let _ = window.show();
                }
            });
        }};
    }
    open_window!(on_open_todos, todos);
    open_window!(on_open_timer, timer);
    open_window!(on_open_settings, settings);
    open_window!(on_open_packages, packages);

    let weak = reminder.as_weak();
    let weak_pet = pet.as_weak();
    pet.on_open_reminder(move || {
        if let (Some(window), Some(pet)) = (weak.upgrade(), weak_pet.upgrade()) {
            configure_reminder_defaults(&window);
            window.invoke_refresh_reminders();
            center_window_on_active_monitor(
                window.window(),
                pet.window(),
                window.window().size().width,
                window.window().size().height,
            );
            let _ = window.show();
        }
    });

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
                    let _ = platform::begin_window_drag(window.window());
                }
            });
        }};
    }
    enable_native_drag!(todos);
    enable_native_drag!(reminder);
    enable_native_drag!(timer);
    enable_native_drag!(settings);
    enable_native_drag!(packages);

    macro_rules! forward_menu_action {
        ($menu_callback:ident, $pet_callback:ident) => {{
            let weak_menu = menu.as_weak();
            let weak_pet = pet.as_weak();
            menu.$menu_callback(move || {
                if let Some(menu) = weak_menu.upgrade() {
                    let _ = menu.hide();
                }
                if let Some(pet) = weak_pet.upgrade() {
                    pet.set_menu_visible(false);
                    pet.$pet_callback();
                }
            });
        }};
    }
    forward_menu_action!(on_open_todos, invoke_open_todos);
    forward_menu_action!(on_open_reminder, invoke_open_reminder);
    forward_menu_action!(on_open_timer, invoke_open_timer);
    forward_menu_action!(on_open_packages, invoke_open_packages);
    forward_menu_action!(on_open_settings, invoke_open_settings);
}

fn wire_menu(menu: &MenuWindow, pet: &PetWindow) {
    let weak_menu = menu.as_weak();
    let weak_pet = pet.as_weak();
    pet.on_open_menu(move || {
        if let (Some(menu), Some(pet)) = (weak_menu.upgrade(), weak_pet.upgrade()) {
            position_menu(&menu, &pet);
            if menu.show().is_ok() {
                platform::focus_window(menu.window());
            }
        }
    });

    let weak_menu = menu.as_weak();
    pet.on_dismiss_menu(move || {
        if let Some(menu) = weak_menu.upgrade() {
            let _ = menu.hide();
        }
    });

    let weak_menu = menu.as_weak();
    let weak_pet = pet.as_weak();
    menu.on_dismiss_menu(move || {
        if let Some(menu) = weak_menu.upgrade() {
            let _ = menu.hide();
        }
        if let Some(pet) = weak_pet.upgrade() {
            pet.set_menu_visible(false);
        }
    });

    let weak_pet = pet.as_weak();
    menu.on_exit(move || {
        if let Some(pet) = weak_pet.upgrade() {
            pet.set_menu_visible(false);
            pet.invoke_exit();
        }
    });
}

fn wire_todos(
    todo_window: &TodoWindow,
    timer: &TimerWindow,
    store: Rc<Store>,
    state: Rc<RefCell<PomodoroState>>,
    active_todo: Rc<RefCell<String>>,
    five_minute_notified: Rc<RefCell<bool>>,
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
        let weak_todos = todo_window.as_weak();
        todo_window.on_start_focus(move |title| {
            *active_todo.borrow_mut() = title.to_string();
            *five_minute_notified.borrow_mut() = false;
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
            if let Some(todos) = weak_todos.upgrade() {
                let _ = todos.hide();
            }
        });
    }
}

fn wire_reminders(window: &ReminderWindow, store: Rc<Store>) {
    refresh_reminders(window, &store);
    {
        let weak = window.as_weak();
        let store = store.clone();
        window.on_save_reminder(move |title, date_index, hour_index, minute_index| {
            let Some(window) = weak.upgrade() else { return };
            let result = reminder_datetime(date_index, hour_index, minute_index)
                .and_then(|value| store.add_reminder(&title, value));
            match result {
                Ok(()) => {
                    refresh_reminders(&window, &store);
                    configure_reminder_defaults(&window);
                    window.set_status_text("提醒已添加，可以继续创建".into());
                }
                Err(error) => window.set_status_text(error.to_string().into()),
            }
        });
    }
    {
        let weak = window.as_weak();
        let store = store.clone();
        window.on_delete_reminder(move |id| {
            let Some(window) = weak.upgrade() else { return };
            match store.delete_reminder(&id) {
                Ok(()) => {
                    refresh_reminders(&window, &store);
                    window.set_status_text("提醒已删除".into());
                }
                Err(error) => window.set_status_text(error.to_string().into()),
            }
        });
    }
    {
        let weak = window.as_weak();
        window.on_refresh_reminders(move || {
            if let Some(window) = weak.upgrade() {
                refresh_reminders(&window, &store);
            }
        });
    }
}

fn wire_timer(
    window: &TimerWindow,
    pet: &PetWindow,
    state: Rc<RefCell<PomodoroState>>,
    active_todo: Rc<RefCell<String>>,
    store: Rc<Store>,
    five_minute_notified: Rc<RefCell<bool>>,
) {
    {
        let weak = window.as_weak();
        let state = state.clone();
        let active_todo = active_todo.clone();
        let weak_pet = pet.as_weak();
        let store = store.clone();
        let five_minute_notified = five_minute_notified.clone();
        window.on_start(move || {
            *five_minute_notified.borrow_mut() = false;
            let mut state = state.borrow_mut();
            state.running = true;
            state.paused = false;
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
        let five_minute_notified = five_minute_notified.clone();
        window.on_stop(move || {
            *five_minute_notified.borrow_mut() = false;
            *state.borrow_mut() = PomodoroState::default();
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
    menu: &MenuWindow,
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
    let weak_menu = menu.as_weak();
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
            if let Some(component) = weak_menu.upgrade() {
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

fn wire_pet_drag(
    pet: &PetWindow,
    settings: Rc<RefCell<AppSettings>>,
    paths: Rc<AppPaths>,
    suppress_next_click: Rc<RefCell<bool>>,
    cancel_motion: CancelMotion,
) {
    let drag_origin = Rc::new(RefCell::new(None::<(i32, i32)>));
    {
        let weak = pet.as_weak();
        let drag_origin = drag_origin.clone();
        pet.on_drag_start(move |_, _| {
            cancel_motion();
            let Some(pet) = weak.upgrade() else { return };
            let position = pet.window().position();
            *drag_origin.borrow_mut() = Some((position.x, position.y));
            if !platform::begin_window_drag(pet.window()) {
                drag_origin.borrow_mut().take();
            }
        });
    }
    {
        let weak = pet.as_weak();
        pet.on_drag_end(move || {
            let Some((origin_x, origin_y)) = drag_origin.borrow_mut().take() else {
                return;
            };
            let Some(pet) = weak.upgrade() else { return };
            let position = pet.window().position();
            if position.x == origin_x && position.y == origin_y {
                return;
            }
            *suppress_next_click.borrow_mut() = true;
            let mut value = settings.borrow_mut();
            value.pet_left = Some(position.x);
            value.pet_top = Some(position.y);
            let _ = storage::save_settings(&paths.settings, &value);
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

fn refresh_reminders(window: &ReminderWindow, store: &Store) {
    let rows = store
        .list_pending_reminders()
        .unwrap_or_default()
        .into_iter()
        .map(|item| ReminderRow {
            id: item.id.into(),
            title: item.title.into(),
            time: item.trigger_at.format("%Y-%m-%d %H:%M").to_string().into(),
        })
        .collect::<Vec<_>>();
    window.set_reminder_count(rows.len() as i32);
    window.set_reminders(ModelRc::from(Rc::new(VecModel::from(rows))));
}

fn configure_reminder_defaults(window: &ReminderWindow) {
    let now = Local::now();
    let default_time = now + chrono::Duration::minutes(10);
    let today = now.date_naive();
    let dates: Vec<slint::SharedString> = (0..31)
        .map(|offset| {
            let date = today + chrono::Duration::days(offset);
            let suffix = match offset {
                0 => "（今天）",
                1 => "（明天）",
                _ => "",
            };
            format!("{}{suffix}", date.format("%Y-%m-%d")).into()
        })
        .collect();
    let hours: Vec<slint::SharedString> =
        (0..24).map(|hour| format!("{hour:02} 时").into()).collect();
    let minutes: Vec<slint::SharedString> = (0..60)
        .map(|minute| format!("{minute:02} 分").into())
        .collect();

    window.set_date_options(ModelRc::from(Rc::new(VecModel::from(dates))));
    window.set_hour_options(ModelRc::from(Rc::new(VecModel::from(hours))));
    window.set_minute_options(ModelRc::from(Rc::new(VecModel::from(minutes))));
    window.set_date_index(
        default_time
            .date_naive()
            .signed_duration_since(today)
            .num_days() as i32,
    );
    window.set_hour_index(default_time.hour() as i32);
    window.set_minute_index(default_time.minute() as i32);
    window.set_status_text("".into());
}

fn reminder_datetime(
    date_index: i32,
    hour_index: i32,
    minute_index: i32,
) -> Result<chrono::DateTime<Local>> {
    anyhow::ensure!((0..31).contains(&date_index), "请选择有效日期");
    anyhow::ensure!((0..24).contains(&hour_index), "请选择有效小时");
    anyhow::ensure!((0..60).contains(&minute_index), "请选择有效分钟");
    let date = Local::now().date_naive() + chrono::Duration::days(date_index as i64);
    let naive = date
        .and_hms_opt(hour_index as u32, minute_index as u32, 0)
        .context("请选择有效日期和时间")?;
    Local
        .from_local_datetime(&naive)
        .single()
        .context("所选时间在当前时区中无效")
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
    menu: &MenuWindow,
    todos: &TodoWindow,
    reminder: &ReminderWindow,
    timer: &TimerWindow,
    settings: &SettingsWindow,
    packages: &PackageWindow,
    notification: &NotificationWindow,
) {
    pet.global::<Theme>().set_light(light);
    menu.global::<Theme>().set_light(light);
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
        let area = platform::active_work_area(pet.window());
        pet.window()
            .set_position(PhysicalPosition::new(area.right - 290, area.bottom - 370));
    }
}

fn center_window_on_active_monitor(
    window: &slint::Window,
    reference: &slint::Window,
    width: u32,
    height: u32,
) {
    let (x, y) = platform::active_work_area(reference).center(width, height);
    window.set_position(PhysicalPosition::new(x, y));
}

fn format_reminder_alert(titles: &[String]) -> Option<String> {
    match titles {
        [] => None,
        [title] => Some(format!("时间到了：{title}")),
        _ => Some(format!(
            "有 {} 个提醒到时间了：\n{}",
            titles.len(),
            titles
                .iter()
                .map(|title| format!("• {title}"))
                .collect::<Vec<_>>()
                .join("\n")
        )),
    }
}

fn smoothstep(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    progress * progress * (3.0 - 2.0 * progress)
}

fn create_alert_presenter(
    pet: &PetWindow,
    notification: &NotificationWindow,
    settings: Rc<RefCell<AppSettings>>,
    notification_message: Rc<RefCell<String>>,
    animation_sequence: Rc<RefCell<Vec<i32>>>,
) -> (AlertPresenter, CancelMotion) {
    let motion = Rc::new(RefCell::new(None::<PetMotion>));
    let motion_timer = Rc::new(Timer::default());
    {
        let weak_pet = pet.as_weak();
        let weak_notification = notification.as_weak();
        let motion = motion.clone();
        let weak_timer = Rc::downgrade(&motion_timer);
        let notification_message = notification_message.clone();
        let animation_sequence = animation_sequence.clone();
        motion_timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
            let (x, y, walking_frame, completed, text) = {
                let motion = motion.borrow();
                let Some(state) = motion.as_ref() else { return };
                let elapsed = state.started_at.elapsed();
                let progress = (elapsed.as_secs_f32() / state.duration.as_secs_f32()).min(1.0);
                let eased = smoothstep(progress);
                let x = state.start_x
                    + ((state.target_x - state.start_x) as f32 * eased).round() as i32;
                let y = state.start_y
                    + ((state.target_y - state.start_y) as f32 * eased).round() as i32;
                let walking_frame = if (elapsed.as_millis() / 160).is_multiple_of(2) {
                    0
                } else {
                    2
                };
                (x, y, walking_frame, progress >= 1.0, state.message.clone())
            };
            let Some(pet) = weak_pet.upgrade() else {
                return;
            };
            pet.window().set_position(PhysicalPosition::new(x, y));
            pet.set_frame_index(walking_frame);
            if !completed {
                return;
            }
            motion.borrow_mut().take();
            if let Some(timer) = weak_timer.upgrade() {
                timer.stop();
            }
            *notification_message.borrow_mut() = text.clone();
            *animation_sequence.borrow_mut() = vec![6, 6, 6, 3];
            if let Some(notification) = weak_notification.upgrade() {
                notification.set_message(text.into());
                position_notification(&notification, &pet);
                let _ = notification.show();
            }
        });
        motion_timer.stop();
    }

    let weak_pet = pet.as_weak();
    let weak_notification = notification.as_weak();
    let presenter_motion = motion.clone();
    let presenter_timer = motion_timer.clone();
    let presenter = Rc::new(move |text: String| {
        let Some(pet) = weak_pet.upgrade() else {
            return;
        };
        if let Some(notification) = weak_notification.upgrade() {
            let _ = notification.hide();
        }
        animation_sequence.borrow_mut().clear();
        pet.set_frame_index(0);
        let start = pet.window().position();
        let (target_x, target_y) = platform::active_work_area(pet.window())
            .center(pet.window().size().width, pet.window().size().height);
        let distance = (((target_x - start.x).pow(2) + (target_y - start.y).pow(2)) as f32).sqrt();
        let base_millis = if settings.borrow().reduce_motion {
            550.0
        } else {
            (750.0 + distance * 0.8).clamp(900.0, 1_800.0)
        };
        *presenter_motion.borrow_mut() = Some(PetMotion {
            start_x: start.x,
            start_y: start.y,
            target_x,
            target_y,
            started_at: Instant::now(),
            duration: Duration::from_millis(base_millis as u64),
            message: text,
        });
        let _ = pet.show();
        presenter_timer.restart();
    });
    let cancel = Rc::new(move || {
        motion.borrow_mut().take();
        motion_timer.stop();
    });
    (presenter, cancel)
}

fn position_notification(notification: &NotificationWindow, pet: &PetWindow) {
    let pet_position = pet.window().position();
    let area = platform::active_work_area(pet.window());
    let width = notification.window().size().width as i32;
    let height = notification.window().size().height as i32;
    let x = (pet_position.x - width - 12).clamp(area.left, area.right - width);
    let y = (pet_position.y + pet.window().size().height as i32 - height)
        .clamp(area.top, area.bottom - height);
    notification
        .window()
        .set_position(PhysicalPosition::new(x, y));
}

fn position_menu(menu: &MenuWindow, pet: &PetWindow) {
    let pet_position = pet.window().position();
    let pet_size = pet.window().size();
    let area = platform::active_work_area(pet.window());
    let width = menu.window().size().width as i32;
    let height = menu.window().size().height as i32;
    let max_x = (area.right - width).max(area.left);
    let max_y = (area.bottom - height).max(area.top);
    let centered_x = pet_position.x + (pet_size.width as i32 - width) / 2;
    let x = centered_x.clamp(area.left, max_x);
    let below = pet_position.y + pet_size.height as i32 + 8;
    let above = pet_position.y - height - 8;
    let y = if below <= max_y {
        below
    } else if above >= area.top {
        above
    } else {
        (pet_position.y + (pet_size.height as i32 - height) / 2).clamp(area.top, max_y)
    };
    menu.window().set_position(PhysicalPosition::new(x, y));
}

#[cfg(test)]
mod app_tests {
    use super::*;

    #[test]
    fn combines_simultaneous_reminders_without_losing_titles() {
        assert_eq!(format_reminder_alert(&[]), None);
        assert_eq!(
            format_reminder_alert(&["喝水".into()]),
            Some("时间到了：喝水".into())
        );
        let combined = format_reminder_alert(&["开会".into(), "提交报告".into()]).unwrap();
        assert!(combined.contains("2 个提醒"));
        assert!(combined.contains("• 开会"));
        assert!(combined.contains("• 提交报告"));
    }

    #[test]
    fn motion_easing_is_bounded_and_monotonic() {
        assert_eq!(smoothstep(-1.0), 0.0);
        assert_eq!(smoothstep(0.0), 0.0);
        assert_eq!(smoothstep(1.0), 1.0);
        assert_eq!(smoothstep(2.0), 1.0);
        let samples = (0..=10)
            .map(|step| smoothstep(step as f32 / 10.0))
            .collect::<Vec<_>>();
        assert!(samples.windows(2).all(|pair| pair[0] <= pair[1]));
    }
}
