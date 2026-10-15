//! Native failures stay visible even when Windows runs without a console.
//! Logs contain only fixed categories and source coordinates, never error payloads,
//! project paths/content, command-line arguments, environment dumps, or credentials.
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const LOG_NAME: &str = "native-errors.log";
const PREVIOUS_LOG_NAME: &str = "native-errors.previous.log";
const MAX_LOG_BYTES: u64 = 64 * 1024;
const MAX_DETAIL_CHARS: usize = 2048;
static LOG_LOCK: Mutex<()> = Mutex::new(());
static REPORTING: AtomicBool = AtomicBool::new(false);

pub(crate) fn install() {
    std::panic::set_hook(Box::new(|info| {
        let location = info.location().map(|location| {
            let file = location
                .file()
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or("unknown");
            format!("source={file}:{}:{}", location.line(), location.column())
        });
        report_failure("panic", location.as_deref(), panic_detail(info.payload()));
    }));
}

pub(crate) fn report_eframe_error(error: &eframe::Error) {
    report_failure(error_category(error), None, Some(error));
}

fn panic_detail(payload: &(dyn std::any::Any + Send)) -> Option<&dyn fmt::Display> {
    if let Some(message) = payload.downcast_ref::<&str>() {
        Some(message)
    } else {
        payload
            .downcast_ref::<String>()
            .map(|message| message as &dyn fmt::Display)
    }
}

fn error_category(error: &eframe::Error) -> &'static str {
    match error {
        eframe::Error::AppCreation(_) => "app-creation",
        eframe::Error::Winit(_) => "window-system",
        eframe::Error::WinitEventLoop(_) => "event-loop",
        _ => "graphics-initialization",
    }
}

fn report_failure(category: &str, location: Option<&str>, detail: Option<&dyn fmt::Display>) {
    // A failure in the reporting path must not recurse into another modal dialog.
    if REPORTING.swap(true, Ordering::SeqCst) {
        return;
    }
    let record = diagnostic_record(category, location);
    let path = write_diagnostic(&record);
    // Raw details are local display only; diagnostic_record never receives them.
    let detail = detail.map(bounded_detail);
    let message = failure_message(category, path.as_deref(), detail.as_deref());
    let _ = writeln!(io::stderr().lock(), "{message}");
    #[cfg(target_os = "windows")]
    {
        // rfd uses a native MessageBoxW; it does not depend on the failed renderer.
        rfd::MessageDialog::new()
            .set_title("worldedit 启动或运行失败")
            .set_description(&message)
            .set_level(rfd::MessageLevel::Error)
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }
    REPORTING.store(false, Ordering::SeqCst);
}

fn bounded_detail(detail: &dyn fmt::Display) -> String {
    struct LimitedWriter {
        text: String,
        characters: usize,
        truncated: bool,
    }
    impl fmt::Write for LimitedWriter {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            for character in value.chars() {
                if self.characters == MAX_DETAIL_CHARS {
                    self.truncated = true;
                    return Err(fmt::Error);
                }
                // An embedded NUL must not hide the rest of a Windows dialog.
                self.text.push(if character == '\0' {
                    '\u{fffd}'
                } else {
                    character
                });
                self.characters += 1;
            }
            Ok(())
        }
    }
    let mut writer = LimitedWriter {
        text: String::new(),
        characters: 0,
        truncated: false,
    };
    // Stream into a bounded buffer rather than allocating an unbounded to_string().
    let _ = fmt::write(&mut writer, format_args!("{detail}"));
    if writer.truncated {
        let marker = "…（详情已截断）";
        let keep = MAX_DETAIL_CHARS - marker.chars().count();
        if let Some((end, _)) = writer.text.char_indices().nth(keep) {
            writer.text.truncate(end);
        }
        writer.text.push_str(marker);
    }
    writer.text
}

fn diagnostic_record(category: &str, location: Option<&str>) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!(
        "unix_seconds={timestamp} version={} os={} arch={} pid={} category={category} {}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::process::id(),
        location.unwrap_or("source=unavailable"),
    )
}

fn failure_message(category: &str, path: Option<&Path>, detail: Option<&str>) -> String {
    let log = match path {
        Some(path) => format!("诊断日志：{}", path.display()),
        None => "诊断日志无法写入，请检查用户目录或临时目录的写入权限。".to_owned(),
    };
    let detail = detail
        .filter(|detail| !detail.is_empty())
        .map(|detail| format!("\n\n错误详情（未写入诊断日志）：\n{detail}"))
        .unwrap_or_default();
    format!(
        "worldedit 遇到错误，无法继续正常启动或运行。\n错误类别：{category}{detail}\n\n{log}\n\n\
         日志只记录版本、平台、错误类别和代码位置，不记录工程内容或凭据。\n\
         请重试；如仍失败，请将错误类别与日志提供给维护者。"
    )
}

fn write_diagnostic(record: &str) -> Option<PathBuf> {
    let _guard = LOG_LOCK.lock().ok()?;
    let fallback = std::env::temp_dir().join("worldedit").join("diagnostics");
    let preferred = diagnostic_directory();
    if let Some(directory) = preferred {
        if let Ok(path) = append_record(&directory, record) {
            return Some(path);
        }
    }
    append_record(&fallback, record).ok()
}

fn absolute_environment_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

fn diagnostic_directory() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = absolute_environment_path("LOCALAPPDATA").map(|path| path.join("worldedit"));
    #[cfg(target_os = "macos")]
    let base = absolute_environment_path("HOME").map(|path| path.join("Library/Logs/worldedit"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = absolute_environment_path("XDG_STATE_HOME")
        .or_else(|| absolute_environment_path("HOME").map(|path| path.join(".local/state")))
        .map(|path| path.join("worldedit"));
    base.map(|path| path.join("diagnostics"))
}

fn append_record(directory: &Path, record: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(directory)?;
    let path = directory.join(LOG_NAME);
    if fs::metadata(&path)
        .is_ok_and(|metadata| metadata.len() + record.len() as u64 > MAX_LOG_BYTES)
    {
        let previous = directory.join(PREVIOUS_LOG_NAME);
        match fs::remove_file(&previous) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        fs::rename(&path, previous)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(record.as_bytes())?;
    file.flush()?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_error_details_are_visible_locally_but_never_serialized_in_logs() {
        let error = eframe::Error::AppCreation("secret token and unpublished project text".into());
        let category = error_category(&error);
        let record = diagnostic_record(category, Some("source=main.rs:12:3"));
        assert!(record.contains("category=app-creation"));
        assert!(record.contains("source=main.rs:12:3"));
        assert!(!record.contains("secret"));
        assert!(!record.contains("project text"));
        let detail = bounded_detail(&error);
        let message = failure_message(category, None, Some(&detail));
        assert!(message.contains("secret token and unpublished project text"));
        assert!(message.contains("未写入诊断日志"));
        assert!(message.contains("无法写入"));
    }

    #[test]
    fn local_details_are_unicode_safe_bounded_and_keep_nul_from_hiding_errors() {
        let detail = bounded_detail(&"界😀".repeat(MAX_DETAIL_CHARS));
        assert_eq!(detail.chars().count(), MAX_DETAIL_CHARS);
        assert!(detail.ends_with("…（详情已截断）"));
        assert_eq!(
            bounded_detail(&"bad\0configuration"),
            "bad\u{fffd}configuration"
        );
        let exact = "x".repeat(MAX_DETAIL_CHARS);
        assert_eq!(bounded_detail(&exact), exact);
    }

    #[test]
    fn huge_display_values_stop_formatting_at_the_detail_limit() {
        struct StreamingDetail(std::cell::Cell<usize>);
        impl fmt::Display for StreamingDetail {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                for _ in 0..100_000 {
                    self.0.set(self.0.get() + 1);
                    formatter.write_str("界")?;
                }
                Ok(())
            }
        }
        let detail = StreamingDetail(std::cell::Cell::new(0));
        assert_eq!(bounded_detail(&detail).chars().count(), MAX_DETAIL_CHARS);
        assert_eq!(detail.0.get(), MAX_DETAIL_CHARS + 1);
    }

    #[test]
    fn panic_string_details_stay_local_and_non_string_payloads_are_omitted() {
        let payload = "private assertion detail".to_owned();
        let detail = bounded_detail(panic_detail(&payload).unwrap());
        assert!(failure_message("panic", None, Some(&detail)).contains(payload.as_str()));
        assert!(!diagnostic_record("panic", Some("source=main.rs:1:2")).contains(payload.as_str()));
        assert_eq!(
            bounded_detail(panic_detail(&"assertion failed").unwrap()),
            "assertion failed"
        );
        assert!(panic_detail(&123_u32).is_none());
    }

    #[test]
    fn diagnostics_rotate_and_stay_outside_the_project() {
        let root =
            std::env::temp_dir().join(format!("worldedit-native-log-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(LOG_NAME), vec![b'x'; MAX_LOG_BYTES as usize]).unwrap();
        let path = append_record(&root, "category=window-system\n").unwrap();
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "category=window-system\n"
        );
        assert_eq!(
            fs::metadata(root.join(PREVIOUS_LOG_NAME)).unwrap().len(),
            MAX_LOG_BYTES
        );
        fs::remove_dir_all(root).unwrap();
        if let Some(directory) = diagnostic_directory() {
            assert!(directory.is_absolute());
            assert!(directory.ends_with("worldedit/diagnostics"));
        }
    }
}
