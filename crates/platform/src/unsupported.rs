//! Placeholder backend for platforms rectangle-rs does not support yet.
//!
//! It keeps `cargo build`/`cargo test` working on Linux so the pure logic crates
//! and their tests can run in CI.

use rectangle_core::{Displays, Permission, PlatformError, Rect, WindowId, WindowSystem};

pub struct System;

impl Default for System {
    fn default() -> Self {
        Self
    }
}

impl System {
    pub fn new() -> Self {
        Self
    }

    fn unsupported() -> PlatformError {
        PlatformError::Unsupported(format!(
            "rectangle-rs 目前只支持 Windows 和 macOS（当前平台 {}）",
            std::env::consts::OS
        ))
    }
}

impl WindowSystem for System {
    fn focused_window(&self) -> Result<WindowId, PlatformError> {
        Err(Self::unsupported())
    }

    fn window_frame(&self, _window: WindowId) -> Result<Rect, PlatformError> {
        Err(Self::unsupported())
    }

    fn set_window_frame(&self, _window: WindowId, _frame: Rect) -> Result<(), PlatformError> {
        Err(Self::unsupported())
    }

    fn set_minimized(&self, _window: WindowId, _minimized: bool) -> Result<(), PlatformError> {
        Err(Self::unsupported())
    }

    fn displays(&self) -> Result<Displays, PlatformError> {
        Err(Self::unsupported())
    }

    fn permission(&self) -> Permission {
        Permission::Missing(Self::unsupported().to_string())
    }

    fn platform_name(&self) -> &'static str {
        "unsupported"
    }
}
