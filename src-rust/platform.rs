#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[cfg(windows)]
fn work_area_from_monitor(monitor: windows::Win32::Graphics::Gdi::HMONITOR) -> Option<WorkArea> {
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFO};
    unsafe {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        GetMonitorInfoW(monitor, &mut info)
            .as_bool()
            .then_some(WorkArea {
                left: info.rcWork.left,
                top: info.rcWork.top,
                right: info.rcWork.right,
                bottom: info.rcWork.bottom,
            })
    }
}

#[cfg(windows)]
pub fn monitor_of_pet(window: &slint::Window) -> WorkArea {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{
        Foundation::HWND,
        Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromWindow},
    };
    unsafe {
        let handle = window.window_handle();
        if let Some(hwnd) = handle
            .window_handle()
            .ok()
            .and_then(|handle| match handle.as_raw() {
                RawWindowHandle::Win32(value) => {
                    Some(HWND(value.hwnd.get() as *mut std::ffi::c_void))
                }
                _ => None,
            })
            && let Some(area) =
                work_area_from_monitor(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST))
        {
            return area;
        }
    }
    fallback_work_area()
}

#[cfg(not(windows))]
pub fn monitor_of_pet(window: &slint::Window) -> WorkArea {
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
        .unwrap_or_else(fallback_work_area)
}

#[cfg(windows)]
pub fn monitor_of_foreground_window(window: &slint::Window) -> WorkArea {
    use windows::Win32::{
        Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromWindow},
        UI::WindowsAndMessaging::GetForegroundWindow,
    };
    unsafe {
        work_area_from_monitor(MonitorFromWindow(
            GetForegroundWindow(),
            MONITOR_DEFAULTTONEAREST,
        ))
        .unwrap_or_else(|| monitor_of_pet(window))
    }
}

#[cfg(not(windows))]
pub fn monitor_of_foreground_window(window: &slint::Window) -> WorkArea {
    monitor_of_pet(window)
}

#[cfg(windows)]
pub fn monitor_of_cursor(window: &slint::Window) -> WorkArea {
    use windows::Win32::{
        Foundation::POINT,
        Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromPoint},
        UI::WindowsAndMessaging::GetCursorPos,
    };
    let mut point = POINT::default();
    unsafe {
        if GetCursorPos(&mut point).is_ok() {
            return work_area_from_monitor(MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST))
                .unwrap_or_else(|| monitor_of_pet(window));
        }
    }
    monitor_of_pet(window)
}

#[cfg(not(windows))]
pub fn monitor_of_cursor(window: &slint::Window) -> WorkArea {
    monitor_of_pet(window)
}

pub fn active_work_area(window: &slint::Window) -> WorkArea {
    monitor_of_pet(window)
}

fn fallback_work_area() -> WorkArea {
    WorkArea {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    }
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
pub fn cursor_position() -> Option<(i32, i32)> {
    use windows::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point).ok().map(|()| (point.x, point.y)) }
}

#[cfg(not(windows))]
pub fn cursor_position() -> Option<(i32, i32)> {
    None
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
}
