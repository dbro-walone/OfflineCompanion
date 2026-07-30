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

#[cfg(windows)]
pub fn hide_from_taskbar(window: &slint::Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_EX_TOOLWINDOW,
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
        let extended_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            extended_style | WS_EX_TOOLWINDOW.0 as isize,
        );
    }
}

#[cfg(not(windows))]
pub fn hide_from_taskbar(_window: &slint::Window) {}

#[cfg(windows)]
struct TrayCallbacks {
    on_toggle: Box<dyn Fn()>,
    on_quit: Box<dyn Fn()>,
}

#[cfg(windows)]
pub struct TrayHandle {
    hwnd: windows::Win32::Foundation::HWND,
    callbacks: *mut TrayCallbacks,
}

#[cfg(not(windows))]
pub struct TrayHandle;

#[cfg(windows)]
const TRAY_ICON_ID: u32 = 1;
#[cfg(windows)]
const TRAY_CALLBACK_MESSAGE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 1;
#[cfg(windows)]
const TRAY_TOGGLE_COMMAND: usize = 1;
#[cfg(windows)]
const TRAY_QUIT_COMMAND: usize = 2;

#[cfg(windows)]
unsafe extern "system" fn tray_window_proc(
    hwnd: windows::Win32::Foundation::HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::{
        Foundation::{LRESULT, POINT},
        UI::WindowsAndMessaging::{
            AppendMenuW, CREATESTRUCTW, CreatePopupMenu, DefWindowProcW, DestroyMenu,
            GWLP_USERDATA, GetCursorPos, GetWindowLongPtrW, MF_STRING, SetForegroundWindow,
            SetWindowLongPtrW, TPM_RIGHTBUTTON, TrackPopupMenu, WM_COMMAND, WM_LBUTTONUP,
            WM_NCCREATE, WM_RBUTTONUP,
        },
    };
    use windows::core::w;

    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
            return LRESULT(1);
        }

        let callbacks = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const TrayCallbacks;
        if callbacks.is_null() {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }

        if message == TRAY_CALLBACK_MESSAGE {
            match lparam.0 as u32 {
                WM_LBUTTONUP => ((*callbacks).on_toggle)(),
                WM_RBUTTONUP => {
                    if let Ok(menu) = CreatePopupMenu() {
                        let _ =
                            AppendMenuW(menu, MF_STRING, TRAY_TOGGLE_COMMAND, w!("显示/隐藏 鸦影"));
                        let _ = AppendMenuW(menu, MF_STRING, TRAY_QUIT_COMMAND, w!("退出"));
                        let mut cursor = POINT::default();
                        if GetCursorPos(&mut cursor).is_ok() {
                            let _ = SetForegroundWindow(hwnd);
                            let _ = TrackPopupMenu(
                                menu,
                                TPM_RIGHTBUTTON,
                                cursor.x,
                                cursor.y,
                                None,
                                hwnd,
                                None,
                            );
                        }
                        let _ = DestroyMenu(menu);
                    }
                }
                _ => {}
            }
            return LRESULT(0);
        }

        if message == WM_COMMAND {
            match wparam.0 & 0xffff {
                TRAY_TOGGLE_COMMAND => ((*callbacks).on_toggle)(),
                TRAY_QUIT_COMMAND => ((*callbacks).on_quit)(),
                _ => {}
            }
            return LRESULT(0);
        }

        DefWindowProcW(hwnd, message, wparam, lparam)
    }
}

#[cfg(windows)]
pub fn create_tray(on_toggle: Box<dyn Fn()>, on_quit: Box<dyn Fn()>) -> TrayHandle {
    use windows::Win32::{
        Foundation::{HINSTANCE, HWND},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Shell::{NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NOTIFYICONDATAW, Shell_NotifyIconW},
            WindowsAndMessaging::{
                CreateWindowExW, HWND_MESSAGE, IDI_APPLICATION, LoadIconW, RegisterClassW,
                WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW,
            },
        },
    };
    use windows::core::w;

    unsafe {
        let Ok(module) = GetModuleHandleW(None) else {
            return TrayHandle {
                hwnd: HWND::default(),
                callbacks: std::ptr::null_mut(),
            };
        };
        let instance = HINSTANCE(module.0);
        let class = WNDCLASSW {
            hInstance: instance,
            lpszClassName: w!("OfflineCompanionTrayWindow"),
            lpfnWndProc: Some(tray_window_proc),
            ..Default::default()
        };
        RegisterClassW(&class);

        let callbacks = Box::into_raw(Box::new(TrayCallbacks { on_toggle, on_quit }));
        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class.lpszClassName,
            w!("鸦影"),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance),
            Some(callbacks.cast()),
        ) {
            Ok(hwnd) => hwnd,
            Err(_) => {
                drop(Box::from_raw(callbacks));
                return TrayHandle {
                    hwnd: HWND::default(),
                    callbacks: std::ptr::null_mut(),
                };
            }
        };

        let mut icon_data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: TRAY_CALLBACK_MESSAGE,
            hIcon: LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
            ..Default::default()
        };
        let tooltip = "鸦影".encode_utf16().chain(std::iter::once(0));
        for (target, value) in icon_data.szTip.iter_mut().zip(tooltip) {
            *target = value;
        }
        let _ = Shell_NotifyIconW(NIM_ADD, &icon_data);

        TrayHandle { hwnd, callbacks }
    }
}

#[cfg(not(windows))]
pub fn create_tray(_on_toggle: Box<dyn Fn()>, _on_quit: Box<dyn Fn()>) -> TrayHandle {
    TrayHandle
}

#[cfg(windows)]
impl Drop for TrayHandle {
    fn drop(&mut self) {
        use windows::Win32::UI::{
            Shell::{NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW},
            WindowsAndMessaging::DestroyWindow,
        };

        if self.hwnd.is_invalid() {
            return;
        }
        unsafe {
            let icon_data = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: TRAY_ICON_ID,
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_DELETE, &icon_data);
            let _ = DestroyWindow(self.hwnd);
            drop(Box::from_raw(self.callbacks));
        }
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
}
