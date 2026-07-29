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
        let hwnd = window
            .window_handle()
            .ok()
            .and_then(|handle| match handle.as_raw() {
                RawWindowHandle::Win32(value) => {
                    Some(HWND(value.hwnd.get() as *mut std::ffi::c_void))
                }
                _ => None,
            })
            .unwrap_or_else(GetForegroundWindow);
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
}
