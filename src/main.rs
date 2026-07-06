#![allow(dead_code)]
#![cfg_attr(not(feature = "bench"), windows_subsystem = "windows")]

#[cfg(feature = "bench")]
pub mod archive;
#[cfg(not(feature = "bench"))]
mod archive;

pub mod dev_logger;

#[cfg(feature = "bench")]
pub mod config;
#[cfg(not(feature = "bench"))]
mod config;

#[cfg(feature = "bench")]
pub mod editor;
#[cfg(not(feature = "bench"))]
mod editor;

#[cfg(feature = "bench")]
pub mod inspector;
#[cfg(not(feature = "bench"))]
mod inspector;

#[cfg(feature = "bench")]
pub mod parser;
#[cfg(not(feature = "bench"))]
mod parser;

#[cfg(feature = "bench")]
pub mod runtime;
#[cfg(not(feature = "bench"))]
mod runtime;

#[cfg(feature = "bench")]
pub mod tasks;
#[cfg(not(feature = "bench"))]
mod tasks;

#[cfg(feature = "bench")]
pub mod ui;
#[cfg(not(feature = "bench"))]
mod ui;

#[cfg(feature = "bench")]
pub mod updater;
#[cfg(not(feature = "bench"))]
mod updater;

#[cfg(feature = "bench")]
pub mod utils;
#[cfg(not(feature = "bench"))]
mod utils;

fn main() -> anyhow::Result<()> {
    dev_logger::init_dev_log();
    install_panic_hook();

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

    let config = config::Config::load();
    crate::ui::run_app(config).map_err(|err| anyhow::anyhow!("{err}"))?;
    Ok(())
}

#[cfg(all(windows, not(feature = "bench")))]
fn hide_console_window() {
    use std::ptr;

    unsafe extern "system" {
        fn GetConsoleWindow() -> *mut std::ffi::c_void;
        fn FreeConsole() -> i32;
    }

    unsafe {
        let window = GetConsoleWindow();
        if window != ptr::null_mut() {
            FreeConsole();
        }
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
    let _ = unsafe { std::env::set_var("RUST_BACKTRACE", "full") };
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Re-route the panic through dev_logger's structured report so
        // version + backtrace + breadcrumbs all land in one place.
        let _ = dev_logger::write_crash_report(info);
        previous(info);
    }));
}
