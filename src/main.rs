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
    editor::Editor::run()?;
    Ok(())
}
