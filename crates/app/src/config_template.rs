//! The commented `config.toml` written on first start.

use rectangle_core::{Action, CONFIG_VERSION, Category, Config};

/// Default config file content: every action with its built-in hotkey, grouped by
/// category and ready to be edited. Generated from [`Config::default`] so it
/// cannot drift apart from the code.
pub fn default_config_text() -> String {
    let config = Config::default();

    let mut text = String::new();
    text.push_str(
        "# rectangle-rs 配置\n\
         #\n\
         # 改完从托盘菜单选择「重新加载配置」即时生效，不需要重启程序。\n\
         #\n\
         # 热键写法：修饰键 + 主键，用 `+` 连接\n\
         #   修饰键：ctrl / alt / shift / platform（platform = macOS 的 ⌘、Windows 的 Win，\n\
         #           也可以写 cmd / command / super / win / meta）\n\
         #   主键：a-z、0-9、up/down/left/right、space、enter、tab、home/end、\n\
         #        pageup/pagedown、delete、f1-f24，以及 minus / equal / comma /\n\
         #        period / slash / backslash / semicolon / quote / backquote /\n\
         #        bracketleft / bracketright\n\
         #\n\
         # 禁用某个动作的热键：把值写成空字符串，例如 center = \"\"\n\
         # 整行删掉 = 使用程序内置的默认热键（注意：删行不是禁用）\n",
    );
    text.push_str(&format!("\nversion = {CONFIG_VERSION}\n"));
    text.push_str(
        "\n# 窗口与工作区边缘的间距（像素/点），范围 0..=100\n\
         # 只影响窗口位置，不会改变窗口尺寸\nmargin = 0\n",
    );
    text.push_str("\n[hotkeys]\n");

    for category in Category::ALL {
        text.push_str(&format!("\n# ---------- {} ----------\n", category.label()));
        if category == Category::Tile {
            text.push_str("# 会改变窗口尺寸，默认不绑定热键；需要就自己填一个\n");
        }
        for action in Action::ALL
            .into_iter()
            .filter(|action| action.category() == category)
        {
            let hotkey = config
                .hotkey(action)
                .map(|hotkey| hotkey.config_string())
                .unwrap_or_default();
            text.push_str(&format!(
                "# {}\n{} = \"{}\"\n",
                action.label(),
                action.id(),
                hotkey
            ));
        }
    }

    text
}

#[cfg(test)]
mod tests {
    use super::default_config_text;
    use rectangle_core::{Action, Config};

    #[test]
    fn template_is_valid_config() {
        let text = default_config_text();
        let (config, warnings) = Config::from_toml(&text).expect("模板必须是合法 TOML");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(config, Config::default());
    }

    #[test]
    fn template_lists_every_action() {
        let text = default_config_text();
        for action in Action::ALL {
            assert!(
                text.contains(&format!("{} = ", action.id())),
                "{}",
                action.id()
            );
        }
    }

    #[test]
    fn template_leaves_tile_actions_unbound() {
        let text = default_config_text();
        let (config, _) = Config::from_toml(&text).unwrap();
        for action in Action::ALL {
            if action.tile_layout().is_some() {
                assert_eq!(config.hotkey(action), None, "{}", action.id());
            }
        }
    }
}
