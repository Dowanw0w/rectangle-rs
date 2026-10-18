//! Operating system backends for rectangle-rs.
//!
//! Only the platform specific glue lives here; every decision about *where* a
//! window should go is made in `rectangle-core`, which keeps this crate small and
//! reviewable.
//!
//! Supported platforms:
//!
//! * Windows: Win32 (`SetWindowPos`, monitor enumeration, per-monitor DPI).
//! * macOS: the accessibility API (`AXUIElement`), which requires the user to
//!   grant the Accessibility permission.
//!
//! On anything else [`System`] returns [`rectangle_core::PlatformError::Unsupported`]
//! so that `cargo test` still works there.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod unsupported;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
pub use macos::System;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub use unsupported::System;
#[cfg(target_os = "windows")]
pub use windows::System;

/// True when this build can manipulate other applications' windows.
pub const fn is_supported() -> bool {
    cfg!(any(target_os = "windows", target_os = "macos"))
}

/// Create the backend for the current platform.
pub fn system() -> System {
    System::new()
}

/// Open the OS privacy pane that has to be unlocked before windows can be moved.
///
/// Returns `false` when the platform needs no such permission.
pub fn open_permission_settings() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::open_accessibility_settings()
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}
