#[derive(Debug, Clone, Copy)]
pub struct WorkArea {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl WorkArea {
    pub fn center(self, width: u32, height: u32) -> (i32, i32) {
        let x = self.left + ((self.right - self.left - width as i32).max(0) / 2);
        let y = self.top + ((self.bottom - self.top - height as i32).max(0) / 2);
        (x, y)
    }
}

pub fn focus_window(window: &slint::Window) {
    use slint::winit_030::WinitWindowAccessor;

    let _ = window.with_winit_window(|window| window.focus_window());
}

pub fn set_window_icon(window: &slint::Window, rgba: Vec<u8>, width: u32, height: u32) {
    use slint::winit_030::{WinitWindowAccessor, winit::window::Icon};

    let Ok(icon) = Icon::from_rgba(rgba, width, height) else {
        return;
    };
    let _ = window.with_winit_window(|window| window.set_window_icon(Some(icon)));
}

pub fn app_icon_rgba(size: u32) -> Vec<u8> {
    use image::imageops::FilterType;

    image::load_from_memory(include_bytes!(
        "../packages/characters/shadow-crow-ninja/icon.png"
    ))
    .map(|icon| {
        icon.resize_exact(size, size, FilterType::Lanczos3)
            .to_rgba8()
            .into_raw()
    })
    .unwrap_or_else(|_| fallback_icon_rgba(size))
}

fn fallback_icon_rgba(size: u32) -> Vec<u8> {
    let mut rgba = vec![0; (size * size * 4) as usize];
    let center = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let distance = (dx * dx + dy * dy).sqrt();
            let pixel = ((y * size + x) * 4) as usize;
            if distance < size as f32 * 0.47 {
                rgba[pixel..pixel + 4].copy_from_slice(&[126, 109, 224, 255]);
            }
            let crow_head =
                dx * dx + (dy + size as f32 * 0.09).powi(2) < (size as f32 * 0.25).powi(2);
            let crow_body =
                dx.abs() < size as f32 * 0.28 && dy > size as f32 * 0.03 && dy < size as f32 * 0.36;
            if crow_head || crow_body {
                rgba[pixel..pixel + 4].copy_from_slice(&[35, 30, 55, 255]);
            }
            let eye_y = dy + size as f32 * 0.12;
            let left_eye =
                (dx + size as f32 * 0.09).powi(2) + eye_y.powi(2) < (size as f32 * 0.035).powi(2);
            let right_eye =
                (dx - size as f32 * 0.09).powi(2) + eye_y.powi(2) < (size as f32 * 0.035).powi(2);
            if left_eye || right_eye {
                rgba[pixel..pixel + 4].copy_from_slice(&[255, 247, 180, 255]);
            }
            if dx > size as f32 * 0.2 && dx < size as f32 * 0.4 && dy.abs() < size as f32 * 0.06 {
                rgba[pixel..pixel + 4].copy_from_slice(&[246, 183, 70, 255]);
            }
        }
    }
    rgba
}

#[cfg(windows)]
pub fn hide_from_taskbar(window: &slint::Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
            SetWindowLongPtrW, SetWindowPos, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
        },
    };

    let handle = window.window_handle();
    let Ok(raw) = handle.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(win32) = raw.as_raw() else {
        return;
    };
    let hwnd = HWND(win32.hwnd.get() as *mut std::ffi::c_void);
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let style = (style & !(WS_EX_APPWINDOW.0 as isize)) | WS_EX_TOOLWINDOW.0 as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
        );
    }
}

#[cfg(not(windows))]
pub fn hide_from_taskbar(_window: &slint::Window) {}

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMenuAction {
    Show,
    Hide,
    Exit,
}

#[cfg(windows)]
type TrayClickHandler = std::rc::Rc<std::cell::RefCell<Option<Box<dyn Fn()>>>>;
#[cfg(windows)]
type TrayMenuHandler = std::rc::Rc<std::cell::RefCell<Option<Box<dyn Fn(TrayMenuAction)>>>>;

#[cfg(windows)]
pub struct TrayController {
    tray: tray_icon::TrayIcon,
    _timer: slint::Timer,
    clicked: TrayClickHandler,
    menu_action: TrayMenuHandler,
}

#[cfg(windows)]
impl TrayController {
    pub fn new(rgba: Vec<u8>, width: u32, height: u32) -> Result<Self, String> {
        use std::{cell::RefCell, rc::Rc, time::Duration};
        use tray_icon::{
            Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent,
            menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
        };

        let menu = Menu::new();
        let show_item = MenuItem::new("显示鸦影", true, None);
        let hide_item = MenuItem::new("隐藏鸦影", true, None);
        let separator = PredefinedMenuItem::separator();
        let exit_item = MenuItem::new("退出", true, None);
        menu.append_items(&[&show_item, &hide_item, &separator, &exit_item])
            .map_err(|error| error.to_string())?;
        let icon = Icon::from_rgba(rgba, width, height).map_err(|error| error.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_tooltip("鸦影")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_icon(icon)
            .build()
            .map_err(|error| error.to_string())?;

        let clicked = Rc::new(RefCell::new(None::<Box<dyn Fn()>>));
        let menu_action = Rc::new(RefCell::new(None::<Box<dyn Fn(TrayMenuAction)>>));
        let timer = slint::Timer::default();
        let clicked_for_timer = clicked.clone();
        let menu_action_for_timer = menu_action.clone();
        let tray_id = tray.id().clone();
        let show_id = show_item.id().clone();
        let hide_id = hide_item.id().clone();
        let exit_id = exit_item.id().clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(75),
            move || {
                while let Ok(event) = MenuEvent::receiver().try_recv() {
                    let action = if event.id == show_id {
                        Some(TrayMenuAction::Show)
                    } else if event.id == hide_id {
                        Some(TrayMenuAction::Hide)
                    } else if event.id == exit_id {
                        Some(TrayMenuAction::Exit)
                    } else {
                        None
                    };
                    if let (Some(action), Some(callback)) =
                        (action, menu_action_for_timer.borrow().as_ref())
                    {
                        callback(action);
                    }
                }
                while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            id,
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } if id == tray_id
                    ) && let Some(callback) = clicked_for_timer.borrow().as_ref()
                    {
                        callback();
                    }
                }
            },
        );

        Ok(Self {
            tray,
            _timer: timer,
            clicked,
            menu_action,
        })
    }

    pub fn show_icon(&self, visible: bool) {
        let _ = self.tray.set_visible(visible);
    }

    pub fn set_tooltip(&self, tooltip: &str) {
        let _ = self.tray.set_tooltip(Some(tooltip));
    }

    pub fn on_clicked(&mut self, callback: impl Fn() + 'static) {
        *self.clicked.borrow_mut() = Some(Box::new(callback));
    }

    pub fn on_menu_action(&mut self, callback: impl Fn(TrayMenuAction) + 'static) {
        *self.menu_action.borrow_mut() = Some(Box::new(callback));
    }
}

#[cfg(not(windows))]
pub struct TrayController;

#[cfg(not(windows))]
impl TrayController {
    pub fn new(_rgba: Vec<u8>, _width: u32, _height: u32) -> Result<Self, String> {
        Ok(Self)
    }
    pub fn show_icon(&self, _visible: bool) {}
    pub fn set_tooltip(&self, _tooltip: &str) {}
    pub fn on_clicked(&mut self, _callback: impl Fn() + 'static) {}
    pub fn on_menu_action(&mut self, _callback: impl Fn(TrayMenuAction) + 'static) {}
}

pub const fn tray_supported() -> bool {
    cfg!(windows)
}

#[cfg(windows)]
pub fn active_work_area(window: &slint::Window) -> WorkArea {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{
        Foundation::HWND,
        Graphics::Gdi::{
            GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
        },
        UI::WindowsAndMessaging::GetForegroundWindow,
    };

    unsafe {
        let handle = window.window_handle();
        let hwnd = handle
            .window_handle()
            .ok()
            .and_then(|handle| match handle.as_raw() {
                RawWindowHandle::Win32(value) => {
                    Some(HWND(value.hwnd.get() as *mut std::ffi::c_void))
                }
                _ => None,
            });
        let hwnd = match hwnd {
            Some(hwnd) => hwnd,
            None => GetForegroundWindow(),
        };
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            return WorkArea {
                left: info.rcWork.left,
                top: info.rcWork.top,
                right: info.rcWork.right,
                bottom: info.rcWork.bottom,
            };
        }
    }
    WorkArea {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    }
}

#[cfg(not(windows))]
pub fn active_work_area(window: &slint::Window) -> WorkArea {
    use slint::winit_030::WinitWindowAccessor;

    window
        .with_winit_window(|window| {
            window
                .current_monitor()
                .or_else(|| window.available_monitors().next())
                .map(|monitor| {
                    let position = monitor.position();
                    let size = monitor.size();
                    WorkArea {
                        left: position.x,
                        top: position.y,
                        right: position.x + size.width as i32,
                        bottom: position.y + size.height as i32,
                    }
                })
        })
        .flatten()
        .unwrap_or(WorkArea {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        })
}

#[cfg(windows)]
pub fn window_has_focus(window: &slint::Window) -> bool {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::GetForegroundWindow};

    let handle = window.window_handle();
    let Ok(raw) = handle.window_handle() else {
        return false;
    };
    let RawWindowHandle::Win32(win32) = raw.as_raw() else {
        return false;
    };
    let hwnd = HWND(win32.hwnd.get() as *mut std::ffi::c_void);
    unsafe { GetForegroundWindow() == hwnd }
}

#[cfg(not(windows))]
pub fn window_has_focus(window: &slint::Window) -> bool {
    use slint::winit_030::WinitWindowAccessor;

    window
        .with_winit_window(|window| window.has_focus())
        .unwrap_or(false)
}

#[cfg(windows)]
pub fn idle_millis() -> u64 {
    use windows::Win32::{
        System::SystemInformation::GetTickCount64,
        UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
    };
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            ..Default::default()
        };
        if GetLastInputInfo(&mut info).as_bool() {
            return GetTickCount64().saturating_sub(info.dwTime as u64);
        }
    }
    0
}

#[cfg(windows)]
pub fn begin_window_drag(window: &slint::Window) -> bool {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::{
            Input::KeyboardAndMouse::ReleaseCapture,
            WindowsAndMessaging::{HTCAPTION, SendMessageW, WM_NCLBUTTONDOWN},
        },
    };

    let handle = window.window_handle();
    let Ok(raw) = handle.window_handle() else {
        return false;
    };
    let RawWindowHandle::Win32(win32) = raw.as_raw() else {
        return false;
    };
    unsafe {
        let _ = ReleaseCapture();
        SendMessageW(
            HWND(win32.hwnd.get() as *mut std::ffi::c_void),
            WM_NCLBUTTONDOWN,
            Some(WPARAM(HTCAPTION as usize)),
            Some(LPARAM(0)),
        );
    }
    true
}

#[cfg(not(windows))]
pub fn begin_window_drag(window: &slint::Window) -> bool {
    use slint::winit_030::WinitWindowAccessor;

    window
        .with_winit_window(|window| window.drag_window().is_ok())
        .unwrap_or(false)
}

#[cfg(not(windows))]
pub fn idle_millis() -> u64 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centers_inside_negative_coordinate_monitor() {
        let area = WorkArea {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1040,
        };
        assert_eq!(area.center(320, 160), (-1120, 440));
    }

    #[test]
    fn generated_icon_has_expected_size_and_visible_pixels() {
        let icon = app_icon_rgba(32);
        assert_eq!(icon.len(), 32 * 32 * 4);
        assert!(icon.chunks_exact(4).any(|pixel| pixel[3] == 255));
    }
}
