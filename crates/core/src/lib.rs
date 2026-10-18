//! Platform independent core of rectangle-rs: geometry, tiling, hotkeys,
//! configuration and the action engine.
//!
//! Nothing in this crate talks to an operating system, which is what makes the
//! window behaviour testable on every platform (including CI runners that cannot
//! build a GUI).

pub mod action;
pub mod config;
pub mod display;
pub mod engine;
pub mod geometry;
pub mod hotkey;
pub mod tiling;
pub mod window;

pub use action::{Action, Category};
pub use config::{
    CONFIG_VERSION, Config, ConfigError, HotkeyConflict, config_dir, config_path, log_path,
};
pub use display::{Display, DisplayId, Displays};
pub use engine::{Engine, EngineError, Outcome};
pub use geometry::{Point, Rect};
pub use hotkey::{Hotkey, HotkeyError, Key, Mods};
pub use tiling::{MoveAnchor, TileLayout};
pub use window::{Permission, PlatformError, WindowId, WindowSystem};
