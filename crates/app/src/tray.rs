//! System tray icon and menu.
//!
//! There is no settings window any more: the tray menu is the whole UI, the
//! configuration lives in `config.toml`.
//!
//! `tray-icon` has to be created on a thread with a live OS event loop (the main
//! thread here). Its menu lives in `muda`, whose clicks arrive on a crossbeam
//! channel that a dedicated thread forwards into the app channel.

use std::collections::HashMap;

use async_channel::Sender;
use rectangle_core::{Action, Category, Config};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::events::AppEvent;
use crate::icon;
use crate::log;

const MENU_ACTION_PREFIX: &str = "action:";
const MENU_TOGGLE_AUTOSTART: &str = "toggle-autostart";
const MENU_OPEN_CONFIG: &str = "open-config";
const MENU_RELOAD_CONFIG: &str = "reload-config";
const MENU_REVEAL_LOG: &str = "reveal-log";
const MENU_QUIT: &str = "quit";

pub struct Tray {
    /// Dropping the `TrayIcon` removes the icon, so it has to outlive startup.
    icon: TrayIcon,
    /// Kept so the hotkey hints can be refreshed after a config reload.
    action_items: HashMap<Action, MenuItem>,
    autostart_item: CheckMenuItem,
}

impl Tray {
    pub fn build(
        tx: Sender<AppEvent>,
        config: &Config,
        autostart_enabled: bool,
        macos: bool,
    ) -> Result<Self, String> {
        let menu = Menu::new();
        let mut action_items = HashMap::new();

        // Move / window / display actions in the main menu; the size-changing
        // tile actions live in a submenu so "move" stays the obvious default.
        for category in [Category::Move, Category::Window, Category::Display] {
            for action in Action::ALL
                .into_iter()
                .filter(|action| action.category() == category)
            {
                let item = MenuItem::with_id(
                    format!("{MENU_ACTION_PREFIX}{}", action.id()),
                    action_label(action, config, macos),
                    true,
                    None,
                );
                menu.append(&item).map_err(|err| err.to_string())?;
                action_items.insert(action, item);
            }
        }

        let submenu = Submenu::new(Category::Tile.label(), true);
        for action in Action::ALL
            .into_iter()
            .filter(|action| action.category() == Category::Tile)
        {
            let item = MenuItem::with_id(
                format!("{MENU_ACTION_PREFIX}{}", action.id()),
                action_label(action, config, macos),
                true,
                None,
            );
            submenu.append(&item).map_err(|err| err.to_string())?;
            action_items.insert(action, item);
        }
        menu.append(&submenu).map_err(|err| err.to_string())?;

        menu.append(&PredefinedMenuItem::separator())
            .map_err(|err| err.to_string())?;

        let autostart_item = CheckMenuItem::with_id(
            MENU_TOGGLE_AUTOSTART,
            "开机自启动",
            true,
            autostart_enabled,
            None,
        );
        menu.append(&autostart_item)
            .map_err(|err| err.to_string())?;

        let open_config = MenuItem::with_id(MENU_OPEN_CONFIG, "编辑配置文件", true, None);
        menu.append(&open_config).map_err(|err| err.to_string())?;
        let reload_config = MenuItem::with_id(MENU_RELOAD_CONFIG, "重新加载配置", true, None);
        menu.append(&reload_config).map_err(|err| err.to_string())?;

        menu.append(&PredefinedMenuItem::separator())
            .map_err(|err| err.to_string())?;
        let reveal_log = MenuItem::with_id(MENU_REVEAL_LOG, "打开日志文件", true, None);
        menu.append(&reveal_log).map_err(|err| err.to_string())?;
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|err| err.to_string())?;
        let quit = MenuItem::with_id(MENU_QUIT, "退出", true, None);
        menu.append(&quit).map_err(|err| err.to_string())?;

        let size = 32;
        let color = if macos { [0, 0, 0] } else { [236, 236, 240] };
        let icon = tray_icon::Icon::from_rgba(icon::rgba(size, color), size, size)
            .map_err(|err| err.to_string())?;

        let builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("rectangle-rs — 窗口贴边")
            .with_menu_on_left_click(true);

        // A template image makes macOS tint the glyph for light and dark menu
        // bars; on Windows the icon keeps its own colours.
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(icon);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(icon);

        let tray = builder.build().map_err(|err| err.to_string())?;

        spawn_menu_forwarder(tx.clone());
        spawn_click_forwarder(tx);

        Ok(Self {
            icon: tray,
            action_items,
            autostart_item,
        })
    }

    /// Show a short hint in the tray tooltip, e.g. the last action's result.
    pub fn set_status(&self, status: &str) {
        let tooltip = format!("rectangle-rs — {status}");
        self.icon.set_tooltip(Some(tooltip.as_str())).ok();
    }

    /// Refresh the hotkey hint next to every action.
    pub fn update_hotkeys(&self, config: &Config, macos: bool) {
        for (action, item) in &self.action_items {
            item.set_text(action_label(*action, config, macos));
        }
    }

    pub fn set_autostart_checked(&self, checked: bool) {
        self.autostart_item.set_checked(checked);
    }
}

fn action_label(action: Action, config: &Config, macos: bool) -> String {
    match config.hotkey(action) {
        Some(hotkey) => format!("{}　{}", action.label(), hotkey.display_string(macos)),
        None => format!("{}　（无热键）", action.label()),
    }
}

fn spawn_menu_forwarder(tx: Sender<AppEvent>) {
    let spawned = std::thread::Builder::new()
        .name("rectangle-rs-tray-menu".to_string())
        .spawn(move || {
            let receiver = MenuEvent::receiver();
            while let Ok(event) = receiver.recv() {
                let menu_id = event.id.0;
                let app_event = match menu_id.as_str() {
                    MENU_TOGGLE_AUTOSTART => AppEvent::ToggleAutostart,
                    MENU_OPEN_CONFIG => AppEvent::OpenConfigFile,
                    MENU_RELOAD_CONFIG => AppEvent::ReloadConfig,
                    MENU_REVEAL_LOG => AppEvent::RevealLog,
                    MENU_QUIT => AppEvent::Quit,
                    other => match other
                        .strip_prefix(MENU_ACTION_PREFIX)
                        .and_then(Action::from_id)
                    {
                        Some(action) => AppEvent::MenuAction(action),
                        None => continue,
                    },
                };
                if tx.send_blocking(app_event).is_err() {
                    break;
                }
            }
        });
    if let Err(err) = spawned {
        log::error(format!("启动托盘菜单线程失败：{err}"));
    }
}

fn spawn_click_forwarder(tx: Sender<AppEvent>) {
    let spawned = std::thread::Builder::new()
        .name("rectangle-rs-tray-click".to_string())
        .spawn(move || {
            let receiver = TrayIconEvent::receiver();
            while let Ok(event) = receiver.recv() {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                    && tx.send_blocking(AppEvent::ShowStatus).is_err()
                {
                    break;
                }
            }
        });
    if let Err(err) = spawned {
        log::error(format!("启动托盘点击线程失败：{err}"));
    }
}
