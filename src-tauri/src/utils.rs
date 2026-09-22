use std::io::Write;
use std::path::{Path, PathBuf};

/// 应用版本号
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn cchub_state_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cchub")
}

pub fn autopilot_root_dir() -> PathBuf {
    cchub_state_dir().join("autopilot")
}

pub fn autopilot_runs_dir() -> PathBuf {
    autopilot_root_dir().join("runs")
}

pub fn crash_log_path() -> PathBuf {
    cchub_state_dir().join("crash.log")
}

pub fn runtime_log_path() -> PathBuf {
    cchub_state_dir().join("app.log")
}

fn trim_text_log(path: &Path, max_bytes: u64, keep_chars: usize) {
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() <= max_bytes {
            return;
        }
        if let Ok(content) = std::fs::read_to_string(path) {
            let keep_from = content.len().saturating_sub(keep_chars);
            let keep_from = content.ceil_char_boundary(keep_from);
            let _ = std::fs::write(path, &content[keep_from..]);
        }
    }
}

fn log_level_rank(level: &str) -> u8 {
    match level.trim().to_ascii_lowercase().as_str() {
        "trace" => 4,
        "debug" => 3,
        "info" => 2,
        "warn" => 1,
        _ => 0,
    }
}

fn runtime_log_enabled(level: &str) -> bool {
    let configured = std::env::var("CCHUB_LOG_LEVEL").unwrap_or_else(|_| "error".to_string());
    log_level_rank(level) <= log_level_rank(&configured)
}

pub fn append_runtime_log(level: &str, scope: &str, message: &str) {
    if !runtime_log_enabled(level) {
        return;
    }

    let log_path = runtime_log_path();
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let entry = format!(
        "[{timestamp}] [{level}] [{scope}] {message}\n",
        level = level.trim().to_ascii_uppercase(),
    );

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let _ = file.write_all(entry.as_bytes());
        let _ = file.flush();
        trim_text_log(&log_path, 400_000, 180_000);
    }
}

#[cfg(target_os = "windows")]
pub fn configure_background_command(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;

    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
pub fn configure_background_command(_command: &mut std::process::Command) {}

/// Install a panic hook that logs crash details to ~/.cchub/crash.log
pub fn install_panic_hook() {
    // Enable backtrace capture
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    let default_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        let log_path = crash_log_path();

        // Ensure directory exists
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Timestamp (with fallback if chrono panics)
        let timestamp = std::panic::catch_unwind(|| {
            chrono::Local::now()
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string()
        })
        .unwrap_or_else(|_| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| format!("unix:{}", d.as_secs()))
                .unwrap_or_else(|_| "unknown".to_string())
        });

        // System info
        let system_info = std::panic::catch_unwind(|| {
            let thread = std::thread::current();
            let thread_name = thread.name().unwrap_or("unnamed");
            format!(
                "OS: {} ({})\nArch: {}\nApp Version: {}\nThread: {} ({:?})",
                std::env::consts::OS,
                std::env::consts::FAMILY,
                std::env::consts::ARCH,
                APP_VERSION,
                thread_name,
                thread.id(),
            )
        })
        .unwrap_or_else(|_| "Failed to get system info".to_string());

        // Panic message
        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            format!("{info}")
        };

        // Location
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".to_string());

        // Backtrace
        let backtrace = std::backtrace::Backtrace::force_capture();

        let separator = "=".repeat(72);
        let crash_entry = format!(
            "\n{separator}\n\
            [CRASH REPORT] {timestamp}\n\
            {separator}\n\n\
            {system_info}\n\n\
            Message: {message}\n\
            Location: {location}\n\n\
            Stack Trace:\n{backtrace}\n\
            {separator}\n"
        );

        // Append to crash log
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            let _ = file.write_all(crash_entry.as_bytes());
            let _ = file.flush();
            trim_text_log(&log_path, 200_000, 100_000);

            eprintln!("\n[CCHub] Crash log saved to: {}", log_path.display());
        }

        eprintln!("{crash_entry}");

        // Call the default hook (shows the standard panic message)
        default_hook(info);
    }));
}

#[cfg(windows)]
fn replace_existing(path: &Path, replacement: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "Kernel32")]
    extern "system" {
        fn ReplaceFileW(
            replaced: *const u16,
            replacement: *const u16,
            backup: *const u16,
            flags: u32,
            exclude: *const std::ffi::c_void,
            reserved: *const std::ffi::c_void,
        ) -> i32;
    }
    let original = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let next = replacement
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        ReplaceFileW(
            original.as_ptr(),
            next.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Write and sync a temporary file before atomically replacing the destination.
pub fn atomic_write(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(content)?;
    temporary.as_file().sync_all()?;
    if path.exists() {
        let temporary_path = temporary.into_temp_path();
        #[cfg(windows)]
        replace_existing(path, temporary_path.as_ref())?;
        #[cfg(not(windows))]
        {
            let temporary_path_ref: &Path = temporary_path.as_ref();
            std::fs::rename(temporary_path_ref, path)?;
        }
    } else {
        temporary.persist(path).map_err(|error| error.error)?;
    }
    Ok(())
}

/// Atomic string write convenience wrapper
pub fn atomic_write_string(path: &Path, content: &str) -> std::io::Result<()> {
    atomic_write(path, content.as_bytes())
}

#[cfg(test)]
mod atomic_write_tests {
    use super::atomic_write;
    use std::fs;

    #[test]
    fn creates_and_replaces_an_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("nested/config.json");
        atomic_write(&path, b"first").unwrap();
        atomic_write(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
    }

    #[test]
    fn a_failed_replacement_keeps_the_original_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.json");
        fs::write(&path, "original").unwrap();
        let result = atomic_write(&path.join("invalid"), b"next");
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "original");
    }

    #[cfg(windows)]
    #[test]
    fn windows_replace_failure_does_not_delete_destination() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.json");
        fs::write(&path, "original").unwrap();
        assert!(super::replace_existing(&path, &temp.path().join("missing.tmp")).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "original");
    }
}
