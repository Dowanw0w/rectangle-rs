//! Minimal file logger, `log` crate sink and panic reporter.
//!
//! A GUI application has no console (especially on Windows, where the release
//! binary uses the `windows` subsystem), so everything that goes wrong has to end
//! up in a file:
//!
//! * our own messages via [`info`]/[`warn`]/[`error`],
//! * GPUI's internal `log::error!`/`log::warn!` messages via the [`log`] sink
//!   (this is how DirectX/rendering failures become visible),
//! * panics, including the exact source location, via [`install_panic_hook`].

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Rotate the log once it grows past this size.
const MAX_LOG_BYTES: u64 = 2 * 1_048_576;

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static FILE_LOGGER: FileLogger = FileLogger;

/// Sink for the `log` crate, i.e. for every dependency that reports problems.
struct FileLogger;

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Debug
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        write(
            record.level().as_str(),
            &format!("{}: {}", record.target(), record.args()),
        );
    }

    fn flush(&self) {}
}

/// Start writing to `path` and route the `log` crate here too.
pub fn init(path: impl Into<PathBuf>) {
    let path = path.into();
    let _ = LOG_PATH.set(path);
    // Ignore the error: another logger may already be installed in tests.
    let _ = log::set_logger(&FILE_LOGGER);
    log::set_max_level(log::LevelFilter::Debug);
}

/// Where the log is written; `None` before [`init`].
pub fn path() -> Option<&'static Path> {
    LOG_PATH.get().map(PathBuf::as_path)
}

pub fn info(message: impl AsRef<str>) {
    write("INFO", message.as_ref());
}

pub fn warn(message: impl AsRef<str>) {
    write("WARN", message.as_ref());
}

pub fn error(message: impl AsRef<str>) {
    write("ERROR", message.as_ref());
}

/// Report panics with their location before the process dies.
///
/// The release profile aborts on panic (panics must not unwind through the Win32
/// / AppKit callbacks), so without a hook the process would simply disappear.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|location| {
                format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            })
            .unwrap_or_else(|| "未知位置".to_string());

        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .map(|message| (*message).to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "未知 panic（payload 不可读）".to_string());

        let thread = std::thread::current()
            .name()
            .unwrap_or("<未命名线程>")
            .to_string();
        let log_hint = path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<未初始化>".to_string());

        write(
            "ERROR",
            &format!("PANIC（线程 {thread}）：{message} @ {location}"),
        );
        write("ERROR", &format!("日志文件：{log_hint}"));

        // Console builds (debug) still get something on stderr.
        eprintln!("rectangle-rs panic: {message} @ {location}");

        fatal_dialog(&format!(
            "rectangle-rs 崩溃了：\n\n{message}\n\n位置：{location}\n线程：{thread}\n\n详细日志：\n{log_hint}"
        ));
    }));
}

fn write(level: &str, message: &str) {
    let Some(path) = LOG_PATH.get() else {
        eprintln!("{level}: {message}");
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    rotate_if_needed(path);

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{} [{level}] {message}", timestamp());
    }
}

fn rotate_if_needed(path: &Path) {
    if let Ok(metadata) = std::fs::metadata(path)
        && metadata.len() > MAX_LOG_BYTES
    {
        let _ = std::fs::remove_file(path);
    }
}

/// Show a native error dialog so a crash is not a silent disappearance.
#[cfg(windows)]
fn fatal_dialog(message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};

    let text: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
    let caption: Vec<u16> = "rectangle-rs"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
fn fatal_dialog(_message: &str) {}

/// `YYYY-MM-DD HH:MM:SS` in UTC, without pulling in a date crate.
fn timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();

    let days = (seconds / 86_400) as i64;
    let time_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);

    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        time_of_day / 3_600,
        (time_of_day % 3_600) / 60,
        time_of_day % 60
    )
}

/// Howard Hinnant's `civil_from_days`.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    #[test]
    fn epoch_converts_to_1970() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn known_dates_convert() {
        // 2024-02-29, 2000-01-01, 2026-10-13
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(10_957), (2000, 1, 1));
        assert_eq!(civil_from_days(20_739), (2026, 10, 13));
    }
}
