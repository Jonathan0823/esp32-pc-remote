//! Renderer-free Windows message host, also used to activate an existing instance.

use std::sync::atomic::{AtomicBool, Ordering};
use windows::{
    Win32::{
        Foundation::{ERROR_CLASS_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, WPARAM},
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, FindWindowW,
            GetMessageW, MB_ICONERROR, MB_OK, MSG, MessageBoxW, PostMessageW, PostThreadMessageW,
            RegisterClassW, SW_RESTORE, SetForegroundWindow, ShowWindow, TranslateMessage,
            WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WNDCLASSW,
        },
    },
    core::{PCWSTR, w},
};

const HOST_CLASS: PCWSTR = w!("Esp32MediaDaemon.TrayHost");
const OPEN_SETTINGS: u32 = WM_APP + 1;
const WAKE: u32 = WM_APP + 2;
static OPEN_REQUESTED: AtomicBool = AtomicBool::new(false);

pub struct MessageHost(HWND);

impl MessageHost {
    pub fn new() -> windows::core::Result<Self> {
        let instance = unsafe { GetModuleHandleW(None)? }.into();
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: HOST_CLASS,
            ..Default::default()
        };
        if unsafe { RegisterClassW(&class) } == 0
            && unsafe { GetLastError() } != ERROR_CLASS_ALREADY_EXISTS
        {
            return Err(windows::core::Error::from_win32());
        }
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                HOST_CLASS,
                w!("ESP32 Media Daemon Host"),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance),
                None,
            )
            .map(Self)
        }
    }
}

impl Drop for MessageHost {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == OPEN_SETTINGS {
        OPEN_REQUESTED.store(true, Ordering::Release);
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

pub fn take_open_request() -> bool {
    OPEN_REQUESTED.swap(false, Ordering::AcqRel)
}

pub fn activate_existing_instance() {
    if let Ok(window) = unsafe { FindWindowW(HOST_CLASS, PCWSTR::null()) } {
        let _ = unsafe { PostMessageW(Some(window), OPEN_SETTINGS, WPARAM(0), LPARAM(0)) };
    }
}

pub fn current_thread_id() -> u32 {
    unsafe { GetCurrentThreadId() }
}

pub fn wake_thread(thread_id: u32) {
    let _ = unsafe { PostThreadMessageW(thread_id, WAKE, WPARAM(0), LPARAM(0)) };
}

pub fn reveal_settings() {
    if let Ok(window) = unsafe { FindWindowW(PCWSTR::null(), w!("ESP32 Media Daemon")) } {
        unsafe {
            let _ = ShowWindow(window, SW_RESTORE);
            let _ = SetForegroundWindow(window);
        }
    }
}

pub fn dispatch_message() -> windows::core::Result<bool> {
    let mut message = MSG::default();
    match unsafe { GetMessageW(&mut message, None, 0, 0) }.0 {
        -1 => Err(windows::core::Error::from_win32()),
        0 => Ok(false),
        _ => {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            Ok(true)
        }
    }
}

pub fn show_error(message: &str) {
    let message: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = unsafe {
        MessageBoxW(
            None,
            PCWSTR::from_raw(message.as_ptr()),
            w!("ESP32 Media Daemon"),
            MB_OK | MB_ICONERROR,
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reopen_is_delivered_without_a_settings_window_or_renderer() {
        let host = MessageHost::new().unwrap();
        assert!(!take_open_request());
        unsafe { PostMessageW(Some(host.0), OPEN_SETTINGS, WPARAM(0), LPARAM(0)) }.unwrap();
        for _ in 0..64 {
            assert!(dispatch_message().unwrap());
            if take_open_request() {
                assert!(!take_open_request());
                return;
            }
        }
        panic!("host did not dispatch the settings activation request");
    }
}
