//! NIF inspector (v3.0).
//!
//! Bully Scholarship Edition uses Gamebryo NIF version 20.3.0.9 with
//! `user_version = 0` and `bs_version = 0`. The on-disk layout is fully
//! documented in `Docs/bully_nif_format.md`; the spec is derived from the
//! niftools nifxml schema and verified against `1950Fridge.nif`.
//!
//! The inspector is split into five layers:
//!
//! 1. [`nif`] — the binary parser. It produces a structured [`NifFile`]
//!    from a byte slice, including the header, string table, block
//!    type index, block sizes, and parsed block payloads for every
//!    block type used in Bully.
//! 2. [`texture`] — IDE-based NIF→NFT mapping and embedded pixel-data
//!    extraction from `.nft` (NIF) texture catalog files.
//! 3. [`viewer3d`] — mesh export + texture resolution + system viewer
//!    launch, all on a dedicated thread. Acts as the external-viewer
//!    fallback behind the right-click menu in v3.4 and is removed
//!    in v4.
//! 4. [`scene3d`] — embedded viewer CPU side (v3.4). Interleaved
//!    vertices, orbit camera math, `Scene` aggregation, NIF→Scene
//!    decode. Phase 17.2 wires these into a wgpu render pipeline.

pub mod nif;
pub mod scene3d;
pub mod texture;
pub mod texture_export;
pub mod viewer3d;
