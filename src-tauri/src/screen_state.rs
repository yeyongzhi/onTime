//! Windows session and console-display state. A hidden native window receives
//! power/session broadcasts even when the Tauri webviews are hidden.
#[cfg(windows)]
mod windows {
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows_sys::Win32::{
        System::{
            Power::{POWERBROADCAST_SETTING, RegisterPowerSettingNotification},
            RemoteDesktop::{NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification},
            StationsAndDesktops::{CloseDesktop, DESKTOP_READOBJECTS, GetUserObjectInformationW, OpenInputDesktop, UOI_NAME},
            SystemServices::GUID_CONSOLE_DISPLAY_STATE,
        },
        UI::WindowsAndMessaging::{CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG, PBT_POWERSETTINGCHANGE, RegisterClassW, TranslateMessage, WNDCLASSW, WM_POWERBROADCAST, WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK, WTS_SESSION_UNLOCK, WS_OVERLAPPED, DEVICE_NOTIFY_WINDOW_HANDLE},
    };

    static DISPLAY_OFF: AtomicBool = AtomicBool::new(false);
    static SESSION_LOCKED: AtomicBool = AtomicBool::new(false);

    fn input_desktop_locked() -> Option<bool> {
        // The active desktop is Winlogon while locked and Default while usable.
        unsafe {
            let desktop = OpenInputDesktop(0, 0, DESKTOP_READOBJECTS);
            if desktop.is_null() { return None; }
            let mut name = [0_u16; 64];
            let mut needed = 0;
            let ok = GetUserObjectInformationW(desktop, UOI_NAME, name.as_mut_ptr().cast(), (name.len() * 2) as u32, &mut needed);
            CloseDesktop(desktop);
            if ok == 0 { return None; }
            let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
            Some(!String::from_utf16_lossy(&name[..end]).eq_ignore_ascii_case("Default"))
        }
    }

    unsafe extern "system" fn window_proc(hwnd: windows_sys::Win32::Foundation::HWND, message: u32, wparam: usize, lparam: isize) -> isize {
        match message {
            WM_WTSSESSION_CHANGE => match wparam as u32 {
                WTS_SESSION_LOCK => SESSION_LOCKED.store(true, Ordering::SeqCst),
                WTS_SESSION_UNLOCK => SESSION_LOCKED.store(false, Ordering::SeqCst),
                _ => {},
            },
            WM_POWERBROADCAST if wparam as u32 == PBT_POWERSETTINGCHANGE && lparam != 0 => {
                let setting = unsafe { &*(lparam as *const POWERBROADCAST_SETTING) };
                if setting.PowerSetting.data1 == GUID_CONSOLE_DISPLAY_STATE.data1
                    && setting.PowerSetting.data2 == GUID_CONSOLE_DISPLAY_STATE.data2
                    && setting.PowerSetting.data3 == GUID_CONSOLE_DISPLAY_STATE.data3
                    && setting.PowerSetting.data4 == GUID_CONSOLE_DISPLAY_STATE.data4
                    && setting.DataLength >= 4 {
                    let value = unsafe { std::ptr::read_unaligned(setting.Data.as_ptr().cast::<u32>()) };
                    // 0 = off, 1 = on, 2 = dimmed (still usable).
                    DISPLAY_OFF.store(value == 0, Ordering::SeqCst);
                }
            }
            _ => {},
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    pub fn start() {
        SESSION_LOCKED.store(input_desktop_locked().unwrap_or(false), Ordering::SeqCst);
        std::thread::spawn(|| unsafe {
            let class: Vec<u16> = "onTimeScreenState\0".encode_utf16().collect();
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(window_proc);
            wc.lpszClassName = class.as_ptr();
            if RegisterClassW(&wc) == 0 {
                log::warn!("无法注册屏幕状态监听窗口");
                return;
            }
            let hwnd = CreateWindowExW(0, class.as_ptr(), class.as_ptr(), WS_OVERLAPPED, 0, 0, 0, 0, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null());
            if hwnd.is_null() {
                log::warn!("无法创建屏幕状态监听窗口");
                return;
            }
            if WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) == 0 {
                log::warn!("无法注册 Windows 锁屏事件");
            }
            if RegisterPowerSettingNotification(hwnd, &GUID_CONSOLE_DISPLAY_STATE, DEVICE_NOTIFY_WINDOW_HANDLE) == 0 {
                log::warn!("无法注册 Windows 息屏事件");
            }
            let mut message: MSG = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        });
    }

    pub fn inactive() -> bool {
        // Recheck the input desktop in case the session event arrived before
        // the listener registered (or Windows dropped one).
        let locked = input_desktop_locked().unwrap_or_else(|| SESSION_LOCKED.load(Ordering::SeqCst));
        SESSION_LOCKED.store(locked, Ordering::SeqCst);
        locked || DISPLAY_OFF.load(Ordering::SeqCst)
    }
}

#[cfg(windows)]
pub use windows::{inactive, start};

#[cfg(not(windows))]
pub fn start() {}

#[cfg(not(windows))]
pub fn inactive() -> bool { false }
