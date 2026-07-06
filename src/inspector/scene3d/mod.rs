//! Pure-Rust scene representation for the embedded 3D viewer.
//!
//! This module owns the CPU-side data that the wgpu renderer (Phase 17.2)
//! later uploads to the GPU. It does not depend on Iced, winit, or wgpu so
//! it can be unit-tested headless.
//!
//! Layout:
//!
//! - [`mesh`] — interleaved vertex format, mesh container, AABB, optional
//!   RGBA8 diffuse texture.
//! - [`camera`] — orbit camera with `glam::Mat4` projection, configurable
//!   base orientation (Y / Z / X up).
//! - [`scene`] — `Scene` aggregating meshes with lighting state.
//! - [`decode`] — turns a parsed [`crate::inspector::nif::NifFile`] into
//!   a `Scene`, reusing [`crate::inspector::viewer3d::collect_mesh`] for
//!   the geometry strip-to-triangle expansion.
//!
//! The expected lifetime is: `parse NIF bytes → resolve NFT pixels
//! (optional) → build Scene → upload to GPU in the widget`. None of those
//! later steps are visible in this module.

pub mod camera;
pub mod decode;
pub mod headless;
pub mod mesh;
pub mod pipeline;
pub mod scene;

// Re-exports are the public surface documented in the Phase 17 plan.
// The renderer crate (Phase 17.3) consumes them; until then nothing in
// the lib does, which is why the `unused_imports` lint is suppressed
// here rather than scattered across each call site.
#[allow(unused_imports)]
pub use camera::{BaseOrientation, OrbitCamera, Viewport};
#[allow(unused_imports)]
pub use decode::{build_scene_from_nif, parse_and_build_scene, DecodeError};
#[allow(unused_imports)]
pub use mesh::{Aabb, SceneMesh, SceneTexture, Vertex, VERTEX_STRIDE};
#[allow(unused_imports)]
pub use scene::Scene;
