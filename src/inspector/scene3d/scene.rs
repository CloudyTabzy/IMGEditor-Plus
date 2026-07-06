//! Aggregate container for the scene graph rendered by the 3D viewer.
//!
//! A `Scene` is what the widget asks for once per entry selection. It
//! contains one or more [`SceneMesh`] objects (one per `NiTriShape`)
//! plus global lighting parameters and the source base orientation
//! recorded for diagnostics.

use crate::inspector::scene3d::camera::BaseOrientation;
use crate::inspector::scene3d::mesh::{Aabb, SceneMesh};

/// Aggregate state for one rendered model.
#[derive(Clone, Debug)]
pub struct Scene {
    pub meshes: Vec<SceneMesh>,
    /// Tight AABB over all mesh positions in the camera frame.
    pub aabb: Aabb,
    /// Linear RGB ambient in `[0, 1]`.
    pub ambient: [f32; 3],
    /// Direction toward the key light (camera frame, unit length).
    pub key_light: [f32; 3],
    /// Source base orientation of the input data; kept for diagnostics
    /// ("Z-up source → Y-up camera") and for the toolbar's status text.
    pub base_orientation: BaseOrientation,
}

impl Scene {
    /// Build an empty scene; useful as the default for an uninitialised
    /// widget and as the result when a NIF parses but has no geometry.
    pub fn empty(base_orientation: BaseOrientation) -> Self {
        Self {
            meshes: Vec::new(),
            aabb: Aabb::default(),
            ambient: [0.20, 0.20, 0.22],
            // Default key light: high-front-right (camera frame).
            key_light: normalize3([0.5, 0.8, 0.4]),
            base_orientation,
        }
    }

    pub fn total_vertices(&self) -> usize {
        self.meshes.iter().map(|m| m.vertex_count()).sum()
    }

    pub fn total_triangles(&self) -> usize {
        self.meshes.iter().map(|m| m.triangle_count()).sum()
    }

    pub fn textured_mesh_count(&self) -> usize {
        self.meshes.iter().filter(|m| m.has_texture()).count()
    }

    /// Distinct diffuse textures across all meshes (some scenes share a
    /// texture across many `NiTriShape`s). Used by the stats footer.
    pub fn distinct_textures(&self) -> usize {
        let mut seen: Vec<&SceneMesh> = Vec::new();
        for m in &self.meshes {
            if m.diffuse.is_some() && !seen.iter().any(|s| core::ptr::eq(*s, m)) {
                seen.push(m);
            }
        }
        seen.len()
    }

    /// `true` iff the scene has at least one mesh with at least one
    /// triangle. An empty scene should be rendered as the placeholder
    /// ("nothing to display") rather than as a blank canvas.
    pub fn has_geometry(&self) -> bool {
        self.meshes.iter().any(|m| !m.indices.is_empty())
    }
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 1e-6 {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        [0.0, 1.0, 0.0]
    }
}
