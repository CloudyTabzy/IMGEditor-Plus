//! Asset compatibility engine — Phase 0.
//!
//! Profile tables, verdict classification, and the corpus scanner from
//! docs/asset-compatibility-engine.md. Read-only today: this module
//! judges assets, it never converts or writes them.

pub mod games;
pub mod hint;
pub mod normalize;
pub mod raster;
pub mod save;
pub mod scan;

pub use games::{
    classify, classify_nft_format, profile_by_id, validate_rasters, Evidence, GameProfile, Offender,
    ValidationSummary, Verdict, VerdictReport, ALL_GAMES, BULLY, GTA3, SA, VC,
};
pub use raster::{Anomaly, LogicalFormat, PaletteKind, RasterProfile, Severity};
