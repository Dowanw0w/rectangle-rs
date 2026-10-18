//! Global hotkey registration.
//!
//! Both backends behind `global-hotkey` need a live OS event loop on the thread
//! that creates the manager: Win32 needs a message loop (GPUI's Windows backend
//! runs a standard `GetMessage`/`DispatchMessage` loop on the main thread) and
//! macOS delivers Carbon hotkey events through the main run loop that GPUI's
//! `NSApplication` provides. The manager is therefore created from the GPUI
//! launch callback, i.e. on the main thread.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_channel::Sender;
use global_hotkey::GlobalHotKeyEvent;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyManager, HotKeyState};
use rectangle_core::{Action, Config, Hotkey, Key};

use crate::events::AppEvent;
use crate::log;

/// Shared mapping from `global-hotkey` event ids to actions.
pub type ActionMap = Arc<Mutex<HashMap<u32, Action>>>;

pub struct HotkeyRegistry {
    manager: GlobalHotKeyManager,
    registered: Vec<HotKey>,
    action_map: ActionMap,
}

impl HotkeyRegistry {
    pub fn new() -> Result<Self, String> {
        let manager = GlobalHotKeyManager::new().map_err(|err| err.to_string())?;
        Ok(Self {
            manager,
            registered: Vec::new(),
            action_map: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn action_map(&self) -> ActionMap {
        Arc::clone(&self.action_map)
    }

    pub fn registered_count(&self) -> usize {
        self.registered.len()
    }

    /// Make the registered hotkeys match `config`.
    ///
    /// Returns one `(action, reason)` pair per hotkey that the OS refused, which
    /// happens when another application already owns the combination.
    pub fn sync(&mut self, config: &Config) -> Vec<(Action, String)> {
        for hotkey in self.registered.drain(..) {
            if let Err(err) = self.manager.unregister(hotkey) {
                log::warn(format!("注销热键 {:?} 失败：{err}", hotkey.key));
            }
        }
        if let Ok(mut map) = self.action_map.lock() {
            map.clear();
        }

        let mut failures = Vec::new();
        for action in Action::ALL {
            let Some(hotkey) = config.hotkey(action) else {
                continue;
            };
            let Some(global) = to_global_hotkey(hotkey) else {
                failures.push((action, "该按键组合无法注册为全局热键".to_string()));
                continue;
            };
            match self.manager.register(global) {
                Ok(()) => {
                    if let Ok(mut map) = self.action_map.lock() {
                        map.insert(global.id(), action);
                    }
                    self.registered.push(global);
                }
                Err(err) => {
                    let reason = match err {
                        global_hotkey::Error::AlreadyRegistered(_) => {
                            "已被其他程序占用".to_string()
                        }
                        other => other.to_string(),
                    };
                    log::warn(format!(
                        "注册热键 {} 失败：{reason}",
                        hotkey.display_string(cfg!(target_os = "macos"))
                    ));
                    failures.push((action, reason));
                }
            }
        }
        failures
    }
}

/// Translate a config hotkey into a `global-hotkey` binding.
pub fn to_global_hotkey(hotkey: Hotkey) -> Option<HotKey> {
    let mut mods = Modifiers::empty();
    if hotkey.mods.control {
        mods |= Modifiers::CONTROL;
    }
    if hotkey.mods.alt {
        mods |= Modifiers::ALT;
    }
    if hotkey.mods.shift {
        mods |= Modifiers::SHIFT;
    }
    if hotkey.mods.platform {
        // ⌘ on macOS, Win on Windows.
        mods |= Modifiers::META;
    }
    if mods.is_empty() {
        return None;
    }
    Some(HotKey::new(Some(mods), code_for(hotkey.key)?))
}

/// Map our layout independent key names onto keyboard scancode names.
pub fn code_for(key: Key) -> Option<Code> {
    Some(match key {
        Key::Char(c) => match c.to_ascii_lowercase() {
            'a' => Code::KeyA,
            'b' => Code::KeyB,
            'c' => Code::KeyC,
            'd' => Code::KeyD,
            'e' => Code::KeyE,
            'f' => Code::KeyF,
            'g' => Code::KeyG,
            'h' => Code::KeyH,
            'i' => Code::KeyI,
            'j' => Code::KeyJ,
            'k' => Code::KeyK,
            'l' => Code::KeyL,
            'm' => Code::KeyM,
            'n' => Code::KeyN,
            'o' => Code::KeyO,
            'p' => Code::KeyP,
            'q' => Code::KeyQ,
            'r' => Code::KeyR,
            's' => Code::KeyS,
            't' => Code::KeyT,
            'u' => Code::KeyU,
            'v' => Code::KeyV,
            'w' => Code::KeyW,
            'x' => Code::KeyX,
            'y' => Code::KeyY,
            'z' => Code::KeyZ,
            '0' => Code::Digit0,
            '1' => Code::Digit1,
            '2' => Code::Digit2,
            '3' => Code::Digit3,
            '4' => Code::Digit4,
            '5' => Code::Digit5,
            '6' => Code::Digit6,
            '7' => Code::Digit7,
            '8' => Code::Digit8,
            '9' => Code::Digit9,
            '-' => Code::Minus,
            '=' => Code::Equal,
            '[' => Code::BracketLeft,
            ']' => Code::BracketRight,
            '\\' => Code::Backslash,
            ';' => Code::Semicolon,
            '\'' => Code::Quote,
            ',' => Code::Comma,
            '.' => Code::Period,
            '/' => Code::Slash,
            '`' => Code::Backquote,
            _ => return None,
        },
        Key::ArrowUp => Code::ArrowUp,
        Key::ArrowDown => Code::ArrowDown,
        Key::ArrowLeft => Code::ArrowLeft,
        Key::ArrowRight => Code::ArrowRight,
        Key::Space => Code::Space,
        Key::Enter => Code::Enter,
        Key::Tab => Code::Tab,
        Key::Escape => Code::Escape,
        Key::Backspace => Code::Backspace,
        Key::Delete => Code::Delete,
        Key::Home => Code::Home,
        Key::End => Code::End,
        Key::PageUp => Code::PageUp,
        Key::PageDown => Code::PageDown,
        Key::Function(1) => Code::F1,
        Key::Function(2) => Code::F2,
        Key::Function(3) => Code::F3,
        Key::Function(4) => Code::F4,
        Key::Function(5) => Code::F5,
        Key::Function(6) => Code::F6,
        Key::Function(7) => Code::F7,
        Key::Function(8) => Code::F8,
        Key::Function(9) => Code::F9,
        Key::Function(10) => Code::F10,
        Key::Function(11) => Code::F11,
        Key::Function(12) => Code::F12,
        Key::Function(13) => Code::F13,
        Key::Function(14) => Code::F14,
        Key::Function(15) => Code::F15,
        Key::Function(16) => Code::F16,
        Key::Function(17) => Code::F17,
        Key::Function(18) => Code::F18,
        Key::Function(19) => Code::F19,
        Key::Function(20) => Code::F20,
        Key::Function(21) => Code::F21,
        Key::Function(22) => Code::F22,
        Key::Function(23) => Code::F23,
        Key::Function(24) => Code::F24,
        Key::Function(_) => return None,
    })
}

/// Forward OS hotkey events into the application channel.
///
/// Runs on its own thread: the platform hotkey manager hands events over a
/// channel, and blocking there keeps the UI thread free.
pub fn spawn_event_forwarder(action_map: ActionMap, tx: Sender<AppEvent>) {
    let spawned = std::thread::Builder::new()
        .name("rectangle-rs-hotkeys".to_string())
        .spawn(move || {
            let receiver = GlobalHotKeyEvent::receiver();
            while let Ok(event) = receiver.recv() {
                if event.state != HotKeyState::Pressed {
                    continue;
                }
                let action = match action_map.lock() {
                    Ok(map) => map.get(&event.id).copied(),
                    Err(_) => None,
                };
                if let Some(action) = action
                    && tx.send_blocking(AppEvent::Hotkey(action)).is_err()
                {
                    break;
                }
            }
        });

    if let Err(err) = spawned {
        log::error(format!("启动热键监听线程失败：{err}"));
    }
}
