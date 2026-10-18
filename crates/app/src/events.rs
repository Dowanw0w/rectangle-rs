//! Events from the OS (global hotkeys, tray menu, tray clicks, polling) that are
//! handled on the main thread.

use rectangle_core::Action;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppEvent {
    /// A global hotkey fired.
    Hotkey(Action),
    /// A tray menu entry was clicked.
    MenuAction(Action),
    /// Toggle "start at login" and update the check mark.
    ToggleAutostart,
    /// Open `config.toml` in the default editor.
    OpenConfigFile,
    /// Re-read `config.toml` and re-register the hotkeys.
    ReloadConfig,
    /// Show the log file in the file manager.
    RevealLog,
    /// Tray icon clicked: refresh the tooltip / log a heartbeat.
    ShowStatus,
    /// Quit requested from the tray.
    Quit,
    /// Periodic tick: keeps the "last focused window" cache warm and notices
    /// permission changes.
    Poll,
}
