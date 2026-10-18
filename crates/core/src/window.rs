//! The narrow interface every platform backend has to implement.

use crate::display::Displays;
use crate::geometry::Rect;
use std::fmt;

/// Opaque, platform specific window handle.
///
/// The value is only meaningful for the [`WindowSystem`] instance that produced
/// it. Backends are free to invalidate it as soon as the window goes away; every
/// engine operation resolves a fresh handle from the currently focused window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

/// Everything that can go wrong while talking to the window server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlatformError {
    /// No window of another application is focused (e.g. the desktop or the
    /// settings window itself has focus and nothing was remembered yet).
    NoFocusedWindow,
    /// The window disappeared or the handle went stale.
    WindowUnavailable(String),
    /// The OS refused because accessibility/automation permission is missing.
    PermissionDenied(String),
    /// The action is not supported on this platform.
    Unsupported(String),
    /// Anything else the backend wants to report.
    Os(String),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformError::NoFocusedWindow => f.write_str("没有找到可操作的窗口"),
            PlatformError::WindowUnavailable(detail) => write!(f, "窗口不可用：{detail}"),
            PlatformError::PermissionDenied(detail) => write!(f, "缺少系统权限：{detail}"),
            PlatformError::Unsupported(detail) => write!(f, "当前平台不支持：{detail}"),
            PlatformError::Os(detail) => write!(f, "系统调用失败：{detail}"),
        }
    }
}

impl std::error::Error for PlatformError {}

/// Whether the backend is allowed to manipulate other applications' windows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Permission {
    Granted,
    Missing(String),
}

impl Permission {
    pub fn is_granted(&self) -> bool {
        matches!(self, Permission::Granted)
    }
}

/// The platform abstraction the engine and the UI work against.
pub trait WindowSystem {
    /// The window the user is currently working in.
    ///
    /// Must skip rect-rs' own windows so that pressing a hotkey while the
    /// settings window is focused keeps acting on the previous application.
    fn focused_window(&self) -> Result<WindowId, PlatformError>;

    /// Current frame of `window` in global screen coordinates.
    fn window_frame(&self, window: WindowId) -> Result<Rect, PlatformError>;

    /// Move and resize `window`.
    fn set_window_frame(&self, window: WindowId, frame: Rect) -> Result<(), PlatformError>;

    /// Minimize or restore `window`.
    fn set_minimized(&self, window: WindowId, minimized: bool) -> Result<(), PlatformError>;

    /// All displays, ordered left-to-right then top-to-bottom.
    fn displays(&self) -> Result<Displays, PlatformError>;

    /// Remember the currently focused foreign window, called right before
    /// rect-rs activates one of its own windows.
    fn remember_foreground(&self) {}

    /// Accessibility style permission state.
    fn permission(&self) -> Permission {
        Permission::Granted
    }

    /// `"windows"` or `"macos"`, for logs and the about panel.
    fn platform_name(&self) -> &'static str;
}

impl<T: WindowSystem + ?Sized> WindowSystem for &T {
    fn focused_window(&self) -> Result<WindowId, PlatformError> {
        (**self).focused_window()
    }

    fn window_frame(&self, window: WindowId) -> Result<Rect, PlatformError> {
        (**self).window_frame(window)
    }

    fn set_window_frame(&self, window: WindowId, frame: Rect) -> Result<(), PlatformError> {
        (**self).set_window_frame(window, frame)
    }

    fn set_minimized(&self, window: WindowId, minimized: bool) -> Result<(), PlatformError> {
        (**self).set_minimized(window, minimized)
    }

    fn displays(&self) -> Result<Displays, PlatformError> {
        (**self).displays()
    }

    fn remember_foreground(&self) {
        (**self).remember_foreground()
    }

    fn permission(&self) -> Permission {
        (**self).permission()
    }

    fn platform_name(&self) -> &'static str {
        (**self).platform_name()
    }
}
