//! Hotkey model plus a parser for the `ctrl+alt+u` syntax used in `config.toml`.
//!
//! This module deliberately knows nothing about the platform hotkey APIs; the
//! GUI crate translates [`Hotkey`] into tauri's `global-hotkey` types.

use std::fmt;

/// Modifier keys.
///
/// `platform` is the OS specific "command" modifier: `⌘` on macOS, `Win` on
/// Windows. Keeping it separate from the others means a config file written on
/// macOS stays usable on Windows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Mods {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub platform: bool,
}

impl Mods {
    pub const fn new(control: bool, alt: bool, shift: bool, platform: bool) -> Self {
        Self {
            control,
            alt,
            shift,
            platform,
        }
    }

    pub fn is_empty(&self) -> bool {
        !(self.control || self.alt || self.shift || self.platform)
    }

    pub fn count(&self) -> usize {
        usize::from(self.control)
            + usize::from(self.alt)
            + usize::from(self.shift)
            + usize::from(self.platform)
    }

    /// Canonical modifier prefix used in `config.toml`.
    pub fn config_prefix(&self) -> String {
        let mut parts = Vec::new();
        if self.platform {
            parts.push("platform");
        }
        if self.control {
            parts.push("ctrl");
        }
        if self.alt {
            parts.push("alt");
        }
        if self.shift {
            parts.push("shift");
        }
        parts.join("+")
    }

    /// Human readable prefix for the current OS.
    pub fn display_prefix(&self, macos: bool) -> String {
        let mut parts = Vec::new();
        if macos {
            if self.control {
                parts.push("⌃".to_string());
            }
            if self.alt {
                parts.push("⌥".to_string());
            }
            if self.shift {
                parts.push("⇧".to_string());
            }
            if self.platform {
                parts.push("⌘".to_string());
            }
        } else {
            if self.control {
                parts.push("Ctrl".to_string());
            }
            if self.alt {
                parts.push("Alt".to_string());
            }
            if self.shift {
                parts.push("Shift".to_string());
            }
            if self.platform {
                parts.push("Win".to_string());
            }
        }
        parts.join(if macos { "" } else { "+" })
    }
}

/// The non-modifier key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    /// A printable key: `a`-`z`, `0`-`9`, or one of `` ` - = [ ] \ ; ' , . / ``.
    Char(char),
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Space,
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    /// `F1`-`F24`.
    Function(u8),
}

impl Key {
    /// Canonical name used in `config.toml`.
    pub fn config_name(self) -> String {
        match self {
            Key::Char(c) => c.to_string(),
            Key::ArrowUp => "up".into(),
            Key::ArrowDown => "down".into(),
            Key::ArrowLeft => "left".into(),
            Key::ArrowRight => "right".into(),
            Key::Space => "space".into(),
            Key::Enter => "enter".into(),
            Key::Tab => "tab".into(),
            Key::Escape => "escape".into(),
            Key::Backspace => "backspace".into(),
            Key::Delete => "delete".into(),
            Key::Home => "home".into(),
            Key::End => "end".into(),
            Key::PageUp => "pageup".into(),
            Key::PageDown => "pagedown".into(),
            Key::Function(n) => format!("f{n}"),
        }
    }

    /// Name as rendered in the UI.
    pub fn display_name(self) -> String {
        match self {
            Key::Char(c) => c.to_ascii_uppercase().to_string(),
            Key::ArrowUp => "↑".into(),
            Key::ArrowDown => "↓".into(),
            Key::ArrowLeft => "←".into(),
            Key::ArrowRight => "→".into(),
            Key::Space => "Space".into(),
            Key::Enter => "Enter".into(),
            Key::Tab => "Tab".into(),
            Key::Escape => "Esc".into(),
            Key::Backspace => "Backspace".into(),
            Key::Delete => "Del".into(),
            Key::Home => "Home".into(),
            Key::End => "End".into(),
            Key::PageUp => "PgUp".into(),
            Key::PageDown => "PgDn".into(),
            Key::Function(n) => format!("F{n}"),
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        let lowered = token.trim().to_ascii_lowercase();
        let key = match lowered.as_str() {
            "up" | "arrowup" => Key::ArrowUp,
            "down" | "arrowdown" => Key::ArrowDown,
            "left" | "arrowleft" => Key::ArrowLeft,
            "right" | "arrowright" => Key::ArrowRight,
            "space" | "spacebar" => Key::Space,
            "enter" | "return" => Key::Enter,
            "tab" => Key::Tab,
            "escape" | "esc" => Key::Escape,
            "backspace" => Key::Backspace,
            "delete" | "del" => Key::Delete,
            "home" => Key::Home,
            "end" => Key::End,
            "pageup" | "pgup" => Key::PageUp,
            "pagedown" | "pgdn" => Key::PageDown,
            "minus" => Key::Char('-'),
            "equal" | "equals" => Key::Char('='),
            "comma" => Key::Char(','),
            "period" | "dot" => Key::Char('.'),
            "slash" => Key::Char('/'),
            "backslash" => Key::Char('\\'),
            "semicolon" => Key::Char(';'),
            "quote" | "apostrophe" => Key::Char('\''),
            "backquote" | "grave" => Key::Char('`'),
            "bracketleft" => Key::Char('['),
            "bracketright" => Key::Char(']'),
            other => {
                if other.len() == 1 {
                    let c = other.chars().next()?;
                    if c.is_ascii_alphanumeric() || "`-=[]\\;',./".contains(c) {
                        return Some(Key::Char(c));
                    }
                    return None;
                }
                if let Some(rest) = other.strip_prefix('f') {
                    let n: u8 = rest.parse().ok()?;
                    return (1..=24).contains(&n).then_some(Key::Function(n));
                }
                return None;
            }
        };
        Some(key)
    }
}

/// A complete hotkey: one or more modifiers plus exactly one key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hotkey {
    pub mods: Mods,
    pub key: Key,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HotkeyError {
    Empty,
    MissingKey,
    MissingModifier,
    MultipleKeys(String),
    UnknownToken(String),
}

impl fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HotkeyError::Empty => f.write_str("热键为空"),
            HotkeyError::MissingKey => f.write_str("缺少主键，例如 ctrl+alt+u"),
            HotkeyError::MissingModifier => {
                f.write_str("至少需要一个修饰键（ctrl/alt/shift/platform）")
            }
            HotkeyError::MultipleKeys(token) => {
                write!(f, "只能有一个主键，多余的是 `{token}`")
            }
            HotkeyError::UnknownToken(token) => write!(f, "无法识别的按键 `{token}`"),
        }
    }
}

impl std::error::Error for HotkeyError {}

impl Hotkey {
    pub fn new(mods: Mods, key: Key) -> Self {
        Self { mods, key }
    }

    /// Parse `ctrl+alt+u`, `Alt+Shift+F5`, `cmd+;` …
    ///
    /// `+`, `-` and whitespace all work as separators, and modifiers may appear
    /// in any order (but must come before the key).
    pub fn parse(input: &str) -> Result<Self, HotkeyError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(HotkeyError::Empty);
        }

        let tokens: Vec<&str> = trimmed
            .split(['+', '-', ' ', '\t'])
            .filter(|token| !token.is_empty())
            .collect();
        if tokens.is_empty() {
            return Err(HotkeyError::Empty);
        }

        let mut mods = Mods::default();
        let mut key: Option<Key> = None;
        for token in tokens {
            match token.trim().to_ascii_lowercase().as_str() {
                "ctrl" | "control" | "ctl" => mods.control = true,
                "alt" | "opt" | "option" => mods.alt = true,
                "shift" => mods.shift = true,
                "cmd" | "command" | "super" | "win" | "windows" | "logo" | "meta" | "platform" => {
                    mods.platform = true
                }
                other => match Key::parse(other) {
                    Some(parsed) => {
                        if key.replace(parsed).is_some() {
                            return Err(HotkeyError::MultipleKeys(other.to_string()));
                        }
                    }
                    None => return Err(HotkeyError::UnknownToken(other.to_string())),
                },
            }
        }

        let key = key.ok_or(HotkeyError::MissingKey)?;
        if mods.is_empty() {
            return Err(HotkeyError::MissingModifier);
        }
        Ok(Self { mods, key })
    }

    /// Canonical representation written to `config.toml` and used as identity.
    pub fn config_string(&self) -> String {
        let prefix = self.mods.config_prefix();
        if prefix.is_empty() {
            self.key.config_name()
        } else {
            format!("{prefix}+{}", self.key.config_name())
        }
    }

    /// Representation for humans; uses `⌃⌥⇧⌘` glyphs on macOS.
    pub fn display_string(&self, macos: bool) -> String {
        let prefix = self.mods.display_prefix(macos);
        let key = self.key.display_name();
        if prefix.is_empty() {
            key
        } else if macos {
            format!("{prefix}{key}")
        } else {
            format!("{prefix}+{key}")
        }
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.config_string())
    }
}

impl std::str::FromStr for Hotkey {
    type Err = HotkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Hotkey::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_combination() {
        let hotkey = Hotkey::parse("ctrl+alt+u").unwrap();
        assert_eq!(
            hotkey,
            Hotkey::new(Mods::new(true, true, false, false), Key::Char('u'))
        );
        assert_eq!(hotkey.config_string(), "ctrl+alt+u");
    }

    #[test]
    fn modifier_order_does_not_matter() {
        let a = Hotkey::parse("alt+ctrl+shift+u").unwrap();
        let b = Hotkey::parse("shift+ctrl+alt+u").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.config_string(), "ctrl+alt+shift+u");
    }

    #[test]
    fn accepts_case_and_spacing_and_dashes() {
        for input in [
            "Ctrl+Alt+U",
            " ctrl + alt + u ",
            "ctrl-alt-u",
            "CONTROL+OPTION+U",
        ] {
            let hotkey = Hotkey::parse(input).unwrap_or_else(|e| panic!("{input}: {e}"));
            assert_eq!(hotkey.config_string(), "ctrl+alt+u");
        }
    }

    #[test]
    fn platform_modifier_aliases() {
        for input in ["cmd+u", "command+u", "super+u", "win+u", "meta+u"] {
            let hotkey = Hotkey::parse(input).unwrap();
            assert!(hotkey.mods.platform, "{input}");
            assert_eq!(hotkey.config_string(), "platform+u");
            // `meta` must not be treated as alt.
            assert!(!hotkey.mods.alt, "{input}");
        }
    }

    #[test]
    fn named_and_function_keys() {
        assert_eq!(Hotkey::parse("ctrl+alt+left").unwrap().key, Key::ArrowLeft);
        assert_eq!(Hotkey::parse("ctrl+alt+return").unwrap().key, Key::Enter);
        assert_eq!(
            Hotkey::parse("ctrl+alt+f12").unwrap().key,
            Key::Function(12)
        );
        assert_eq!(Hotkey::parse("ctrl+alt+minus").unwrap().key, Key::Char('-'));
        assert_eq!(Hotkey::parse("ctrl+alt+;").unwrap().key, Key::Char(';'));
        assert_eq!(
            Hotkey::parse("ctrl+alt+f0").unwrap_err(),
            HotkeyError::UnknownToken("f0".into())
        );
    }

    #[test]
    fn rejects_incomplete_input() {
        assert_eq!(Hotkey::parse("").unwrap_err(), HotkeyError::Empty);
        assert_eq!(Hotkey::parse("   ").unwrap_err(), HotkeyError::Empty);
        assert_eq!(
            Hotkey::parse("ctrl+alt").unwrap_err(),
            HotkeyError::MissingKey
        );
        assert_eq!(
            Hotkey::parse("u").unwrap_err(),
            HotkeyError::MissingModifier
        );
        assert_eq!(
            Hotkey::parse("ctrl+u+i").unwrap_err(),
            HotkeyError::MultipleKeys("i".into())
        );
        assert_eq!(
            Hotkey::parse("ctrl+alt+nope").unwrap_err(),
            HotkeyError::UnknownToken("nope".into())
        );
    }

    #[test]
    fn error_messages_are_human_readable() {
        assert!(HotkeyError::MissingModifier.to_string().contains("修饰键"));
        assert!(
            HotkeyError::UnknownToken("zz".into())
                .to_string()
                .contains("zz")
        );
    }

    #[test]
    fn display_strings_are_platform_aware() {
        let hotkey = Hotkey::parse("ctrl+alt+shift+u").unwrap();
        assert_eq!(hotkey.display_string(true), "⌃⌥⇧U");
        assert_eq!(hotkey.display_string(false), "Ctrl+Alt+Shift+U");

        let cmd = Hotkey::parse("cmd+space").unwrap();
        assert_eq!(cmd.display_string(true), "⌘Space");
        assert_eq!(cmd.display_string(false), "Win+Space");
    }

    #[test]
    fn config_round_trip() {
        for input in ["ctrl+alt+u", "ctrl+alt+shift+left", "platform+f5", "ctrl+;"] {
            let hotkey = Hotkey::parse(input).unwrap();
            let again = Hotkey::parse(&hotkey.config_string()).unwrap();
            assert_eq!(hotkey, again, "{input}");
        }
    }

    #[test]
    fn modifier_counting() {
        assert_eq!(Mods::new(true, true, false, false).count(), 2);
        assert!(Mods::default().is_empty());
        assert_eq!(
            Mods::new(false, false, false, true).config_prefix(),
            "platform"
        );
        assert_eq!(
            Mods::new(true, true, true, true).config_prefix(),
            "platform+ctrl+alt+shift"
        );
    }
}
