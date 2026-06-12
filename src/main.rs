#![allow(dead_code)]

mod archive;
mod editor;
mod hotkeys;
mod parser;
mod ui;
mod updater;
mod utils;
mod widget;

use anyhow::Result;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "-h" | "--help" => {
                println!("IMGEditor 0.1.0");
                println!("Usage: imgeditor [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -h, --help    Print help");
                return Ok(());
            }
            _ => {}
        }
    }

    editor::Editor::run()?;
    Ok(())
}
