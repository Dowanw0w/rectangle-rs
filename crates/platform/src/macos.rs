//! macOS backend.
//!
//! Windows are manipulated through the accessibility API (`AXUIElement`), which
//! is the only supported way to move other applications' windows. It requires
//! the user to grant rectangle-rs the Accessibility permission in
//! System Settings → Privacy & Security → Accessibility; until then every call
//! fails with `kAXErrorAPIDisabled` and this backend reports
//! [`PlatformError::PermissionDenied`].
//!
//! Coordinate space: the accessibility API uses points with the origin at the
//! top-left corner of the primary display and `y` growing downwards. AppKit
//! (`NSScreen`) uses a bottom-left origin, so display rects are flipped with
//! [`to_ax_rect`].

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr;

use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::string::CFString;
use core_foundation_sys::base::CFRelease;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSScreen, NSWorkspace};
use rectangle_core::{
    Display, DisplayId, Displays, Permission, PlatformError, Rect, WindowId, WindowSystem,
};

/// `AXValueType` for `CGPoint`.
const AX_VALUE_CGPOINT: u32 = 1;
/// `AXValueType` for `CGSize`.
const AX_VALUE_CGSIZE: u32 = 2;

const ATTR_FOCUSED_WINDOW: &str = "AXFocusedWindow";
const ATTR_POSITION: &str = "AXPosition";
const ATTR_SIZE: &str = "AXSize";
const ATTR_MINIMIZED: &str = "AXMinimized";
const ATTR_FULL_SCREEN: &str = "AXFullScreen";

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGPointF {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGSizeF {
    width: f64,
    height: f64,
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXUIElementCreateApplication(pid: libc::pid_t) -> *const c_void;
    fn AXUIElementCopyAttributeValue(
        element: *const c_void,
        attribute: *const c_void,
        value: *mut *const c_void,
    ) -> i32;
    fn AXUIElementSetAttributeValue(
        element: *const c_void,
        attribute: *const c_void,
        value: *const c_void,
    ) -> i32;
    fn AXValueCreate(the_type: u32, value_ptr: *const c_void) -> *const c_void;
    fn AXValueGetValue(value: *const c_void, the_type: u32, value_ptr: *mut c_void) -> u8;
}

/// Ownership wrapper for a `+1` CoreFoundation object.
struct CfOwned(*const c_void);

impl Drop for CfOwned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

impl CfOwned {
    fn into_raw(self) -> u64 {
        let raw = self.0 as usize as u64;
        std::mem::forget(self);
        raw
    }

    fn from_raw(raw: u64) -> Self {
        Self(raw as usize as *const c_void)
    }
}

fn copy_attribute(element: *const c_void, attribute: &str) -> Option<CfOwned> {
    let name = CFString::new(attribute);
    let mut value: *const c_void = ptr::null();
    let error = unsafe {
        AXUIElementCopyAttributeValue(element, name.as_concrete_TypeRef().cast(), &raw mut value)
    };
    if error == 0 && !value.is_null() {
        Some(CfOwned(value))
    } else {
        None
    }
}

fn set_attribute_raw(element: *const c_void, attribute: &str, value: *const c_void) -> i32 {
    let name = CFString::new(attribute);
    unsafe { AXUIElementSetAttributeValue(element, name.as_concrete_TypeRef().cast(), value) }
}

fn read_point(element: *const c_void) -> Option<CGPointF> {
    let value = copy_attribute(element, ATTR_POSITION)?;
    let mut point = CGPointF::default();
    let ok =
        unsafe { AXValueGetValue(value.0, AX_VALUE_CGPOINT, (&raw mut point).cast::<c_void>()) };
    (ok != 0).then_some(point)
}

fn read_size(element: *const c_void) -> Option<CGSizeF> {
    let value = copy_attribute(element, ATTR_SIZE)?;
    let mut size = CGSizeF::default();
    let ok = unsafe { AXValueGetValue(value.0, AX_VALUE_CGSIZE, (&raw mut size).cast::<c_void>()) };
    (ok != 0).then_some(size)
}

fn read_bool(element: *const c_void, attribute: &str) -> Option<bool> {
    let value = copy_attribute(element, attribute)?;
    let boolean: CFBoolean = unsafe { CFBoolean::wrap_under_get_rule(value.0.cast()) };
    Some(boolean.into())
}

fn write_point(element: *const c_void, point: CGPointF) -> Result<(), i32> {
    let value = unsafe { AXValueCreate(AX_VALUE_CGPOINT, (&raw const point).cast::<c_void>()) };
    if value.is_null() {
        return Err(-1);
    }
    let value = CfOwned(value);
    let error = set_attribute_raw(element, ATTR_POSITION, value.0);
    if error == 0 { Ok(()) } else { Err(error) }
}

fn write_size(element: *const c_void, size: CGSizeF) -> Result<(), i32> {
    let value = unsafe { AXValueCreate(AX_VALUE_CGSIZE, (&raw const size).cast::<c_void>()) };
    if value.is_null() {
        return Err(-1);
    }
    let value = CfOwned(value);
    let error = set_attribute_raw(element, ATTR_SIZE, value.0);
    if error == 0 { Ok(()) } else { Err(error) }
}

fn write_bool(element: *const c_void, attribute: &str, value: bool) -> Result<(), i32> {
    let boolean = CFBoolean::from(value);
    let error = set_attribute_raw(element, attribute, boolean.as_CFTypeRef());
    if error == 0 { Ok(()) } else { Err(error) }
}

/// Accessibility API error codes we care about (from `AXError.h`).
///
/// `kAXErrorAPIDisabled` is what every call returns until the user grants the
/// Accessibility permission, so it gets a dedicated, actionable message.
fn describe_ax_error(code: i32) -> PlatformError {
    match code {
        -25211 => PlatformError::PermissionDenied(
            "需要在「系统设置 → 隐私与安全性 → 辅助功能」中允许 rectangle-rs".to_string(),
        ),
        -25202 => PlatformError::WindowUnavailable("窗口已失效".to_string()),
        -25204 => PlatformError::WindowUnavailable("目标程序没有响应".to_string()),
        -25205 => PlatformError::WindowUnavailable("窗口不支持该属性".to_string()),
        -25206 => PlatformError::WindowUnavailable("窗口不支持该操作".to_string()),
        other => PlatformError::Os(format!("辅助功能 API 返回错误 {other}")),
    }
}

/// Flip an AppKit rect into accessibility (top-left origin) coordinates.
fn to_ax_rect(frame: objc2_foundation::NSRect, primary_top: f64) -> Rect {
    Rect::new(
        frame.origin.x,
        primary_top - (frame.origin.y + frame.size.height),
        frame.size.width,
        frame.size.height,
    )
}

struct Target {
    raw: u64,
    /// Kept so the accessibility element stays valid while cached.
    element: CfOwned,
}

pub struct System {
    target: RefCell<Option<Target>>,
}

impl Default for System {
    fn default() -> Self {
        Self::new()
    }
}

impl System {
    pub fn new() -> Self {
        Self {
            target: RefCell::new(None),
        }
    }

    fn cached_raw(&self) -> Option<u64> {
        self.target.borrow().as_ref().map(|target| target.raw)
    }

    fn cached_element(&self, window: WindowId) -> Result<*const c_void, PlatformError> {
        let target = self.target.borrow();
        match target.as_ref() {
            Some(target) if target.raw == window.0 => Ok(target.element.0),
            _ => Err(PlatformError::WindowUnavailable(
                "窗口已变化，请重新触发操作".to_string(),
            )),
        }
    }

    /// Frontmost application pid, or `None` when it cannot be determined.
    fn frontmost_pid() -> Option<libc::pid_t> {
        let workspace = NSWorkspace::sharedWorkspace();
        let app = workspace.frontmostApplication()?;
        Some(app.processIdentifier())
    }

    fn resolve(&self) -> Result<u64, PlatformError> {
        let own_pid = std::process::id() as libc::pid_t;
        let pid = Self::frontmost_pid().ok_or(PlatformError::NoFocusedWindow)?;

        // Our own settings window is frontmost: fall back to the window the user
        // was working in before.
        if pid == own_pid {
            return self.cached_raw().ok_or(PlatformError::NoFocusedWindow);
        }

        let app = unsafe { AXUIElementCreateApplication(pid) };
        if app.is_null() {
            return Err(PlatformError::Os("无法连接目标进程".to_string()));
        }
        let app = CfOwned(app);

        let window = copy_attribute(app.0, ATTR_FOCUSED_WINDOW).ok_or_else(|| {
            if unsafe { AXIsProcessTrusted() } != 0 {
                PlatformError::NoFocusedWindow
            } else {
                PlatformError::PermissionDenied(
                    "需要在「系统设置 → 隐私与安全性 → 辅助功能」中允许 rectangle-rs".to_string(),
                )
            }
        })?;

        if read_bool(window.0, ATTR_FULL_SCREEN) == Some(true) {
            return Err(PlatformError::Unsupported(
                "窗口处于全屏状态，请先退出全屏".to_string(),
            ));
        }

        let raw = window.into_raw();
        *self.target.borrow_mut() = Some(Target {
            raw,
            element: CfOwned::from_raw(raw),
        });
        Ok(raw)
    }
}

impl WindowSystem for System {
    fn focused_window(&self) -> Result<WindowId, PlatformError> {
        self.resolve().map(WindowId)
    }

    fn window_frame(&self, window: WindowId) -> Result<Rect, PlatformError> {
        let element = self.cached_element(window)?;
        let position = read_point(element)
            .ok_or_else(|| PlatformError::WindowUnavailable("读取窗口位置失败".to_string()))?;
        let size = read_size(element)
            .ok_or_else(|| PlatformError::WindowUnavailable("读取窗口尺寸失败".to_string()))?;
        Ok(Rect::new(position.x, position.y, size.width, size.height))
    }

    fn set_window_frame(&self, window: WindowId, frame: Rect) -> Result<(), PlatformError> {
        let element = self.cached_element(window)?;

        if read_bool(element, ATTR_MINIMIZED) == Some(true) {
            write_bool(element, ATTR_MINIMIZED, false).map_err(describe_ax_error)?;
        }

        let point = CGPointF {
            x: frame.x,
            y: frame.y,
        };
        let size = CGSizeF {
            width: frame.width.max(1.0),
            height: frame.height.max(1.0),
        };

        // Position -> size -> position: several applications clamp the first
        // size change based on the old screen edge, the second position pass
        // fixes the drift that causes.
        write_point(element, point).map_err(describe_ax_error)?;
        write_size(element, size).map_err(describe_ax_error)?;
        write_point(element, point).map_err(describe_ax_error)?;
        Ok(())
    }

    fn set_minimized(&self, window: WindowId, minimized: bool) -> Result<(), PlatformError> {
        let element = self.cached_element(window)?;
        write_bool(element, ATTR_MINIMIZED, minimized).map_err(describe_ax_error)
    }

    fn displays(&self) -> Result<Displays, PlatformError> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| PlatformError::Os("显示器枚举必须在主线程执行".to_string()))?;

        let screens = NSScreen::screens(mtm);
        let primary_top = screens
            .iter()
            .next()
            .map(|screen| {
                let frame = screen.frame();
                frame.origin.y + frame.size.height
            })
            .unwrap_or(0.0);

        let displays: Vec<Display> = screens
            .iter()
            .enumerate()
            .map(|(index, screen)| {
                let frame = screen.frame();
                let visible = screen.visibleFrame();
                Display::new(
                    DisplayId(index as u64 + 1),
                    screen.localizedName().to_string(),
                    to_ax_rect(frame, primary_top),
                    to_ax_rect(visible, primary_top),
                )
                .with_scale(screen.backingScaleFactor())
                .with_primary(index == 0)
            })
            .collect();

        if displays.is_empty() {
            return Err(PlatformError::Os("没有检测到显示器".to_string()));
        }
        Ok(Displays::new(displays))
    }

    fn remember_foreground(&self) {
        let own_pid = std::process::id() as libc::pid_t;
        if Self::frontmost_pid() == Some(own_pid) {
            return;
        }
        let _ = self.resolve();
    }

    fn permission(&self) -> Permission {
        if unsafe { AXIsProcessTrusted() } != 0 {
            Permission::Granted
        } else {
            Permission::Missing("未获得「辅助功能」权限，window 操作会被系统拒绝".to_string())
        }
    }

    fn platform_name(&self) -> &'static str {
        "macos"
    }
}

/// Open System Settings directly on the Accessibility privacy pane.
pub fn open_accessibility_settings() -> bool {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
