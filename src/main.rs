#![cfg_attr(not(feature = "bench"), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    imgeditor::dev_logger::init_dev_log();
    install_panic_hook();
    clean_temp_preview();

    #[cfg(all(windows, not(feature = "bench")))]
    hide_console_window();

    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "-h" | "--help" => {
                println!("IMGEditor {}", env!("CARGO_PKG_VERSION"));
                println!("Usage: imgeditor [OPTIONS]");
                println!();
                println!("Options:");

                println!("  -h, --help    Print help");
                return Ok(());
            }
            _ => {}
        }
    }

    let config = imgeditor::config::Config::load();
    imgeditor::ui::run_app(config).map_err(|err| anyhow::anyhow!("{err}"))?;
    Ok(())
}

#[cfg(all(windows, not(feature = "bench")))]
fn hide_console_window() {
    unsafe extern "system" {
        fn GetConsoleWindow() -> *mut std::ffi::c_void;
        fn FreeConsole() -> i32;
    }

    unsafe {
        let window = GetConsoleWindow();
        if window.is_null() {
            FreeConsole();
        }
    }
}

/// Best-effort sweep of leftover render previews from prior sessions.
/// The directory is created on demand by `inspector::viewer3d`; a crashed
/// run can orphan files there, so we clear it once at startup. Locked or
/// in-use entries are skipped silently — this must never block startup.
fn clean_temp_preview() {
    let dir = std::env::temp_dir().join("IMGEditor").join("preview");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let _ = std::fs::remove_file(&path).or_else(|_| std::fs::remove_dir_all(&path));
    }
}

/// Capture every panic to `imgeditor-panic.log` next to the executable so
/// the next "the GUI silently disappeared" bug actually leaves a breadcrumb.
/// Falls back to the temp dir if the executable path can't be resolved.
fn install_panic_hook() {
    // Force a backtrace even when the binary is launched without
    // RUST_BACKTRACE=1 (the user-facing default), so the panic log
    // captures frame-by-frame information. `set_var` is unsafe in
    // recent Rust because env reads can race; safe inside a single
    // thread at startup before any other thread is spawned.
    #[allow(unused_unsafe)]
    unsafe {
        std::env::set_var("RUST_BACKTRACE", "full");
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Re-route the panic through dev_logger's structured report so
        // version + backtrace + breadcrumbs all land in one place.
        let _ = imgeditor::dev_logger::write_crash_report(info);
        previous(info);
    }));
}
