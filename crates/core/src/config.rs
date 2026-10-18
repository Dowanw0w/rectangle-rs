//! Configuration file handling.
//!
//! The file lives in the platform config directory:
//!
//! | OS      | Path                                                     |
//! |---------|----------------------------------------------------------|
//! | Windows | `%APPDATA%\rectangle-rs\config.toml`                     |
//! | macOS   | `~/Library/Application Support/rectangle-rs/config.toml` |
//! | Linux   | `$XDG_CONFIG_HOME/rectangle-rs/config.toml`              |
//!
//! `RECTANGLE_RS_CONFIG_DIR` overrides the directory (used by tests and for
//! portable installs).

use crate::action::Action;
use crate::hotkey::Hotkey;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const CONFIG_VERSION: u32 = 1;

/// Largest allowed gap between a window and the work area edge.
pub const MAX_MARGIN: f64 = 100.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// Gap in screen units kept between the window edges and the work area.
    margin: f64,
    hotkeys: BTreeMap<Action, Hotkey>,
}

impl Default for Config {
    fn default() -> Self {
        let hotkeys = Action::ALL
            .into_iter()
            .filter_map(|action| {
                let raw = action.default_hotkey()?;
                Hotkey::parse(raw).ok().map(|hotkey| (action, hotkey))
            })
            .collect();
        Self {
            margin: 0.0,
            hotkeys,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigError {
    Io(String),
    Parse(String),
    Serialize(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(detail) => write!(f, "读写配置文件失败：{detail}"),
            ConfigError::Parse(detail) => write!(f, "配置文件格式错误：{detail}"),
            ConfigError::Serialize(detail) => write!(f, "序列化配置失败：{detail}"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// A hotkey that is already in use by another action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotkeyConflict {
    pub action: Action,
}

impl std::fmt::Display for HotkeyConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "该热键已被「{}」占用", self.action.label())
    }
}

impl std::error::Error for HotkeyConflict {}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ConfigFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    margin: Option<f64>,
    #[serde(default)]
    hotkeys: BTreeMap<String, String>,
}

impl Config {
    pub fn margin(&self) -> f64 {
        self.margin
    }

    /// Clamped to `0..=MAX_MARGIN`.
    pub fn set_margin(&mut self, margin: f64) {
        self.margin = if margin.is_finite() {
            margin.clamp(0.0, MAX_MARGIN)
        } else {
            0.0
        };
    }

    pub fn hotkey(&self, action: Action) -> Option<Hotkey> {
        self.hotkeys.get(&action).copied()
    }

    pub fn hotkeys(&self) -> &BTreeMap<Action, Hotkey> {
        &self.hotkeys
    }

    /// Action already bound to `hotkey`, ignoring `for_action`.
    pub fn conflict_for(&self, hotkey: Hotkey, for_action: Action) -> Option<Action> {
        self.hotkeys
            .iter()
            .find(|(action, bound)| **action != for_action && **bound == hotkey)
            .map(|(action, _)| *action)
    }

    /// Bind `hotkey` to `action`, or unbind it with `None`.
    pub fn set_hotkey(
        &mut self,
        action: Action,
        hotkey: Option<Hotkey>,
    ) -> Result<(), HotkeyConflict> {
        match hotkey {
            None => {
                self.hotkeys.remove(&action);
                Ok(())
            }
            Some(hotkey) => {
                if let Some(other) = self.conflict_for(hotkey, action) {
                    return Err(HotkeyConflict { action: other });
                }
                self.hotkeys.insert(action, hotkey);
                Ok(())
            }
        }
    }

    /// Snapshot of `(action, hotkey)` pairs, in action order.
    pub fn bindings(&self) -> Vec<(Action, Hotkey)> {
        self.hotkeys
            .iter()
            .map(|(action, hotkey)| (*action, *hotkey))
            .collect()
    }

    /// Serialize the config.
    ///
    /// Every action is written, using `""` for actions without a binding: an
    /// *absent* key means "use the built-in default", so omitting a cleared
    /// binding would resurrect it on the next start.
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        let hotkeys = Action::ALL
            .into_iter()
            .map(|action| {
                let value = self
                    .hotkeys
                    .get(&action)
                    .map(Hotkey::config_string)
                    .unwrap_or_default();
                (action.id().to_string(), value)
            })
            .collect();

        let file = ConfigFile {
            version: Some(CONFIG_VERSION),
            margin: Some(self.margin),
            hotkeys,
        };
        toml::to_string_pretty(&file).map_err(|err| ConfigError::Serialize(err.to_string()))
    }

    /// Parse `config.toml`, returning the config plus human readable warnings for
    /// entries that had to be ignored or repaired.
    pub fn from_toml(input: &str) -> Result<(Self, Vec<String>), ConfigError> {
        let file: ConfigFile =
            toml::from_str(input).map_err(|err| ConfigError::Parse(err.to_string()))?;

        let mut warnings = Vec::new();
        let mut config = Config::default();

        if let Some(version) = file.version
            && version > CONFIG_VERSION
        {
            warnings.push(format!(
                "配置文件版本 {version} 高于本程序支持的 {CONFIG_VERSION}，未知选项会被忽略"
            ));
        }
        if let Some(margin) = file.margin {
            if !margin.is_finite() || !(0.0..=MAX_MARGIN).contains(&margin) {
                warnings.push(format!("边距 {margin} 超出 0..={MAX_MARGIN}，已自动调整"));
            }
            config.set_margin(margin);
        }
        for (raw_action, raw_hotkey) in file.hotkeys {
            let Some(action) = Action::from_id(&raw_action) else {
                warnings.push(format!("忽略未知动作 `{raw_action}`"));
                continue;
            };
            if raw_hotkey.trim().is_empty() {
                // Explicitly disabled binding.
                config.hotkeys.remove(&action);
                continue;
            }
            match Hotkey::parse(&raw_hotkey) {
                Ok(hotkey) => {
                    if let Some(other) = config.conflict_for(hotkey, action) {
                        warnings.push(format!(
                            "`{raw_hotkey}` 同时绑定给「{}」和「{}」，已保留前者",
                            other.label(),
                            action.label()
                        ));
                        continue;
                    }
                    config.hotkeys.insert(action, hotkey);
                }
                Err(err) => warnings.push(format!(
                    "「{}」的热键 `{raw_hotkey}` 无效（{err}），已使用默认值",
                    action.label()
                )),
            }
        }

        Ok((config, warnings))
    }

    /// Load from `path`; a missing file yields defaults.
    pub fn load(path: &Path) -> Result<(Self, Vec<String>), ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(contents) => Config::from_toml(&contents),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Ok((Config::default(), vec![]))
            }
            Err(err) => Err(ConfigError::Io(err.to_string())),
        }
    }

    /// Write to `path`, creating the parent directory.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| ConfigError::Io(err.to_string()))?;
        }
        let contents = self.to_toml()?;
        std::fs::write(path, contents).map_err(|err| ConfigError::Io(err.to_string()))
    }
}

/// Directory that holds `config.toml` and the log file.
pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("RECTANGLE_RS_CONFIG_DIR") {
        return PathBuf::from(dir);
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("rectangle-rs");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("rectangle-rs");
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("rectangle-rs");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".config").join("rectangle-rs");
        }
    }

    PathBuf::from(".").join("rectangle-rs")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn log_path() -> PathBuf {
    config_dir().join("rectangle-rs.log")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_bind_exactly_the_actions_with_a_default_hotkey() {
        let config = Config::default();
        assert_eq!(config.margin(), 0.0);
        for action in Action::ALL {
            match action.default_hotkey() {
                Some(raw) => {
                    let hotkey = config.hotkey(action);
                    assert!(hotkey.is_some(), "{} has no default hotkey", action.id());
                    assert_eq!(
                        hotkey.unwrap().config_string(),
                        raw,
                        "{} default hotkey mismatch",
                        action.id()
                    );
                }
                None => assert_eq!(
                    config.hotkey(action),
                    None,
                    "{} must be unbound by default",
                    action.id()
                ),
            }
        }
        assert_eq!(config.bindings().len(), 14);
    }

    #[test]
    fn toml_round_trip() {
        let mut config = Config::default();
        config.set_margin(12.0);
        config
            .set_hotkey(
                Action::Center,
                Some(Hotkey::parse("platform+shift+c").unwrap()),
            )
            .unwrap();

        let toml = config.to_toml().unwrap();
        assert!(toml.contains("version = 1"));
        assert!(toml.contains("center = \"platform+shift+c\""));

        let (parsed, warnings) = Config::from_toml(&toml).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(parsed, config);
    }

    #[test]
    fn missing_fields_use_defaults() {
        let (config, warnings) = Config::from_toml("margin = 4\n").unwrap();
        assert!(warnings.is_empty());
        assert_eq!(config.margin(), 4.0);
        assert_eq!(config.bindings().len(), 14, "仅默认绑定的 14 个动作");
        assert_eq!(
            config,
            Config {
                margin: 4.0,
                ..Config::default()
            }
        );
    }

    #[test]
    fn empty_file_is_valid() {
        let (config, warnings) = Config::from_toml("").unwrap();
        assert!(warnings.is_empty());
        assert_eq!(config, Config::default());
    }

    #[test]
    fn unknown_action_and_bad_hotkey_warn() {
        let input = r#"
[hotkeys]
warp-drive = "ctrl+alt+w"
center = "not-a-hotkey"
"#;
        let (config, warnings) = Config::from_toml(input).unwrap();
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("warp-drive")),
            "{warnings:?}"
        );
        assert!(
            warnings.iter().any(|warning| warning.contains("居中")),
            "{warnings:?}"
        );
        // The broken entry keeps the default instead of silently disappearing.
        assert_eq!(
            config.hotkey(Action::Center).unwrap().config_string(),
            Action::Center.default_hotkey().unwrap()
        );
    }

    #[test]
    fn duplicate_hotkeys_keep_the_first() {
        let input = r#"
[hotkeys]
top-left = "ctrl+alt+u"
top-right = "ctrl+alt+u"
"#;
        let (config, warnings) = Config::from_toml(input).unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("左上角"));
        assert_eq!(
            config.hotkey(Action::TopLeft).unwrap().config_string(),
            "ctrl+alt+u"
        );
        assert_ne!(
            config.hotkey(Action::TopRight).unwrap().config_string(),
            "ctrl+alt+u"
        );
    }

    #[test]
    fn empty_string_disables_a_binding() {
        let (config, _) = Config::from_toml("[hotkeys]\nminimize = \"\"\n").unwrap();
        assert_eq!(config.hotkey(Action::Minimize), None);

        // A cleared binding must be written back as `""`, otherwise it would fall
        // back to the built-in default on the next load.
        let toml = config.to_toml().unwrap();
        assert!(toml.contains("minimize = \"\""), "{toml}");
        let (reloaded, warnings) = Config::from_toml(&toml).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(reloaded.hotkey(Action::Minimize), None);
        assert_eq!(reloaded, config);
    }

    #[test]
    fn every_action_is_written_explicitly() {
        let toml = Config::default().to_toml().unwrap();
        for action in Action::ALL {
            assert!(
                toml.contains(&format!("{} = ", action.id())),
                "{} missing from {toml}",
                action.id()
            );
        }
    }

    #[test]
    fn set_hotkey_detects_conflicts() {
        let mut config = Config::default();
        let taken = config.hotkey(Action::TopLeft).unwrap();
        let err = config.set_hotkey(Action::Center, Some(taken)).unwrap_err();
        assert_eq!(err.action, Action::TopLeft);
        assert!(err.to_string().contains("左上角"));
    }

    #[test]
    fn set_hotkey_validates_and_clears() {
        let mut config = Config::default();
        config
            .set_hotkey(Action::Center, Some(Hotkey::parse("ctrl+alt+z").unwrap()))
            .unwrap();
        assert_eq!(
            config.hotkey(Action::Center).unwrap().config_string(),
            "ctrl+alt+z"
        );
        config.set_hotkey(Action::Center, None).unwrap();
        assert_eq!(config.hotkey(Action::Center), None);
    }

    #[test]
    fn margin_is_clamped() {
        let mut config = Config::default();
        config.set_margin(-5.0);
        assert_eq!(config.margin(), 0.0);
        config.set_margin(500.0);
        assert_eq!(config.margin(), MAX_MARGIN);
        config.set_margin(f64::NAN);
        assert_eq!(config.margin(), 0.0);
    }

    #[test]
    fn out_of_range_margin_warns() {
        let (config, warnings) = Config::from_toml("margin = 500\n").unwrap();
        assert_eq!(config.margin(), MAX_MARGIN);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn newer_version_warns_but_loads() {
        let (_, warnings) = Config::from_toml("version = 99\n").unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("99"));
    }

    #[test]
    fn broken_toml_is_an_error() {
        assert!(matches!(
            Config::from_toml("margin = "),
            Err(ConfigError::Parse(_))
        ));
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "rectangle-rs-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("config.toml");
        let mut config = Config::default();
        config.set_margin(6.0);
        config.save(&path).unwrap();

        let (loaded, warnings) = Config::load(&path).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(loaded.margin(), 6.0);

        // Missing file -> defaults, no error.
        let (defaults, warnings) = Config::load(&dir.join("nope.toml")).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(defaults, Config::default());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn config_paths_are_stable() {
        assert!(config_path().ends_with("config.toml"));
        assert!(log_path().ends_with("rectangle-rs.log"));
    }
}
