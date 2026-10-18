//! The tray resident daemon: everything that happens while rectangle-rs runs.
//!
//! There is no settings window. Configuration comes from `config.toml`, the tray
//! menu is the only UI, and every event is handled on the main thread (needed for
//! the accessibility API and the tray/status item).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use async_channel::Sender;
use rectangle_core::{Action, Config, Engine, Outcome, Permission, log_path};
use rectangle_platform::System;

use crate::autostart;
use crate::config_template::default_config_text;
use crate::events::AppEvent;
use crate::hotkeys::HotkeyRegistry;
use crate::log;
use crate::shell;
use crate::tray::Tray;

/// How often the "last foreign window" cache is refreshed.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

pub struct Runtime {
    pub engine: Engine<System>,
    pub config: Config,
    pub config_path: PathBuf,
    pub hotkeys: Option<HotkeyRegistry>,
    pub tray: Option<Tray>,
    pub events_tx: Sender<AppEvent>,
}

pub struct Daemon {
    engine: Engine<System>,
    config: Config,
    config_path: PathBuf,
    macos: bool,
    hotkeys: Option<HotkeyRegistry>,
    tray: Option<Tray>,
    hotkey_errors: BTreeMap<Action, String>,
    events_tx: Sender<AppEvent>,
    ticks: u64,
    permission_warned: bool,
    poll_thread: Option<std::thread::JoinHandle<()>>,
}

impl Daemon {
    pub fn new(runtime: Runtime) -> Self {
        let Runtime {
            engine,
            config,
            config_path,
            hotkeys,
            tray,
            events_tx,
        } = runtime;
        Self {
            engine,
            config,
            config_path,
            macos: cfg!(target_os = "macos"),
            hotkeys,
            tray,
            hotkey_errors: BTreeMap::new(),
            events_tx,
            ticks: 0,
            permission_warned: false,
            poll_thread: None,
        }
    }

    pub fn start(&mut self) {
        self.apply_hotkeys();
        self.log_status();
        if let Some(tray) = self.tray.as_ref() {
            tray.update_hotkeys(&self.config, self.macos);
            tray.set_status(&self.status_line());
        }
        self.spawn_poll_thread();
        log::info("daemon 已就绪（托盘菜单为唯一界面，配置见 config.toml）");
    }

    /// Handle one event. Returns `true` when the process should quit.
    pub fn handle_event(&mut self, event: AppEvent) -> bool {
        match event {
            AppEvent::Hotkey(action) | AppEvent::MenuAction(action) => self.perform(action),
            AppEvent::ToggleAutostart => self.toggle_autostart(),
            AppEvent::OpenConfigFile => self.open_config_file(),
            AppEvent::ReloadConfig => self.reload_config(),
            AppEvent::RevealLog => self.reveal_log(),
            AppEvent::ShowStatus => {
                log::info(self.status_line());
                if let Some(tray) = self.tray.as_ref() {
                    tray.set_status(&self.status_line());
                }
            }
            AppEvent::Quit => {
                log::info("从托盘退出");
                return true;
            }
            AppEvent::Poll => self.tick(),
        }
        false
    }

    fn tick(&mut self) {
        self.ticks = self.ticks.wrapping_add(1);

        match self.engine.permission() {
            Permission::Granted => {
                // Keep the remembered window fresh so tray actions target the
                // window the user is working in.
                self.engine.remember_foreground();
                self.permission_warned = false;
            }
            Permission::Missing(hint) => {
                if !self.permission_warned {
                    self.permission_warned = true;
                    log::error(format!(
                        "缺少系统权限，无法操作其他窗口：{hint}（macOS 需在 系统设置 → 隐私与安全性 → 辅助功能 中允许 rectangle-rs）"
                    ));
                }
            }
        }
    }

    fn perform(&mut self, action: Action) {
        self.engine.remember_foreground();
        let status = match self.engine.perform(action) {
            Ok(Outcome::Applied { frame, .. }) => match frame {
                Some(frame) => format!("{} · {frame}", action.label()),
                None => action.label().to_string(),
            },
            Ok(Outcome::NoOp { reason, .. }) => format!("{}：{reason}", action.label()),
            Err(err) => format!("{} 失败：{err}", action.label()),
        };
        log::info(status.clone());
        if let Some(tray) = self.tray.as_ref() {
            tray.set_status(&status);
        }
    }

    fn toggle_autostart(&mut self) {
        let enabled = autostart::is_enabled();
        let status = match autostart::set_enabled(!enabled) {
            Ok(()) => format!("开机自启动已{}", if enabled { "关闭" } else { "开启" }),
            Err(err) => format!("设置开机自启动失败：{err}"),
        };
        log::info(status.clone());

        let now_enabled = autostart::is_enabled();
        if let Some(tray) = self.tray.as_ref() {
            tray.set_autostart_checked(now_enabled);
            tray.set_status(&status);
        }
    }

    fn open_config_file(&mut self) {
        self.ensure_config_file();
        let opened = shell::open_file(&self.config_path);
        log::info(format!(
            "打开配置文件 {}：{opened}",
            self.config_path.display()
        ));
        if let Some(tray) = self.tray.as_ref() {
            tray.set_status(if opened {
                "已打开 config.toml，改完选择「重新加载配置」"
            } else {
                "打不开 config.toml，路径见日志"
            });
        }
    }

    fn reveal_log(&mut self) {
        let revealed = log_path().exists() && shell::reveal_file(&log_path());
        log::info(format!("打开日志文件：{revealed}"));
        if let Some(tray) = self.tray.as_ref() {
            tray.set_status(if revealed {
                "已打开日志文件"
            } else {
                "还没有日志文件"
            });
        }
    }

    fn ensure_config_file(&mut self) {
        if self.config_path.exists() {
            return;
        }
        if let Some(parent) = self.config_path.parent()
            && let Err(err) = std::fs::create_dir_all(parent)
        {
            log::error(format!("创建配置目录失败：{err}"));
            return;
        }
        match std::fs::write(&self.config_path, default_config_text()) {
            Ok(()) => log::info(format!("已生成默认配置文件 {}", self.config_path.display())),
            Err(err) => log::error(format!("写入默认配置失败：{err}")),
        }
    }

    fn reload_config(&mut self) {
        self.ensure_config_file();
        match Config::load(&self.config_path) {
            Ok((config, warnings)) => {
                for warning in &warnings {
                    log::warn(warning.clone());
                }
                self.config = config;
                self.apply_hotkeys();
                if let Some(tray) = self.tray.as_ref() {
                    tray.update_hotkeys(&self.config, self.macos);
                    tray.set_status(&self.status_line());
                }
                let status = if self.hotkey_errors.is_empty() {
                    format!("配置已重新加载（{} 项绑定）", self.config.bindings().len())
                } else {
                    format!(
                        "配置已重新加载，但有 {} 个热键注册失败",
                        self.hotkey_errors.len()
                    )
                };
                log::info(status.clone());
                if let Some(tray) = self.tray.as_ref() {
                    tray.set_status(&status);
                }
            }
            Err(err) => {
                log::error(format!("重新加载配置失败：{err}"));
                if let Some(tray) = self.tray.as_ref() {
                    tray.set_status(&format!("重新加载配置失败：{err}"));
                }
            }
        }
    }

    fn apply_hotkeys(&mut self) {
        self.hotkey_errors.clear();
        self.engine.set_config(self.config.clone());
        if let Some(registry) = self.hotkeys.as_mut() {
            self.hotkey_errors = registry.sync(&self.config).into_iter().collect();
        }
        for (action, reason) in &self.hotkey_errors {
            log::warn(format!("热键注册失败「{}」：{reason}", action.label()));
        }
    }

    fn status_line(&self) -> String {
        let registered = self
            .hotkeys
            .as_ref()
            .map(|registry| registry.registered_count())
            .unwrap_or(0);
        format!(
            "已注册 {registered} 个热键，{} 个失败",
            self.hotkey_errors.len()
        )
    }

    fn log_status(&self) {
        log::info(format!(
            "热键：已注册 {} 个，失败 {} 个",
            self.hotkeys
                .as_ref()
                .map(|registry| registry.registered_count())
                .unwrap_or(0),
            self.hotkey_errors.len()
        ));
        log::info(format!(
            "权限：{}",
            match self.engine.permission() {
                Permission::Granted => "正常".to_string(),
                Permission::Missing(hint) => format!("缺失（{hint}）"),
            }
        ));
        match self.engine.displays() {
            Ok(displays) => log::info(format!(
                "显示器：{}",
                displays
                    .items()
                    .iter()
                    .map(|display| display.label())
                    .collect::<Vec<_>>()
                    .join(" | ")
            )),
            Err(err) => log::warn(format!("枚举显示器失败：{err}")),
        }
        log::info(format!(
            "开机自启动：{}",
            if autostart::is_enabled() {
                "已开启"
            } else {
                "未开启"
            }
        ));
    }

    fn spawn_poll_thread(&mut self) {
        let tx = self.events_tx.clone();
        let spawned = std::thread::Builder::new()
            .name("rectangle-rs-poll".to_string())
            .spawn(move || {
                loop {
                    std::thread::sleep(POLL_INTERVAL);
                    if tx.send_blocking(AppEvent::Poll).is_err() {
                        break;
                    }
                }
            });
        match spawned {
            Ok(handle) => self.poll_thread = Some(handle),
            Err(err) => log::error(format!("启动轮询线程失败：{err}")),
        }
    }
}
