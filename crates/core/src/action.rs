//! The set of window actions rect-rs can perform.
//!
//! Two families with different contracts:
//!
//! * **move** actions (`top-left`, `center`, …) never change the window size,
//! * **tile** actions (`tile-left-half`, …) resize the window to a quarter/half
//!   of the work area and are not bound to a hotkey by default.
//!
//! `maximize` / `restore` are the only other size-changing actions.

use crate::tiling::{MoveAnchor, TileLayout};
use std::fmt;

/// A user visible action, bound to a hotkey (in `config.toml`) and to a tray menu
/// entry.
///
/// The string ids are stable: they are the `[hotkeys]` keys in `config.toml` and
/// the ids of the tray menu entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    // Move only.
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Left,
    Right,
    Top,
    Bottom,
    Center,
    // Resize (opt-in, no default hotkey).
    TileTopLeft,
    TileTopRight,
    TileBottomLeft,
    TileBottomRight,
    TileLeftHalf,
    TileRightHalf,
    TileTopHalf,
    TileBottomHalf,
    // Window state.
    Maximize,
    Restore,
    Minimize,
    // Displays.
    NextDisplay,
    PreviousDisplay,
}

/// Grouping used by the tray menu and the config template.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Move,
    Tile,
    Window,
    Display,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::Move => "窗口移动（不改尺寸）",
            Category::Tile => "分屏（会改变尺寸）",
            Category::Window => "窗口状态",
            Category::Display => "多显示器",
        }
    }

    pub const ALL: [Category; 4] = [
        Category::Move,
        Category::Tile,
        Category::Window,
        Category::Display,
    ];
}

impl Action {
    /// Every action, in display order.
    pub const ALL: [Action; 22] = [
        Action::TopLeft,
        Action::TopRight,
        Action::BottomLeft,
        Action::BottomRight,
        Action::Left,
        Action::Right,
        Action::Top,
        Action::Bottom,
        Action::Center,
        Action::Maximize,
        Action::Restore,
        Action::Minimize,
        Action::NextDisplay,
        Action::PreviousDisplay,
        Action::TileTopLeft,
        Action::TileTopRight,
        Action::TileBottomLeft,
        Action::TileBottomRight,
        Action::TileLeftHalf,
        Action::TileRightHalf,
        Action::TileTopHalf,
        Action::TileBottomHalf,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Action::TopLeft => "top-left",
            Action::TopRight => "top-right",
            Action::BottomLeft => "bottom-left",
            Action::BottomRight => "bottom-right",
            Action::Left => "left",
            Action::Right => "right",
            Action::Top => "top",
            Action::Bottom => "bottom",
            Action::Center => "center",
            Action::TileTopLeft => "tile-top-left",
            Action::TileTopRight => "tile-top-right",
            Action::TileBottomLeft => "tile-bottom-left",
            Action::TileBottomRight => "tile-bottom-right",
            Action::TileLeftHalf => "tile-left-half",
            Action::TileRightHalf => "tile-right-half",
            Action::TileTopHalf => "tile-top-half",
            Action::TileBottomHalf => "tile-bottom-half",
            Action::Maximize => "maximize",
            Action::Restore => "restore",
            Action::Minimize => "minimize",
            Action::NextDisplay => "next-display",
            Action::PreviousDisplay => "previous-display",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        let id = id.trim().to_ascii_lowercase();
        let id = match id.as_str() {
            "topleft" | "top_left" | "upper-left" => "top-left",
            "topright" | "top_right" | "upper-right" => "top-right",
            "bottomleft" | "bottom_left" | "lower-left" => "bottom-left",
            "bottomright" | "bottom_right" | "lower-right" => "bottom-right",
            // Older configs called the edge moves "<edge>-half"; they keep working
            // and now mean "move to that edge" like the rest of the move family.
            "lefthalf" | "left_half" | "left-half" | "leftedge" => "left",
            "righthalf" | "right_half" | "right-half" | "rightedge" => "right",
            "tophalf" | "top_half" | "top-half" | "topedge" => "top",
            "bottomhalf" | "bottom_half" | "bottom-half" | "bottomedge" => "bottom",
            "unmaximize" | "un-maximize" | "un_maximize" | "restore-previous" => "restore",
            "minimise" => "minimize",
            "nextdisplay" | "next_display" | "next-monitor" => "next-display",
            "previousdisplay" | "previous_display" | "prev-display" => "previous-display",
            other => other,
        };
        Action::ALL.into_iter().find(|action| action.id() == id)
    }

    /// Chinese label shown in the tray menu.
    pub fn label(self) -> &'static str {
        match self {
            Action::TopLeft => "移动到左上角",
            Action::TopRight => "移动到右上角",
            Action::BottomLeft => "移动到左下角",
            Action::BottomRight => "移动到右下角",
            Action::Left => "靠左",
            Action::Right => "靠右",
            Action::Top => "靠上",
            Action::Bottom => "靠下",
            Action::Center => "居中",
            Action::TileTopLeft => "左上四分之一",
            Action::TileTopRight => "右上四分之一",
            Action::TileBottomLeft => "左下四分之一",
            Action::TileBottomRight => "右下四分之一",
            Action::TileLeftHalf => "左半屏",
            Action::TileRightHalf => "右半屏",
            Action::TileTopHalf => "上半屏",
            Action::TileBottomHalf => "下半屏",
            Action::Maximize => "最大化",
            Action::Restore => "取消最大化",
            Action::Minimize => "最小化",
            Action::NextDisplay => "移动到下一个显示器",
            Action::PreviousDisplay => "移动到上一个显示器",
        }
    }

    /// Short English identifier, used for tooltips and logs.
    pub fn label_en(self) -> &'static str {
        match self {
            Action::TopLeft => "Move to top left",
            Action::TopRight => "Move to top right",
            Action::BottomLeft => "Move to bottom left",
            Action::BottomRight => "Move to bottom right",
            Action::Left => "Move to left edge",
            Action::Right => "Move to right edge",
            Action::Top => "Move to top edge",
            Action::Bottom => "Move to bottom edge",
            Action::Center => "Center",
            Action::TileTopLeft => "Tile top left",
            Action::TileTopRight => "Tile top right",
            Action::TileBottomLeft => "Tile bottom left",
            Action::TileBottomRight => "Tile bottom right",
            Action::TileLeftHalf => "Tile left half",
            Action::TileRightHalf => "Tile right half",
            Action::TileTopHalf => "Tile top half",
            Action::TileBottomHalf => "Tile bottom half",
            Action::Maximize => "Maximize",
            Action::Restore => "Restore",
            Action::Minimize => "Minimize",
            Action::NextDisplay => "Next display",
            Action::PreviousDisplay => "Previous display",
        }
    }

    pub fn category(self) -> Category {
        match self {
            Action::TileTopLeft
            | Action::TileTopRight
            | Action::TileBottomLeft
            | Action::TileBottomRight
            | Action::TileLeftHalf
            | Action::TileRightHalf
            | Action::TileTopHalf
            | Action::TileBottomHalf => Category::Tile,
            Action::Maximize | Action::Restore | Action::Minimize => Category::Window,
            Action::NextDisplay | Action::PreviousDisplay => Category::Display,
            _ => Category::Move,
        }
    }

    /// Does this action only move the window (never resize it)?
    pub fn move_anchor(self) -> Option<MoveAnchor> {
        Some(match self {
            Action::TopLeft => MoveAnchor::TopLeft,
            Action::TopRight => MoveAnchor::TopRight,
            Action::BottomLeft => MoveAnchor::BottomLeft,
            Action::BottomRight => MoveAnchor::BottomRight,
            Action::Left => MoveAnchor::Left,
            Action::Right => MoveAnchor::Right,
            Action::Top => MoveAnchor::Top,
            Action::Bottom => MoveAnchor::Bottom,
            Action::Center => MoveAnchor::Center,
            _ => return None,
        })
    }

    /// Does this action resize the window to a quarter/half of the work area?
    pub fn tile_layout(self) -> Option<TileLayout> {
        Some(match self {
            Action::TileTopLeft => TileLayout::TopLeft,
            Action::TileTopRight => TileLayout::TopRight,
            Action::TileBottomLeft => TileLayout::BottomLeft,
            Action::TileBottomRight => TileLayout::BottomRight,
            Action::TileLeftHalf => TileLayout::LeftHalf,
            Action::TileRightHalf => TileLayout::RightHalf,
            Action::TileTopHalf => TileLayout::TopHalf,
            Action::TileBottomHalf => TileLayout::BottomHalf,
            _ => return None,
        })
    }

    /// Built-in hotkey, overridable in `config.toml`.
    ///
    /// `None` means "unbound by default" (all tile actions), which keeps the
    /// default setup free of size-changing shortcuts.
    pub fn default_hotkey(self) -> Option<&'static str> {
        Some(match self {
            Action::TopLeft => "ctrl+alt+u",
            Action::TopRight => "ctrl+alt+i",
            Action::BottomLeft => "ctrl+alt+j",
            Action::BottomRight => "ctrl+alt+k",
            Action::Left => "ctrl+alt+left",
            Action::Right => "ctrl+alt+right",
            Action::Top => "ctrl+alt+up",
            Action::Bottom => "ctrl+alt+down",
            Action::Center => "ctrl+alt+c",
            Action::Maximize => "ctrl+alt+f",
            Action::Restore => "ctrl+alt+r",
            Action::Minimize => "ctrl+alt+m",
            Action::NextDisplay => "ctrl+alt+n",
            Action::PreviousDisplay => "ctrl+alt+shift+n",
            Action::TileTopLeft
            | Action::TileTopRight
            | Action::TileBottomLeft
            | Action::TileBottomRight
            | Action::TileLeftHalf
            | Action::TileRightHalf
            | Action::TileTopHalf
            | Action::TileBottomHalf => return None,
        })
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn ids_are_unique_and_round_trip() {
        let mut seen = BTreeSet::new();
        for action in Action::ALL {
            assert!(seen.insert(action.id()), "duplicate id {}", action.id());
            assert_eq!(Action::from_id(action.id()), Some(action));
        }
        assert_eq!(seen.len(), Action::ALL.len());
    }

    #[test]
    fn aliases_resolve() {
        assert_eq!(Action::from_id("Unmaximize"), Some(Action::Restore));
        assert_eq!(Action::from_id(" top_left "), Some(Action::TopLeft));
        assert_eq!(Action::from_id("nope"), None);
        // Legacy edge ids keep working and now mean "move to that edge".
        assert_eq!(Action::from_id("left-half"), Some(Action::Left));
        assert_eq!(Action::from_id("top-half"), Some(Action::Top));
        // ... but the tiling variants are separate ids.
        assert_eq!(
            Action::from_id("tile-left-half"),
            Some(Action::TileLeftHalf)
        );
    }

    #[test]
    fn default_hotkeys_are_valid_and_unique() {
        let mut seen = BTreeSet::new();
        for action in Action::ALL {
            let Some(raw) = action.default_hotkey() else {
                continue;
            };
            let hotkey = crate::hotkey::Hotkey::parse(raw)
                .unwrap_or_else(|err| panic!("{}: {err}", action.id()));
            assert!(
                seen.insert(hotkey.config_string()),
                "duplicate default hotkey for {}",
                action.id()
            );
        }
        assert_eq!(seen.len(), 14, "14 actions are bound by default");
    }

    #[test]
    fn tile_actions_are_unbound_by_default() {
        for action in Action::ALL {
            if action.tile_layout().is_some() {
                assert_eq!(action.default_hotkey(), None, "{}", action.id());
            }
        }
    }

    #[test]
    fn move_and_tile_families_are_disjoint_and_cover_the_layout_actions() {
        let mut layout_actions = 0;
        for action in Action::ALL {
            let is_move = action.move_anchor().is_some();
            let is_tile = action.tile_layout().is_some();
            assert!(!(is_move && is_tile), "{}", action.id());
            if is_move || is_tile {
                layout_actions += 1;
            }
        }
        assert_eq!(layout_actions, 17, "9 move + 8 tile actions");
    }

    #[test]
    fn every_category_is_populated() {
        for category in Category::ALL {
            assert!(
                Action::ALL
                    .into_iter()
                    .any(|action| action.category() == category),
                "empty category {category:?}"
            );
        }
    }
}
