//! NIF inspector (v3.0).
//!
//! Bully Scholarship Edition uses Gamebryo NIF version 20.3.0.9 with
//! `user_version = 0` and `bs_version = 0`. The on-disk layout is fully
//! documented in `Docs/bully_nif_format.md`; the spec is derived from the
//! niftools nifxml schema and verified against `1950Fridge.nif`.
//!
//! The inspector is split into four layers:
//!
//! 1. [`nif`] — the binary parser. It produces a structured [`NifFile`]
//!    from a byte slice, including the header, string table, block
//!    type index, block sizes, and parsed block payloads for every
//!    block type used in Bully.
//! 2. [`texture`] — IDE-based NIF→NFT mapping and embedded pixel-data
//!    extraction from `.nft` (NIF) texture catalog files.
//! 3. [`viewer3d`] — mesh export + texture resolution + system viewer
//!    launch, all on a dedicated thread.

pub mod nif;
pub mod texture;
pub mod viewer3d;
