#![allow(dead_code)]

mod archive;
mod config;
mod editor;
mod inspector;
mod parser;
mod runtime;
mod tasks;
mod ui;
mod updater;
mod utils;

fn main() -> anyhow::Result<()> {
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
