//! Win32 backend.
//!
//! Coordinate space: physical pixels. The process declares per-monitor DPI
//! awareness v2 as early as possible, so Win32 never lies to us about pixel
//! coordinates on scaled displays.
//!
//! Frame handling: `GetWindowRect` includes the invisible resize borders that
//! DWM adds, `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` is what the
//! user actually sees. We report the visible frame and translate it back to a
//! window rect when applying a new frame, so a tiled window really fills half
//! the screen instead of overflowing it by ~16px.

use std::mem::size_of;
use std::sync::Mutex;

use rectangle_core::{
    Display, DisplayId, Displays, Permission, PlatformError, Rect, WindowId, WindowSystem,
};
use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MONITORINFOEXW, MonitorFromWindow,
};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindow, IsZoomed,
    MONITORINFOF_PRIMARY, SW_MINIMIZE, SW_RESTORE, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
    ShowWindow,
};
use windows_sys::core::BOOL;

/// Window handle stored as an integer so the backend stays `Send`.
type RawWindow = isize;

pub struct System {
    /// Last focused window that belonged to another process.
    ///
    /// Tray menu clicks and hotkeys pressed while the settings window is focused
    /// would otherwise resolve to rect-rs itself.
    last_foreign: Mutex<Option<RawWindow>>,
}

impl Default for System {
    fn default() -> Self {
        Self::new()
    }
}

impl System {
    pub fn new() -> Self {
        // Best effort: fails harmlessly when the manifest already declared it.
        unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        Self {
            last_foreign: Mutex::new(None),
        }
    }

    fn remember(&self, window: RawWindow) {
        if let Ok(mut slot) = self.last_foreign.lock() {
            *slot = Some(window);
        }
    }

    fn recall(&self) -> Option<RawWindow> {
        let window = *self.last_foreign.lock().ok()?;
        window.filter(|window| unsafe { IsWindow(hwnd(*window)) != 0 })
    }
}

fn hwnd(window: RawWindow) -> HWND {
    window as HWND
}

fn raw(hwnd: HWND) -> RawWindow {
    hwnd as RawWindow
}

fn to_rect(rect: &RECT) -> Rect {
    Rect::new(
        f64::from(rect.left),
        f64::from(rect.top),
        f64::from(rect.right - rect.left),
        f64::from(rect.bottom - rect.top),
    )
}

/// Window rect (including invisible borders) and visible rect.
fn window_rects(hwnd: HWND) -> Result<(RECT, RECT), PlatformError> {
    let mut window_rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut window_rect) } == 0 {
        return Err(PlatformError::WindowUnavailable(
            "GetWindowRect 失败".to_string(),
        ));
    }

    let mut visual_rect = RECT::default();
    let hr = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS as u32,
            (&raw mut visual_rect).cast(),
            size_of::<RECT>() as u32,
        )
    };
    if hr < 0 {
        visual_rect = window_rect;
    }
    Ok((window_rect, visual_rect))
}

fn monitor_display(monitor: HMONITOR) -> Option<Display> {
    if monitor.is_null() {
        return None;
    }
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    let ok = unsafe { GetMonitorInfoW(monitor, (&raw mut info.monitorInfo).cast::<MONITORINFO>()) };
    if ok == 0 {
        return None;
    }

    let name = String::from_utf16_lossy(&info.szDevice)
        .trim_end_matches('\0')
        .to_string();
    let is_primary = info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0;

    Some(
        Display::new(
            DisplayId(monitor as usize as u64),
            if name.is_empty() {
                "显示器".to_string()
            } else {
                name
            },
            to_rect(&info.monitorInfo.rcMonitor),
            to_rect(&info.monitorInfo.rcWork),
        )
        .with_primary(is_primary),
    )
}

unsafe extern "system" fn enum_monitor(
    monitor: HMONITOR,
    _hdc: HDC,
    _clip: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let displays = unsafe { &mut *(data as *mut Vec<Display>) };
    if let Some(display) = monitor_display(monitor) {
        displays.push(display);
    }
    1
}

impl WindowSystem for System {
    fn focused_window(&self) -> Result<WindowId, PlatformError> {
        let foreground = unsafe { GetForegroundWindow() };
        if !foreground.is_null() && unsafe { IsWindow(foreground) } != 0 {
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(foreground, &mut pid) };
            if pid != unsafe { GetCurrentProcessId() } {
                self.remember(raw(foreground));
                return Ok(WindowId(raw(foreground) as u64));
            }
        }

        self.recall()
            .map(|window| WindowId(window as u64))
            .ok_or(PlatformError::NoFocusedWindow)
    }

    fn window_frame(&self, window: WindowId) -> Result<Rect, PlatformError> {
        let hwnd = hwnd(window.0 as RawWindow);
        if hwnd.is_null() || unsafe { IsWindow(hwnd) } == 0 {
            return Err(PlatformError::WindowUnavailable(
                "窗口句柄已失效".to_string(),
            ));
        }
        let (_, visual) = window_rects(hwnd)?;
        Ok(to_rect(&visual))
    }

    fn set_window_frame(&self, window: WindowId, frame: Rect) -> Result<(), PlatformError> {
        let hwnd = hwnd(window.0 as RawWindow);
        if hwnd.is_null() || unsafe { IsWindow(hwnd) } == 0 {
            return Err(PlatformError::WindowUnavailable(
                "窗口句柄已失效".to_string(),
            ));
        }

        // Maximized/minimized windows ignore SetWindowPos, restore them first.
        let minimized = unsafe { IsIconic(hwnd) } != 0;
        let maximized = unsafe { IsZoomed(hwnd) } != 0;
        if minimized || maximized {
            unsafe { ShowWindow(hwnd, SW_RESTORE) };
        }

        let (window_rect, visual_rect) = window_rects(hwnd)?;
        // Invisible border, typically 7-8px per side on Windows 10/11.
        let left = visual_rect.left - window_rect.left;
        let top = visual_rect.top - window_rect.top;
        let right = window_rect.right - visual_rect.right;
        let bottom = window_rect.bottom - visual_rect.bottom;

        let width = frame.width.round() as i32;
        let height = frame.height.round() as i32;
        let x = frame.x.round() as i32 - left;
        let y = frame.y.round() as i32 - top;

        let ok = unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                x,
                y,
                (width + left + right).max(1),
                (height + top + bottom).max(1),
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        if ok == 0 {
            return Err(PlatformError::Os(format!(
                "SetWindowPos 失败（{}）",
                std::io::Error::last_os_error()
            )));
        }
        self.remember(raw(hwnd));
        Ok(())
    }

    fn set_minimized(&self, window: WindowId, minimized: bool) -> Result<(), PlatformError> {
        let hwnd = hwnd(window.0 as RawWindow);
        if hwnd.is_null() || unsafe { IsWindow(hwnd) } == 0 {
            return Err(PlatformError::WindowUnavailable(
                "窗口句柄已失效".to_string(),
            ));
        }
        unsafe {
            ShowWindow(hwnd, if minimized { SW_MINIMIZE } else { SW_RESTORE });
        }
        Ok(())
    }

    fn displays(&self) -> Result<Displays, PlatformError> {
        let mut collected: Vec<Display> = Vec::new();
        let ok = unsafe {
            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null(),
                Some(enum_monitor),
                (&raw mut collected) as LPARAM,
            )
        };
        if ok == 0 {
            return Err(PlatformError::Os("EnumDisplayMonitors 失败".to_string()));
        }
        if collected.is_empty() {
            // Fall back to the monitor that currently holds the focused window.
            let foreground = unsafe { GetForegroundWindow() };
            let monitor = unsafe { MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST) };
            if let Some(display) = monitor_display(monitor) {
                collected.push(display);
            }
        }
        Ok(Displays::new(collected))
    }

    fn remember_foreground(&self) {
        let foreground = unsafe { GetForegroundWindow() };
        if foreground.is_null() || unsafe { IsWindow(foreground) } == 0 {
            return;
        }
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(foreground, &mut pid) };
        if pid != unsafe { GetCurrentProcessId() } {
            self.remember(raw(foreground));
        }
    }

    fn permission(&self) -> Permission {
        Permission::Granted
    }

    fn platform_name(&self) -> &'static str {
        "windows"
    }
}
