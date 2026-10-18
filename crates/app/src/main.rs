#![cfg_attr(
    all(windows, not(debug_assertions), not(rectangle_rs_console)),
    windows_subsystem = "windows"
)]
//! rectangle-rs: window snapping for Windows and macOS.
//!
//! Tray resident, no settings window:
//!
//! * `config.toml` describes the hotkeys and the window gap,
//! * the tray menu triggers the actions, toggles start-at-login and reloads the
//!   configuration,
//! * `rectangle-platform` moves the windows (Win32 / accessibility API),
//! * `rectangle-core` decides where they go.
//!
//! A GUI API is still linked (GPUI) but only used for the platform event loop and
//! the foreground executor, which is what `global-hotkey` and `tray-icon` need on
//! both platforms. No window is ever created.
//!
//! Everything that can go wrong is written to `rectangle-rs.log` next to
//! `config.toml`, because a GUI subsystem binary has no console. Use
//! `RECTANGLE_RS_NO_TRAY=1` / `RECTANGLE_RS_NO_HOTKEYS=1` to bisect startup
//! problems, and build with `RUSTFLAGS="--cfg rectangle_rs_console"` to get a
//! console window.

mod autostart;
mod config_template;
mod daemon;
mod events;
mod hotkeys;
mod icon;
mod log;
mod shell;
mod tray;

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{App, Application, AsyncApp};
use rectangle_core::{Config, Engine, config_path, log_path};

use crate::config_template::default_config_text;
use crate::daemon::{Daemon, Runtime};
use crate::events::AppEvent;

fn main() {
    // Install logging and panic reporting before anything else can fail.
    log::init(log_path());
    log::install_panic_hook();

    let config_path = config_path();
    let macos = cfg!(target_os = "macos");
    log::info("================ rectangle-rs 启动 ================");
    log::info(format!(
        "版本 {}｜平台 {} {}｜配置 {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        config_path.display()
    ));

    if !rectangle_platform::is_supported() {
        log::error("当前平台不受支持：只能构建 Windows 和 macOS 版本");
    }

    // First run: drop a fully commented config file so there is something to edit.
    if !config_path.exists() {
        if let Some(parent) = config_path.parent()
            && let Err(err) = std::fs::create_dir_all(parent)
        {
            log::error(format!("创建配置目录失败：{err}"));
        }
        match std::fs::write(&config_path, default_config_text()) {
            Ok(()) => log::info(format!("已生成默认配置文件 {}", config_path.display())),
            Err(err) => log::error(format!("写入默认配置失败：{err}")),
        }
    }

    let (config, warnings) = match Config::load(&config_path) {
        Ok(loaded) => loaded,
        Err(err) => {
            log::error(format!("读取配置失败，改用默认配置：{err}"));
            (Config::default(), vec![err.to_string()])
        }
    };
    log::info(format!(
        "配置加载完成：边距 {}｜绑定 {} 项｜警告 {} 条",
        config.margin(),
        config.bindings().len(),
        warnings.len()
    ));
    for warning in &warnings {
        log::warn(warning.clone());
    }

    let skip_tray = env_flag("RECTANGLE_RS_NO_TRAY");
    let skip_hotkeys = env_flag("RECTANGLE_RS_NO_HOTKEYS");
    if skip_tray {
        log::warn("RECTANGLE_RS_NO_TRAY 已设置：跳过托盘图标");
    }
    if skip_hotkeys {
        log::warn("RECTANGLE_RS_NO_HOTKEYS 已设置：跳过热键注册");
    }

    log::info("启动平台事件循环");
    Application::new().run(move |cx: &mut App| {
        let (events_tx, events_rx) = async_channel::unbounded::<AppEvent>();

        // Both need the main thread's event loop: Win32 hotkeys want this thread's
        // message loop, macOS the main run loop (and the NSStatusItem/NSMenu).
        let hotkeys = if skip_hotkeys {
            None
        } else {
            match hotkeys::HotkeyRegistry::new() {
                Ok(registry) => {
                    log::info("全局热键管理器已创建");
                    hotkeys::spawn_event_forwarder(registry.action_map(), events_tx.clone());
                    Some(registry)
                }
                Err(err) => {
                    log::error(format!("初始化全局热键管理器失败：{err}"));
                    None
                }
            }
        };

        let tray = if skip_tray {
            None
        } else {
            match tray::Tray::build(events_tx.clone(), &config, autostart::is_enabled(), macos) {
                Ok(tray) => {
                    log::info("托盘图标已创建");
                    Some(tray)
                }
                Err(err) => {
                    log::error(format!("创建托盘图标失败：{err}"));
                    None
                }
            }
        };

        let runtime = Runtime {
            engine: Engine::new(rectangle_platform::system(), config.clone()),
            config,
            config_path,
            hotkeys,
            tray,
            events_tx,
        };

        let daemon = Rc::new(RefCell::new(Daemon::new(runtime)));
        daemon.borrow_mut().start();

        // Drain OS events on the foreground (main) thread.
        cx.spawn(async move |cx: &mut AsyncApp| {
            while let Ok(event) = events_rx.recv().await {
                let quit = daemon.borrow_mut().handle_event(event);
                if quit {
                    let _ = cx.update(|cx: &mut App| cx.quit());
                    break;
                }
            }
        })
        .detach();
    });

    // Reached only when the platform event loop ends.
    log::warn("事件循环已退出，进程结束");
}

fn env_flag(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty() && value != "0")
}
