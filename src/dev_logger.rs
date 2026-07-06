//! Developer crash + diagnostic logger.
//!
//! # Scope
//!
//! The Iced GUI itself is hard to debug from inside the winit event
//! loop — panics, wgpu validation errors, and iced widget panics all
//! surface to the user as "the window disappeared" with no on-screen
//! feedback. This module is the breadcrumb-trail that turns those
//! silent failures into a file the developer can `cat` afterwards.
//!
//! Two logger profiles:
//!
//! - **Dev (debug build)**: `log::trace!` and above, written to
//!   `target/debug/imgeditor-dev.log` plus mirrored to `stderr` so
//!   `cargo run` is enough to see what's happening.
//! - **Release**: `log::warn!` and above, written to
//!   `<exe-dir>/imgeditor.log`. Volume is kept low so public users can
//!   attach a log if they file a crash report.
//!
//! Both share the same on-disk schema so a release-build crash log
//! can be diffed against a dev-build one in the same script.
//!
//! # Format
//!
//! Each line:
//!   `[2026-07-06 14:32:11.042] [INFO ] [viewer3d_widget] compiled grid shader
//!
//! The prefix fields stay aligned for `awk`/`cut` readability. The
//! level tag and target module make it easy to grep for a single
//! subsystem.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};

/// Records whether `init_dev_log` has already been called so we don't
/// double-install the panic hook (which would chain) or open the log
/// file twice.
static INITIALIZED: Mutex<bool> = Mutex::new(false);

/// Once-per-process bootstrap. Call from `main` as the very first
/// thing so even early-stage panics are captured.
///
/// In debug builds we set `RUST_BACKTRACE=full` and route every
/// `log::*` record to `target/debug/imgeditor-dev.log` and to
/// `stderr`. In release builds we go quieter (`warn`+) and write
/// next to the executable, alongside the existing
/// `imgeditor-panic.log`.
pub fn init_dev_log() {
    let mut flag = INITIALIZED.lock().expect("dev_logger init flag");
    if *flag {
        return;
    }
    *flag = true;
    drop(flag);

    let path = log_path();
    let log_file = match open_append(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[imgeditor] failed to open dev log at {path:?}: {e}");
            return;
        }
    };
    if let Err(e) = write_header(&path) {
        eprintln!("[imgeditor] failed to write dev log header: {e}");
    }

    let max = if cfg!(debug_assertions) {
        LevelFilter::Trace
    } else {
        LevelFilter::Warn
    };
    // `log::set_logger` requires a `&'static dyn Log`. The logger is
    // live for the rest of the program so leaking the box is fine;
    // the File/Mutex inside the box are cheap to drop when the OS
    // reclaims the heap.
    let logger: &'static FileLogger = Box::leak(Box::new(FileLogger {
        file: Mutex::new(log_file),
        path,
        max_level: max,
    }));
    if log::set_logger(logger).is_err() {
        eprintln!("[imgeditor] log::set_logger already initialised; skipping dev logger");
        return;
    }
    log::set_max_level(max);

    if cfg!(debug_assertions) {
        // Force a backtrace even when launched without
        // RUST_BACKTRACE=1. set_var is racy in general, but this runs
        // at startup before any other thread is spawned.
        #[allow(unused_unsafe)]
        let _ = unsafe { std::env::set_var("RUST_BACKTRACE", "full") };
    }

    log::info!(
        target: "imgeditor",
        "=== imgeditor dev logger online ({}) ===",
        if cfg!(debug_assertions) { "debug" } else { "release" }
    );
    let log_path_str = log_path();
    log::info!(target: "imgeditor", "log file: {}", log_path_str.display());
    log::info!(target: "imgeditor", "version: {} ({})", env!("CARGO_PKG_VERSION"), std::env::consts::OS);
}

fn log_path() -> PathBuf {
    let base = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::temp_dir());
    let name = if cfg!(debug_assertions) {
        "imgeditor-dev.log"
    } else {
        "imgeditor.log"
    };
    base.join(name)
}

fn open_append(path: &PathBuf) -> std::io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

fn write_header(path: &PathBuf) -> std::io::Result<()> {
    let mut f = open_append(path)?;
    writeln!(
        f,
        "\n--- imgeditor {} {} ({}) boot @ {} ---",
        env!("CARGO_PKG_VERSION"),
        if cfg!(debug_assertions) { "debug" } else { "release" },
        std::env::consts::ARCH,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    )
}

struct FileLogger {
    file: Mutex<File>,
    path: PathBuf,
    max_level: LevelFilter,
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.max_level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let level = match record.level() {
            Level::Error => "ERROR",
            Level::Warn => "WARN ",
            Level::Info => "INFO ",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        };
        let target = record.target();
        let line = format!(
            "[{}] [{}] [{}] {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            level,
            target,
            record.args()
        );
        if let Ok(mut f) = self.file.lock() {
            let _ = f.write_all(line.as_bytes());
        }
        // Mirror to stderr in debug builds so `cargo run` is enough.
        if cfg!(debug_assertions) {
            eprint!("{line}");
        }
    }

    fn flush(&self) {
        if let Ok(mut f) = self.file.lock() {
            let _ = f.flush();
        }
    }
}

/// Best-effort append of a one-line breadcrumb to the dev log.
/// Call this at any point of interest (entry into the 3D viewer
/// tab, scene load, model render) so the panic log has a
/// breadcrumb trail showing what the user was doing when things
/// went wrong.
pub fn breadcrumb(message: &str) {
    log::info!(target: "imgeditor.breadcrumb", "{}", message);
}

/// File path of the active dev log (mirrors what `init_dev_log`
/// opened). Useful for the toast / help dialog.
pub fn log_file_path() -> PathBuf {
    log_path()
}

/// On a panic, write a structured crash report that always survives
/// the unwinding into iced_wgpu's event loop. Returns the path the
/// report was written to so the caller can show a toast.
#[allow(deprecated)]
pub fn write_crash_report(info: &std::panic::PanicInfo<'_>) -> std::io::Result<PathBuf> {
    let path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::temp_dir())
        .join("imgeditor-panic.log");
    let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(f, "[panic at {}] {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), info)?;
    writeln!(f, "version: {} ({})", env!("CARGO_PKG_VERSION"), if cfg!(debug_assertions) { "debug" } else { "release" })?;
    writeln!(f, "os: {} {}", std::env::consts::OS, std::env::consts::ARCH)?;
    writeln!(f, "backtrace:")?;
    writeln!(f, "{}", std::backtrace::Backtrace::capture())?;
    Ok(path)
}
