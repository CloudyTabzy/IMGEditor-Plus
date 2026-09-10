//! Asset compatibility engine — Phase 0.
//!
//! Profile tables, verdict classification, and the corpus scanner from
//! docs/asset-compatibility-engine.md. Read-only today: this module
//! judges assets, it never converts or writes them.

pub mod games;
pub mod raster;
pub mod scan;

pub use games::{classify, Evidence, GameProfile, Verdict, VerdictReport, ALL_GAMES, BULLY, GTA3, SA, VC};
pub use raster::{Anomaly, LogicalFormat, PaletteKind, RasterProfile, Severity};
