//! Asset compatibility engine — Phase 0 profiles and the Phase B
//! converter.
//!
//! Profile tables, verdict classification, and the corpus scanner from
//! docs/asset-compatibility-engine.md. The validator is read-only; the
//! converter writes only as an explicit user action with a plan
//! preview.

pub mod convert;
pub mod encode;
pub mod games;
pub mod hint;
pub mod normalize;
pub mod raster;
pub mod save;
pub mod scan;

pub use encode::{encode_texture, header_spec, EncodeFormat, EncodeOptions, EncodedTexture};
pub use games::{
    classify, classify_nft_format, profile_by_id, validate_rasters, Evidence, GameProfile, Offender,
    ValidationSummary, Verdict, VerdictReport, ALL_GAMES, BULLY, GTA3, SA, VC,
};
pub use raster::{Anomaly, LogicalFormat, PaletteKind, RasterProfile, Severity};
