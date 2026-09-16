//! Fixture roots for the "when present" integration tests.
//!
//! Tests that need real game data read their root from an environment
//! variable, so no developer machine layout lives in the source. Tests
//! skip when the variable is unset or the path does not exist — a skipped
//! gate is silent and looks identical to a passing one, so confirm a gate
//! really ran before trusting a green suite.
//!
//! Set these to run the optional suites: `IMGEDITOR_BULLY_STREAM`,
//! `IMGEDITOR_BULLY_NIF`, `IMGEDITOR_BULLY_NIF_TOOLS`,
//! `IMGEDITOR_GTA3_EXPORTS`, and `IMGEDITOR_CORPUS_ROOT` for the supplied
//! GTA/Bully corpus validation tests.
//!
//! The corpus sub-folder names are load-bearing: gates join them as string
//! literals, so renaming one makes that gate skip rather than fail.

use std::path::PathBuf;

fn root(var: &str) -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os(var)?);
    path.exists().then_some(path)
}

/// Bully SE install `...\Stream` directory (World.img, NIF/, test1/).
pub fn bully_stream() -> Option<PathBuf> {
    root("IMGEDITOR_BULLY_STREAM")
}

/// Loose Bully NIF folder (`...\Stream\NIF`).
pub fn bully_nif() -> Option<PathBuf> {
    root("IMGEDITOR_BULLY_NIF")
}

/// The external `bully-nif-tools` fixture folder (`Nif_Files`).
pub fn bully_nif_tools() -> Option<PathBuf> {
    root("IMGEDITOR_BULLY_NIF_TOOLS")
}

/// Exported GTA III assets used by DFF/decoder fixtures (the folder holding
/// the extracted `.dff`/`.txd` files).
pub fn gta3_exports() -> Option<PathBuf> {
    root("IMGEDITOR_GTA3_EXPORTS")
}

/// Root directory holding the retail corpora (`Gta_3_img\models`,
/// `Grand Theft Auto Vice City\models`, `GTA San Andreas\models`,
/// `Bully script img xbox 360\Scripts.img`, and the Bully install).
/// Tests walk the known archive layout under it.
pub fn corpus_root() -> Option<PathBuf> {
    root("IMGEDITOR_CORPUS_ROOT")
}
