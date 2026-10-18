//! Small OS integration helpers used by the tray menu.

use std::path::Path;

/// Reveal a file in the platform file manager.
pub fn reveal_file(path: &Path) -> bool {
    let shown = path.display().to_string();

    #[cfg(target_os = "windows")]
    {
        // `explorer.exe /select,<path>` opens the containing folder with the file
        // selected; explorer parses the command line itself, so keep it one arg.
        std::process::Command::new("explorer.exe")
            .arg(format!("/select,{shown}"))
            .spawn()
            .is_ok()
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(shown)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = shown;
        false
    }
}

/// Open a file with its default application (used for `config.toml`).
pub fn open_file(path: &Path) -> bool {
    let shown = path.display().to_string();

    #[cfg(target_os = "windows")]
    {
        // `explorer.exe <file>` hands the file to its default handler; if no
        // association exists Windows shows the "open with" dialog.
        std::process::Command::new("explorer.exe")
            .arg(&shown)
            .spawn()
            .is_ok()
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&shown)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = shown;
        false
    }
}
