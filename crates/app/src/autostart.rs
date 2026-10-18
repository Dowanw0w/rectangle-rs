//! Launch-at-login support.
//!
//! * Windows: a value in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
//!   (no admin rights needed, visible in Task Manager → Startup).
//! * macOS: a LaunchAgent in `~/Library/LaunchAgents` that runs the binary at
//!   login. `SMAppService` would need a signed bundle + entitlement, which an
//!   unsigned open source build does not have.

use std::path::PathBuf;

/// Registry value name (Windows).
#[cfg(windows)]
const ID: &str = "rectangle-rs";
/// LaunchAgent label (macOS).
#[cfg(target_os = "macos")]
const BUNDLE_LABEL: &str = "dev.rectangle-rs.app";

/// Is rectangle-rs registered to start at login?
pub fn is_enabled() -> bool {
    imp::is_enabled()
}

/// Enable or disable starting at login. Returns a human readable error.
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    imp::set_enabled(enabled)
}

/// Path of the currently running executable, used as the launch target.
fn current_exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|err| format!("无法获取自身路径：{err}"))
}

#[cfg(windows)]
mod imp {
    use std::mem::size_of;

    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW,
    };

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const ERROR_SUCCESS: u32 = 0;
    const ERROR_FILE_NOT_FOUND: u32 = 2;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn is_enabled() -> bool {
        let run_key = wide(RUN_KEY);
        let value_name = wide(super::ID);

        unsafe {
            let mut key: HKEY = std::ptr::null_mut();
            let status = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                run_key.as_ptr(),
                0,
                KEY_QUERY_VALUE,
                &mut key,
            );
            if status != ERROR_SUCCESS {
                return false;
            }

            let mut size: u32 = 0;
            let status = RegQueryValueExW(
                key,
                value_name.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size,
            );
            RegCloseKey(key);
            status == ERROR_SUCCESS && size > 0
        }
    }

    pub fn set_enabled(enabled: bool) -> Result<(), String> {
        let run_key = wide(RUN_KEY);
        let value_name = wide(super::ID);

        unsafe {
            let mut key: HKEY = std::ptr::null_mut();
            let status = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                run_key.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            );
            if status != ERROR_SUCCESS {
                return Err(format!("打开注册表 Run 项失败（错误码 {status}）"));
            }

            let result = if enabled {
                let exe = super::current_exe()?;
                let command = format!("\"{}\"", exe.display());
                let data: Vec<u16> = command.encode_utf16().chain(std::iter::once(0)).collect();
                let bytes = std::slice::from_raw_parts(
                    data.as_ptr().cast::<u8>(),
                    data.len() * size_of::<u16>(),
                );
                let status = RegSetValueExW(
                    key,
                    value_name.as_ptr(),
                    0,
                    REG_SZ,
                    bytes.as_ptr(),
                    bytes.len() as u32,
                );
                if status == ERROR_SUCCESS {
                    Ok(())
                } else {
                    Err(format!("写入注册表失败（错误码 {status}）"))
                }
            } else {
                let status = RegDeleteValueW(key, value_name.as_ptr());
                if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
                    Ok(())
                } else {
                    Err(format!("删除注册表值失败（错误码 {status}）"))
                }
            };

            RegCloseKey(key);
            result
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::path::PathBuf;

    fn plist_path() -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        Some(
            PathBuf::from(home)
                .join("Library")
                .join("LaunchAgents")
                .join(format!("{}.plist", super::BUNDLE_LABEL)),
        )
    }

    pub fn is_enabled() -> bool {
        plist_path().map(|path| path.exists()).unwrap_or(false)
    }

    pub fn set_enabled(enabled: bool) -> Result<(), String> {
        let Some(path) = plist_path() else {
            return Err("无法定位 ~/Library/LaunchAgents".to_string());
        };

        if !enabled {
            let _ = std::process::Command::new("launchctl")
                .args(["unload", "-w"])
                .arg(&path)
                .status();
            if path.exists() {
                std::fs::remove_file(&path)
                    .map_err(|err| format!("删除 LaunchAgent 失败：{err}"))?;
            }
            return Ok(());
        }

        let exe = super::current_exe()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("创建 LaunchAgents 目录失败：{err}"))?;
        }

        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
    <key>StandardErrorPath</key>
    <string>/tmp/{label}.err</string>
</dict>
</plist>
"#,
            label = super::BUNDLE_LABEL,
            exe = exe.display()
        );
        std::fs::write(&path, plist).map_err(|err| format!("写入 LaunchAgent 失败：{err}"))?;

        let status = std::process::Command::new("launchctl")
            .args(["load", "-w"])
            .arg(&path)
            .status()
            .map_err(|err| format!("执行 launchctl 失败：{err}"))?;
        if !status.success() {
            return Err(format!("launchctl load 返回 {status}"));
        }
        Ok(())
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    pub fn is_enabled() -> bool {
        false
    }

    pub fn set_enabled(_enabled: bool) -> Result<(), String> {
        Err("当前平台不支持设置开机自启动".to_string())
    }
}
