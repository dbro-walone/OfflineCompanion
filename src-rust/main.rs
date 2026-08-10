#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use chrono::{Local, NaiveDateTime, TimeZone, Timelike};
use offline_companion::{
    app_runtime::{AppRuntime, BusinessFact, RuntimeUpdate},
    behavior::{
        self,
        event::PetEvent,
        locomotion::{self, LocomotionController, ReleasePath},
    },
    model::{AppSettings, PomodoroPhase, PomodoroState},
    package_runtime, packages, platform,
    storage::{self, AppPaths, Store},
};
use slint::{ComponentHandle, ModelRc, PhysicalPosition, Timer, TimerMode, VecModel};

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

struct ReleaseMotion {
    motion: locomotion::MotionState,
    path: ReleasePath,
    last_tick: Instant,
}

type AlertPresenter = Rc<dyn Fn(String)>;
type CancelMotion = Rc<dyn Fn()>;
#[derive(Clone)]
struct PetRuntimeBinding {
    pet: slint::Weak<PetWindow>,
    runtime: Rc<RefCell<AppRuntime>>,
}

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
    package_runtime::seeder::seed_defaults(&paths.root.join("packages"))?;
    let store = Rc::new(Store::open(&paths.database)?);
    let settings = Rc::new(RefCell::new(storage::load_settings(&paths.settings)));
    let runtime = Rc::new(RefCell::new(AppRuntime::new(
        &paths.characters,
        &paths.actions,
        &settings.borrow(),
    )));
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
    if let Some(update) = runtime
        .borrow_mut()
        .dispatch(PetEvent::AppStarted, monotonic_ms())?
    {
        apply_runtime_update(&pet, &update);
    }
    refresh_todos(&todos, &store, false);

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
        PetRuntimeBinding {
            pet: pet.as_weak(),
            runtime: runtime.clone(),
        },
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
        runtime.clone(),
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
        runtime.clone(),
    );
    wire_packages(
        &package_window,
        &pet,
        paths.clone(),
        settings.clone(),
        runtime.clone(),
    );
    wire_notification(
        &notification,
        notification_message.clone(),
        store.clone(),
        PetRuntimeBinding {
            pet: pet.as_weak(),
            runtime: runtime.clone(),
        },
    );
    {
        let suppress_next_pet_click = suppress_next_pet_click.clone();
        let runtime = runtime.clone();
        let weak_pet = pet.as_weak();
        pet.on_pet_clicked(move |y| {
            if std::mem::take(&mut *suppress_next_pet_click.borrow_mut()) {
                return;
            }
            if let Some(pet) = weak_pet.upgrade() {
                let region = behavior::event::hit_region(y, pet.window().size().height as f32);
                if let Ok(Some(update)) = runtime.borrow_mut().dispatch(
                    PetEvent::PetClicked {
                        region,
                        click_count: 1,
                    },
                    monotonic_ms(),
                ) {
                    apply_runtime_update(&pet, &update);
                }
            }
        });
    }
    {
        let runtime = runtime.clone();
        let weak_pet = pet.as_weak();
        pet.on_hover_changed(move |hover| {
            if let Some(pet) = weak_pet.upgrade() {
                dispatch_runtime(
                    &runtime,
                    &pet,
                    if hover {
                        PetEvent::PointerNear { distance_px: 0.0 }
                    } else {
                        PetEvent::PointerExited
                    },
                );
            }
        });
    }

    let pointer_near_timer = Timer::default();
    {
        let runtime = runtime.clone();
        let weak_pet = pet.as_weak();
        let settings = settings.clone();
        let was_near = Rc::new(RefCell::new(false));
        let approach_steps = Rc::new(RefCell::new(0_u8));
        pointer_near_timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
            let (Some(pet), Some((x, y))) = (weak_pet.upgrade(), platform::cursor_position())
            else {
                return;
            };
            let p = pet.window().position();
            let s = pet.window().size();
            let dx = if x < p.x {
                p.x - x
            } else if x > p.x + s.width as i32 {
                x - (p.x + s.width as i32)
            } else {
                0
            };
            let dy = if y < p.y {
                p.y - y
            } else if y > p.y + s.height as i32 {
                y - (p.y + s.height as i32)
            } else {
                0
            };
            let distance = (dx as f32).hypot(dy as f32);
            let near = distance <= settings.borrow().pointer_near_distance_px as f32;
            if near != *was_near.borrow() {
                *was_near.borrow_mut() = near;
                *approach_steps.borrow_mut() = if near
                    && settings.borrow().allow_pet_approach
                    && settings.borrow().pet_interaction_level != "quiet"
                    && !settings.borrow().reduce_motion
                {
                    5
                } else {
                    0
                };
                dispatch_runtime(
                    &runtime,
                    &pet,
                    if near {
                        PetEvent::PointerNear {
                            distance_px: distance,
                        }
                    } else {
                        PetEvent::PointerExited
                    },
                );
            }
            let mut steps = approach_steps.borrow_mut();
            if near && *steps > 0 {
                let center_x = p.x + s.width as i32 / 2;
                let center_y = p.y + s.height as i32 / 2;
                let delta_x = (x - center_x).clamp(-8, 8);
                let delta_y = (y - center_y).clamp(-8, 8);
                let area = platform::monitor_of_pet(pet.window());
                let (next_x, next_y) = locomotion::clamp_to_work_area(
                    p.x + delta_x,
                    p.y + delta_y,
                    s.width,
                    s.height,
                    area,
                );
                pet.window()
                    .set_position(PhysicalPosition::new(next_x, next_y));
                *steps -= 1;
            }
        });
    }

    let animation_timer = Timer::default();
    {
        let weak_pet = pet.as_weak();
        let runtime = runtime.clone();
        animation_timer.start(TimerMode::Repeated, Duration::from_millis(25), move || {
            let Some(pet) = weak_pet.upgrade() else {
                return;
            };
            if let Ok(Some(update)) = runtime.borrow_mut().tick(monotonic_ms()) {
                apply_runtime_update(&pet, &update);
            }
        });
    }

    let display_change_timer = Timer::default();
    {
        let weak_pet = pet.as_weak();
        let runtime = runtime.clone();
        display_change_timer.start(TimerMode::Repeated, Duration::from_secs(2), move || {
            let Some(pet) = weak_pet.upgrade() else {
                return;
            };
            clamp_pet_to_work_area(&pet);
            dispatch_runtime(&runtime, &pet, PetEvent::DisplayChanged);
        });
    }

    let (present_alert, cancel_pet_motion) = create_alert_presenter(
        &pet,
        &notification,
        settings.clone(),
        notification_message.clone(),
    );
    wire_pet_drag(
        &pet,
        settings.clone(),
        paths.clone(),
        suppress_next_pet_click.clone(),
        cancel_pet_motion,
        runtime.clone(),
    );

    let scheduler_timer = Timer::default();
    {
        let store = store.clone();
        let present_alert = present_alert.clone();
        let runtime = runtime.clone();
        let weak_pet = pet.as_weak();
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
            if let Some(pet) = weak_pet.upgrade() {
                let id = due.first().map(|x| x.id.clone()).unwrap_or_default();
                dispatch_fact_runtime(
                    &runtime,
                    &pet,
                    BusinessFact::ReminderDue {
                        kind: behavior::event::ReminderKind::Default,
                        id,
                    },
                );
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
        let runtime = runtime.clone();
        let weak_pet = pet.as_weak();
        pomodoro_timer.start(TimerMode::Repeated, Duration::from_secs(1), move || {
            let mut state = state.borrow_mut();
            if !state.running || state.paused {
                return;
            }
            let completed = state.update_at(Local::now().timestamp_millis());
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
            if !completed {
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
            if let Some(pet) = weak_pet.upgrade() {
                dispatch_fact_runtime(&runtime, &pet, BusinessFact::PomodoroCompleted);
            }
            present_alert(text.into());
        });
    }

    let sedentary_timer = Timer::default();
    {
        let settings = settings.clone();
        let present_alert = present_alert.clone();
        let active_seconds = Rc::new(RefCell::new(0u64));
        let runtime = runtime.clone();
        let weak_pet = pet.as_weak();
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
            if let Some(pet) = weak_pet.upgrade() {
                dispatch_fact_runtime(&runtime, &pet, BusinessFact::SedentaryWarning);
            }
            present_alert(text.into());
        });
    }

    let menu_dismiss_timer = Timer::default();
    {
        let weak_pet = pet.as_weak();
        pet.on_request_menu_focus(move || {
            if let Some(pet) = weak_pet.upgrade() {
                platform::focus_window(pet.window());
            }
        });

        let weak_pet = pet.as_weak();
        let menu_was_visible = Rc::new(RefCell::new(false));
        menu_dismiss_timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
            let Some(pet) = weak_pet.upgrade() else {
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
            if !platform::window_has_focus(pet.window()) {
                pet.set_menu_visible(false);
                *menu_was_visible.borrow_mut() = false;
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
}

fn wire_todos(
    todo_window: &TodoWindow,
    timer: &TimerWindow,
    pet_runtime: PetRuntimeBinding,
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
        let runtime = pet_runtime.runtime.clone();
        let weak_pet = pet_runtime.pet.clone();
        todo_window.on_toggle_todo(move |id| {
            if let Some(window) = weak.upgrade()
                && let Ok(completed) = store.toggle_todo(&id)
            {
                if completed {
                    window.set_status_text("完成啦，鸦影在为你庆祝！".into());
                    if let Some(pet) = weak_pet.upgrade() {
                        dispatch_fact_runtime(
                            &runtime,
                            &pet,
                            BusinessFact::TodoCompleted { id: id.to_string() },
                        );
                    }
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
        let runtime = pet_runtime.runtime.clone();
        let weak_pet = pet_runtime.pet.clone();
        todo_window.on_start_focus(move |title| {
            *active_todo.borrow_mut() = title.to_string();
            *five_minute_notified.borrow_mut() = false;
            let mut state = state.borrow_mut();
            *state = PomodoroState::default();
            state.start_at(Local::now().timestamp_millis());
            let _ = store.save_pomodoro(&state);
            if let Some(timer) = weak_timer.upgrade() {
                timer.set_active_todo(title);
                update_timer_view(&timer, &state, "");
                let _ = timer.hide();
            }
            if let Some(todos) = weak_todos.upgrade() {
                let _ = todos.hide();
            }
            if let Some(pet) = weak_pet.upgrade() {
                dispatch_fact_runtime(&runtime, &pet, BusinessFact::PomodoroStarted);
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
    runtime: Rc<RefCell<AppRuntime>>,
) {
    {
        let weak = window.as_weak();
        let state = state.clone();
        let active_todo = active_todo.clone();
        let weak_pet = pet.as_weak();
        let store = store.clone();
        let five_minute_notified = five_minute_notified.clone();
        let runtime = runtime.clone();
        window.on_start(move || {
            *five_minute_notified.borrow_mut() = false;
            let mut state = state.borrow_mut();
            state.start_at(Local::now().timestamp_millis());
            let _ = store.save_pomodoro(&state);
            if let Some(window) = weak.upgrade() {
                update_timer_view(&window, &state, &active_todo.borrow());
                let _ = window.hide();
            }
            if let Some(pet) = weak_pet.upgrade() {
                dispatch_fact_runtime(&runtime, &pet, BusinessFact::PomodoroStarted);
            }
        });
    }
    {
        let weak = window.as_weak();
        let state = state.clone();
        let active_todo = active_todo.clone();
        let store = store.clone();
        let runtime = runtime.clone();
        let weak_pet = pet.as_weak();
        window.on_toggle_pause(move || {
            let mut state = state.borrow_mut();
            if state.running {
                if state.paused {
                    state.resume_at(Local::now().timestamp_millis())
                } else {
                    state.pause_at(Local::now().timestamp_millis())
                }
            }
            let _ = store.save_pomodoro(&state);
            if let Some(window) = weak.upgrade() {
                update_timer_view(&window, &state, &active_todo.borrow());
            }
            if let Some(pet) = weak_pet.upgrade() {
                dispatch_fact_runtime(
                    &runtime,
                    &pet,
                    if state.paused {
                        BusinessFact::PomodoroPaused
                    } else {
                        BusinessFact::PomodoroStarted
                    },
                );
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
    notification: &NotificationWindow,
    settings: Rc<RefCell<AppSettings>>,
    paths: Rc<AppPaths>,
    runtime: Rc<RefCell<AppRuntime>>,
) {
    let initial = settings.borrow().clone();
    window.set_scale_value(initial.pet_scale);
    window.set_topmost_value(initial.topmost);
    window.set_idle_value(initial.idle_actions_enabled);
    window.set_reduce_motion_value(initial.reduce_motion);
    window.set_theme_index(if initial.theme == "light" { 1 } else { 0 });
    window.set_sedentary_minutes(initial.sedentary_minutes as i32);
    window.set_proactive_invitation(initial.allow_proactive_invitation);
    window.set_interaction_cooldown(initial.interaction_cooldown_seconds as i32);
    window.set_pointer_near_distance(initial.pointer_near_distance_px as i32);
    window.set_allow_pet_approach(initial.allow_pet_approach);
    window.set_allow_mouse_follow(initial.allow_mouse_follow);
    window.set_interaction_level_index(match initial.pet_interaction_level.as_str() {
        "quiet" => 0,
        "active" => 2,
        _ => 1,
    });
    window.set_reminder_follow_pet(initial.reminder_follow_pet);
    let weak_window = window.as_weak();
    let weak_pet = pet.as_weak();
    let weak_todos = todos.as_weak();
    let weak_reminder = reminder.as_weak();
    let weak_timer = timer.as_weak();
    let weak_packages = packages.as_weak();
    let weak_notification = notification.as_weak();
    window.on_save(
        move |scale,
              topmost,
              idle,
              reduce_motion,
              theme_index,
              sedentary,
              proactive,
              cooldown,
              near_distance,
              allow_approach,
              allow_mouse_follow,
              interaction_level,
              reminder_follow_pet| {
            let mut value = settings.borrow_mut();
            value.pet_scale = scale.clamp(0.75, 1.4);
            value.topmost = topmost;
            value.idle_actions_enabled = idle;
            value.reduce_motion = reduce_motion;
            value.theme = if theme_index == 1 { "light" } else { "dark" }.into();
            value.sedentary_minutes = sedentary.clamp(30, 120) as u32;
            value.allow_proactive_invitation = proactive;
            value.interaction_cooldown_seconds = cooldown.clamp(10, 120) as u64;
            value.pointer_near_distance_px = near_distance.clamp(60, 240) as u32;
            value.allow_pet_approach = allow_approach;
            value.allow_mouse_follow = allow_mouse_follow;
            value.pet_interaction_level = match interaction_level {
                0 => "quiet",
                2 => "active",
                _ => "balanced",
            }
            .into();
            value.reminder_follow_pet = reminder_follow_pet;
            {
                let mut runtime = runtime.borrow_mut();
                runtime.apply_settings(&value, monotonic_ms());
            }
            let _ = storage::save_settings(&paths.settings, &value);
            if let Some(pet) = weak_pet.upgrade() {
                apply_settings_to_pet(&pet, &value);
                clamp_pet_to_work_area(&pet);
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

fn wire_packages(
    window: &PackageWindow,
    pet: &PetWindow,
    paths: Rc<AppPaths>,
    settings: Rc<RefCell<AppSettings>>,
    runtime: Rc<RefCell<AppRuntime>>,
) {
    refresh_packages(window, &runtime.borrow());
    {
        let weak = window.as_weak();
        let paths = paths.clone();
        let runtime = runtime.clone();
        window.on_import_package(move || {
            let Some(window) = weak.upgrade() else { return };
            let Some(path) = rfd::FileDialog::new()
                .add_filter("扩展包", &["zip"])
                .pick_file()
            else {
                return;
            };
            match packages::install_package(&path, &paths.characters, &paths.actions) {
                Ok(status) => {
                    runtime
                        .borrow_mut()
                        .reload(&paths.characters, &paths.actions);
                    refresh_packages(&window, &runtime.borrow());
                    window.set_status_text(format!("{status}；可点击“启用并预览”立即验证").into())
                }
                Err(error) => window.set_status_text(format!("导入失败：{error}").into()),
            }
        });
    }
    {
        let weak = window.as_weak();
        let weak_pet = pet.as_weak();
        let paths = paths.clone();
        let settings = settings.clone();
        let runtime = runtime.clone();
        window.on_toggle_package(move |id| {
            let Some(window) = weak.upgrade() else { return };
            let id = id.to_string();
            let enabled = runtime
                .borrow()
                .enabled_action_pack_ids()
                .iter()
                .any(|value| value == &id);
            let update = {
                let mut runtime = runtime.borrow_mut();
                if !runtime.set_action_pack_enabled(&id, !enabled) {
                    window.set_status_text("动作包不存在或已失效".into());
                    return;
                }
                if enabled {
                    runtime.activate_default(monotonic_ms())
                } else {
                    runtime.preview_action_pack(&id, monotonic_ms())
                }
            };
            persist_runtime_package_settings(&settings, &paths, &runtime.borrow());
            if let (Ok(Some(update)), Some(pet)) = (update, weak_pet.upgrade()) {
                apply_runtime_update(&pet, &update);
            }
            refresh_packages(&window, &runtime.borrow());
            window.set_status_text(
                if enabled {
                    "动作包已禁用，已回退默认动作"
                } else {
                    "动作包已启用并开始预览"
                }
                .into(),
            );
        });
    }
    {
        let weak = window.as_weak();
        let weak_pet = pet.as_weak();
        let paths = paths.clone();
        let settings = settings.clone();
        let runtime = runtime.clone();
        window.on_select_character(move |id| {
            let Some(window) = weak.upgrade() else { return };
            let update = {
                let mut runtime = runtime.borrow_mut();
                if !runtime.set_character(&id) {
                    window.set_status_text("角色包不存在或已失效".into());
                    return;
                }
                runtime.activate_default(monotonic_ms())
            };
            persist_runtime_package_settings(&settings, &paths, &runtime.borrow());
            if let (Ok(Some(update)), Some(pet)) = (update, weak_pet.upgrade()) {
                apply_runtime_update(&pet, &update);
            }
            refresh_packages(&window, &runtime.borrow());
            window.set_status_text("当前角色已切换，无需重启".into());
        });
    }
    {
        let weak = window.as_weak();
        let weak_pet = pet.as_weak();
        let paths = paths.clone();
        let settings = settings.clone();
        let runtime = runtime.clone();
        window.on_delete_package(move |id| {
            let Some(window) = weak.upgrade() else { return };
            let id = id.to_string();
            if matches!(
                id.as_str(),
                "character.shadow-crow-ninja" | "action.shadow-crow.office"
            ) {
                window.set_status_text("内置默认包受保护，不能删除".into());
                return;
            }
            let target = {
                let runtime = runtime.borrow();
                if let Some(package) = runtime.catalog().actions.get(&id) {
                    Some((package.root.clone(), paths.actions.clone(), true))
                } else {
                    runtime
                        .catalog()
                        .characters
                        .get(&id)
                        .map(|package| (package.root.clone(), paths.characters.clone(), false))
                }
            };
            let Some((root, package_root, is_action)) = target else {
                window.set_status_text("扩展包不存在或已失效".into());
                return;
            };
            if !is_action && runtime.borrow().current_character_id() == id {
                window.set_status_text("请先切换到其他角色，再删除当前角色".into());
                return;
            }
            if let Err(error) = packages::remove_package(&root, &package_root) {
                window.set_status_text(format!("删除失败：{error}").into());
                return;
            }
            {
                let mut runtime = runtime.borrow_mut();
                runtime.reload(&paths.characters, &paths.actions);
            }
            persist_runtime_package_settings(&settings, &paths, &runtime.borrow());
            let update = runtime.borrow_mut().activate_default(monotonic_ms());
            if let (Ok(Some(update)), Some(pet)) = (update, weak_pet.upgrade()) {
                apply_runtime_update(&pet, &update);
            }
            refresh_packages(&window, &runtime.borrow());
            window.set_status_text("扩展包已删除，运行时已安全回退".into());
        });
    }
}

fn refresh_packages(window: &PackageWindow, runtime: &AppRuntime) {
    let mut rows = Vec::new();
    for p in runtime.catalog().characters.values() {
        rows.push(PackageRow {
            id: p.manifest.id.clone().into(),
            name: p.manifest.name.clone().into(),
            version: p.manifest.version.clone().into(),
            kind: "角色".into(),
            status: if runtime.current_character_id() == p.manifest.id {
                "当前"
            } else {
                "可用"
            }
            .into(),
            details: format!("{} 个动作", p.manifest.actions.len()).into(),
            is_action: false,
            is_current: runtime.current_character_id() == p.manifest.id,
            can_delete: p.manifest.id != "character.shadow-crow-ninja"
                && runtime.current_character_id() != p.manifest.id,
        });
    }
    for p in runtime.catalog().actions.values() {
        rows.push(PackageRow {
            id: p.manifest.id.clone().into(),
            name: p.manifest.name.clone().into(),
            version: p.manifest.version.clone().into(),
            kind: "动作包".into(),
            status: if runtime.enabled_action_pack_ids().contains(&p.manifest.id) {
                "已启用"
            } else {
                "已禁用"
            }
            .into(),
            details: p
                .manifest
                .actions
                .iter()
                .map(|action| {
                    format!(
                        "{} · {}",
                        action.semantic.as_deref().unwrap_or(&action.id),
                        action.trigger
                    )
                })
                .collect::<Vec<_>>()
                .join("；")
                .into(),
            is_action: true,
            is_current: runtime.enabled_action_pack_ids().contains(&p.manifest.id),
            can_delete: p.manifest.id != "action.shadow-crow.office",
        });
    }
    window.set_package_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
    if !runtime.catalog().warnings.is_empty() {
        window.set_status_text(
            format!(
                "部分扩展包未加载：{}",
                runtime.catalog().warnings.join("；")
            )
            .into(),
        );
    }
}

fn persist_runtime_package_settings(
    settings: &Rc<RefCell<AppSettings>>,
    paths: &AppPaths,
    runtime: &AppRuntime,
) {
    let mut settings = settings.borrow_mut();
    settings.current_character_id = runtime.current_character_id().into();
    settings.enabled_action_pack_ids = runtime.enabled_action_pack_ids().to_vec();
    let _ = storage::save_settings(&paths.settings, &settings);
}

fn wire_notification(
    window: &NotificationWindow,
    message: Rc<RefCell<String>>,
    store: Rc<Store>,
    pet_runtime: PetRuntimeBinding,
) {
    let weak = window.as_weak();
    let runtime = pet_runtime.runtime.clone();
    let weak_pet = pet_runtime.pet.clone();
    window.on_dismiss(move || {
        if let Some(window) = weak.upgrade() {
            let _ = window.hide();
        }
        if let Some(pet) = weak_pet.upgrade() {
            dispatch_fact_runtime(
                &runtime,
                &pet,
                BusinessFact::ReminderHandled {
                    kind: behavior::event::ReminderKind::Default,
                    id: "notification".into(),
                },
            );
        }
    });
    let weak = window.as_weak();
    let runtime = pet_runtime.runtime;
    let weak_pet = pet_runtime.pet;
    window.on_snooze(move || {
        let _ = store.add_reminder(
            &message.borrow(),
            Local::now() + chrono::Duration::minutes(10),
        );
        if let Some(window) = weak.upgrade() {
            let _ = window.hide();
        }
        if let Some(pet) = weak_pet.upgrade() {
            dispatch_fact_runtime(
                &runtime,
                &pet,
                BusinessFact::ReminderHandled {
                    kind: behavior::event::ReminderKind::Default,
                    id: "snoozed".into(),
                },
            );
        }
    });
}

fn wire_pet_drag(
    pet: &PetWindow,
    settings: Rc<RefCell<AppSettings>>,
    paths: Rc<AppPaths>,
    suppress_next_click: Rc<RefCell<bool>>,
    cancel_motion: CancelMotion,
    runtime: Rc<RefCell<AppRuntime>>,
) {
    let drag_origin = Rc::new(RefCell::new(None::<(i32, i32, Instant)>));
    let drag_sample = Rc::new(RefCell::new(None::<(Instant, f32, f32)>));
    let release_motion = Rc::new(RefCell::new(None::<ReleaseMotion>));
    let release_timer = Rc::new(Timer::default());
    {
        let weak = pet.as_weak();
        let runtime = runtime.clone();
        let release_motion = release_motion.clone();
        let weak_timer = Rc::downgrade(&release_timer);
        release_timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
            let Some(pet) = weak.upgrade() else { return };
            let mut motion = release_motion.borrow_mut();
            let Some(state) = motion.as_mut() else { return };
            let elapsed = state.last_tick.elapsed().as_secs_f32().clamp(0.001, 0.05);
            state.last_tick = Instant::now();
            let step = state.motion.step(elapsed);
            pet.window()
                .set_position(PhysicalPosition::new(step.x, step.y));
            if step.landed {
                let path = state.path;
                motion.take();
                if let Some(timer) = weak_timer.upgrade() {
                    timer.stop();
                }
                drop(motion);
                dispatch_runtime(&runtime, &pet, PetEvent::Landing { path });
            }
        });
        release_timer.stop();
    }
    {
        let weak = pet.as_weak();
        let drag_origin = drag_origin.clone();
        let runtime = runtime.clone();
        let drag_sample = drag_sample.clone();
        let release_motion = release_motion.clone();
        let release_timer = release_timer.clone();
        pet.on_drag_start(move |x, y| {
            cancel_motion();
            let Some(pet) = weak.upgrade() else { return };
            let position = pet.window().position();
            release_motion.borrow_mut().take();
            release_timer.stop();
            *drag_origin.borrow_mut() = Some((position.x, position.y, Instant::now()));
            *drag_sample.borrow_mut() = Some((Instant::now(), x, y));
            dispatch_runtime(
                &runtime,
                &pet,
                PetEvent::DragStarted {
                    pointer_x: x,
                    pointer_y: y,
                },
            );
            if !platform::begin_window_drag(pet.window()) {
                drag_origin.borrow_mut().take();
            }
        });
    }
    {
        let weak = pet.as_weak();
        let runtime = runtime.clone();
        let drag_sample = drag_sample.clone();
        pet.on_drag_move(move |x, y| {
            let Some(pet) = weak.upgrade() else { return };
            let mut sample = drag_sample.borrow_mut();
            let Some((at, px, py)) = *sample else { return };
            let seconds = at.elapsed().as_secs_f32().max(0.001);
            dispatch_runtime(
                &runtime,
                &pet,
                PetEvent::DragMoved {
                    dx: x - px,
                    dy: y - py,
                    velocity_x: (x - px) / seconds,
                    velocity_y: (y - py) / seconds,
                },
            );
            *sample = Some((Instant::now(), x, y));
        });
    }
    {
        let weak = pet.as_weak();
        let runtime = runtime.clone();
        let drag_sample = drag_sample.clone();
        let release_motion = release_motion.clone();
        let release_timer = release_timer.clone();
        pet.on_drag_end(move || {
            let Some((origin_x, origin_y, started)) = drag_origin.borrow_mut().take() else {
                return;
            };
            let Some(pet) = weak.upgrade() else { return };
            let position = pet.window().position();
            drag_sample.borrow_mut().take();
            let seconds = started.elapsed().as_secs_f32().max(0.05);
            let velocity_x = (position.x - origin_x) as f32 / seconds;
            let velocity_y = (position.y - origin_y) as f32 / seconds;
            let area = platform::monitor_of_pet(pet.window());
            let size = pet.window().size();
            let plan = LocomotionController {
                reduce_motion: settings.borrow().reduce_motion,
            }
            .release(velocity_x, velocity_y, position.x, size.width, area);
            dispatch_runtime(
                &runtime,
                &pet,
                PetEvent::DragReleased {
                    velocity_x,
                    velocity_y,
                    x: position.x,
                    y: position.y,
                    path: plan.path,
                },
            );
            if !plan.animate_flight {
                let (x, y) = locomotion::clamp_to_work_area(
                    position.x,
                    position.y,
                    size.width,
                    size.height,
                    area,
                );
                pet.window().set_position(PhysicalPosition::new(x, y));
                if matches!(plan.path, ReleasePath::Drop | ReleasePath::Thrown) {
                    dispatch_runtime(&runtime, &pet, PetEvent::Landing { path: plan.path });
                }
            } else {
                *release_motion.borrow_mut() = Some(ReleaseMotion {
                    motion: locomotion::MotionState::new(
                        position.x,
                        position.y,
                        plan,
                        area,
                        size.width,
                        size.height,
                    ),
                    path: plan.path,
                    last_tick: Instant::now(),
                });
                release_timer.restart();
            }
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
    pet.set_pet_scale(settings.pet_scale.clamp(0.75, 1.4));
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
        let area = platform::active_work_area(pet.window());
        let scale = settings.pet_scale.clamp(0.75, 1.4);
        let (x, y) = offline_companion::behavior::locomotion::clamp_to_work_area(
            x,
            y,
            (250.0 * scale) as u32,
            (330.0 * scale) as u32,
            area,
        );
        pet.window().set_position(PhysicalPosition::new(x, y));
    } else {
        let area = platform::active_work_area(pet.window());
        pet.window()
            .set_position(PhysicalPosition::new(area.right - 290, area.bottom - 370));
    }
}

fn clamp_pet_to_work_area(pet: &PetWindow) {
    let position = pet.window().position();
    let scale = pet.get_pet_scale().clamp(0.75, 1.4);
    let area = platform::active_work_area(pet.window());
    let (x, y) = offline_companion::behavior::locomotion::clamp_to_work_area(
        position.x,
        position.y,
        (250.0 * scale) as u32,
        (330.0 * scale) as u32,
        area,
    );
    pet.window().set_position(PhysicalPosition::new(x, y));
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

fn monotonic_ms() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

fn apply_runtime_update(pet: &PetWindow, update: &RuntimeUpdate) {
    if let Ok(image) = slint::Image::load_from_path(&update.render.atlas) {
        pet.set_atlas_image(image);
    }
    pet.set_frame_width(update.render.frame_width as i32);
    pet.set_frame_height(update.render.frame_height as i32);
    pet.set_atlas_columns(update.render.columns as i32);
    pet.set_mirror_x(update.render.mirror_x);
    pet.set_frame_index(update.render.frame_index as i32);
    let _ = &update.action_id;
    if update.completed {
        pet.set_frame_index(update.render.frame_index as i32);
    }
}

fn dispatch_runtime(runtime: &Rc<RefCell<AppRuntime>>, pet: &PetWindow, event: PetEvent) {
    if let Ok(Some(update)) = runtime.borrow_mut().dispatch(event, monotonic_ms()) {
        apply_runtime_update(pet, &update)
    }
}

fn dispatch_fact_runtime(runtime: &Rc<RefCell<AppRuntime>>, pet: &PetWindow, fact: BusinessFact) {
    if let Ok(Some(update)) = runtime.borrow_mut().dispatch_fact(fact, monotonic_ms()) {
        apply_runtime_update(pet, &update)
    }
}

fn create_alert_presenter(
    pet: &PetWindow,
    notification: &NotificationWindow,
    settings: Rc<RefCell<AppSettings>>,
    notification_message: Rc<RefCell<String>>,
) -> (AlertPresenter, CancelMotion) {
    let motion = Rc::new(RefCell::new(None::<PetMotion>));
    let motion_timer = Rc::new(Timer::default());
    {
        let weak_pet = pet.as_weak();
        let weak_notification = notification.as_weak();
        let motion = motion.clone();
        let weak_timer = Rc::downgrade(&motion_timer);
        let notification_message = notification_message.clone();
        motion_timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
            let (x, y, completed, text) = {
                let motion = motion.borrow();
                let Some(state) = motion.as_ref() else { return };
                let elapsed = state.started_at.elapsed();
                let progress = (elapsed.as_secs_f32() / state.duration.as_secs_f32()).min(1.0);
                let eased = smoothstep(progress);
                let x = state.start_x
                    + ((state.target_x - state.start_x) as f32 * eased).round() as i32;
                let y = state.start_y
                    + ((state.target_y - state.start_y) as f32 * eased).round() as i32;
                (x, y, progress >= 1.0, state.message.clone())
            };
            let Some(pet) = weak_pet.upgrade() else {
                return;
            };
            pet.window().set_position(PhysicalPosition::new(x, y));
            if !completed {
                return;
            }
            motion.borrow_mut().take();
            if let Some(timer) = weak_timer.upgrade() {
                timer.stop();
            }
            *notification_message.borrow_mut() = text.clone();
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
        let start = pet.window().position();
        let move_for_reminder =
            settings.borrow().reminder_follow_pet && !settings.borrow().reduce_motion;
        let (target_x, target_y) = if move_for_reminder {
            platform::monitor_of_foreground_window(pet.window())
                .center(pet.window().size().width, pet.window().size().height)
        } else {
            (start.x, start.y)
        };
        let distance = (((target_x - start.x).pow(2) + (target_y - start.y).pow(2)) as f32).sqrt();
        let base_millis = if !move_for_reminder {
            1.0
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
