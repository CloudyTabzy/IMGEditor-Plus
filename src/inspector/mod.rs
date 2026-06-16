//! NIF inspector (v3.0).
//!
//! Bully Scholarship Edition uses Gamebryo NIF version 20.3.0.9 with
//! `user_version = 0` and `bs_version = 0`. The on-disk layout is fully
//! documented in `Docs/bully_nif_format.md`; the spec is derived from the
//! niftools nifxml schema and verified against `1950Fridge.nif`.
//!
//! The inspector is split into three layers:
//!
//! 1. [`nif`] — the binary parser. It produces a structured [`NifFile`]
//!    from a byte slice, including the header, string table, block
//!    type index, block sizes, and parsed block payloads for every
//!    block type used in Bully.
//! 2. (future) mesh preparation — converts [`nif::NiTriShapeData`] into
//!    GPU-ready vertex/index/UV/normal arrays suitable for handing to
//!    the 3D renderer.
//! 3. [`viewer3d`] — the 3D viewport window. Built on `three-d 0.19`,
//!    it runs in its own `winit 0.28` event loop on a dedicated thread
//!    and communicates with the Iced shell over a `tokio::sync::mpsc`
//!    channel. Iced 0.14 keeps its own `winit 0.30` on the main thread.

pub mod nif;

pub mod viewer3d;
