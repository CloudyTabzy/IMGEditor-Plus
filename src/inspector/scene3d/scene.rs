//! Aggregate container for the scene graph rendered by the 3D viewer.
//!
//! A `Scene` is what the widget asks for once per entry selection. It
//! contains one or more [`SceneMesh`] objects (one per `NiTriShape`)
//! plus global lighting parameters and the source base orientation
//! recorded for diagnostics.

use crate::inspector::scene3d::camera::BaseOrientation;
use crate::inspector::scene3d::mesh::{Aabb, SceneMesh, VERTEX_STRIDE};

/// Maximum physical pixels used by one embedded 3D render target.
///
/// This keeps a large/high-DPI window from allocating unbounded color and
/// depth targets on integrated GPUs.
pub const MAX_VIEWPORT_PIXELS: u64 = 16_777_216;

/// Maximum estimated GPU memory for one uploaded scene.
pub const MAX_SCENE_GPU_BYTES: u64 = 512 * 1024 * 1024;

/// Maximum diffuse texture dimension accepted by the embedded viewer.
pub const MAX_TEXTURE_DIMENSION: u32 = 8_192;

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
            ambient: [0.42, 0.44, 0.48],
            // Default key light: high-front-right (camera frame).
            key_light: normalize3([0.65, 0.85, 0.55]),
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

    /// Estimate the persistent GPU resources required for the scene.
    ///
    /// The estimate intentionally covers only buffers and decoded RGBA
    /// textures. Render targets are accounted for separately by the widget,
    /// since their size follows the current viewport.
    pub fn estimated_gpu_bytes(&self) -> Option<u64> {
        self.meshes.iter().try_fold(0_u64, |total, mesh| {
            let vertices = (mesh.vertices.len() as u64).checked_mul(VERTEX_STRIDE as u64)?;
            let indices = (mesh.indices.len() as u64).checked_mul(std::mem::size_of::<u32>() as u64)?;
            let texture = mesh.diffuse.as_ref().map_or(Some(0), |texture| {
                (texture.width as u64)
                    .checked_mul(texture.height as u64)?
                    .checked_mul(4)
            })?;
            total.checked_add(vertices)?.checked_add(indices)?.checked_add(texture)
        })
    }
}

/// Validate CPU-side scene data before it reaches a graphics device.
///
/// Parsers should already produce valid data, but this boundary is still
/// important because a malformed or unusually large archive must not turn
/// into an unchecked GPU allocation or an out-of-bounds index draw.
pub fn validate_scene_data(scene: &Scene) -> Result<(), String> {
    let estimated_bytes = scene
        .estimated_gpu_bytes()
        .ok_or_else(|| "scene GPU size overflowed".to_string())?;
    if estimated_bytes > MAX_SCENE_GPU_BYTES {
        return Err(format!(
            "scene needs about {:.1} MiB of GPU memory; the viewer limit is {:.0} MiB",
            estimated_bytes as f64 / (1024.0 * 1024.0),
            MAX_SCENE_GPU_BYTES as f64 / (1024.0 * 1024.0),
        ));
    }

    for mesh in &scene.meshes {
        if let Some(&index) = mesh.indices.iter().find(|&&index| {
            index as usize >= mesh.vertices.len()
        }) {
            return Err(format!(
                "mesh '{}' contains index {} outside its {} vertices",
                mesh.name,
                index,
                mesh.vertices.len()
            ));
        }

        if let Some(texture) = &mesh.diffuse {
            if texture.width == 0 || texture.height == 0 {
                return Err(format!("mesh '{}' has a zero-sized texture", mesh.name));
            }
            if texture.width > MAX_TEXTURE_DIMENSION || texture.height > MAX_TEXTURE_DIMENSION {
                return Err(format!(
                    "mesh '{}' texture is {}x{}; the viewer limit is {}x{}",
                    mesh.name,
                    texture.width,
                    texture.height,
                    MAX_TEXTURE_DIMENSION,
                    MAX_TEXTURE_DIMENSION
                ));
            }
            let expected = (texture.width as usize)
                .checked_mul(texture.height as usize)
                .and_then(|pixels| pixels.checked_mul(4))
                .ok_or_else(|| format!("mesh '{}' texture size overflowed", mesh.name))?;
            if texture.rgba.len() != expected {
                return Err(format!(
                    "mesh '{}' texture has {} bytes; expected {}",
                    mesh.name,
                    texture.rgba.len(),
                    expected
                ));
            }
        }
    }

    Ok(())
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 1e-6 {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        [0.0, 1.0, 0.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspector::scene3d::mesh::{SceneTexture, Vertex};

    #[test]
    fn estimated_gpu_bytes_accounts_for_mesh_and_texture_data() {
        let scene = Scene {
            meshes: vec![SceneMesh {
                name: "test".to_string(),
                vertices: vec![
                    Vertex {
                        position: [0.0; 3],
                        normal: [0.0; 3],
                        uv: [0.0; 2],
                    };
                    2
                ],
                indices: vec![0, 1, 0],
                diffuse: Some(SceneTexture {
                    width: 2,
                    height: 3,
                    rgba: vec![0; 24],
                }),
                aabb: Aabb::default(),
            }],
            ..Scene::empty(BaseOrientation::Yup)
        };

        assert_eq!(
            scene.estimated_gpu_bytes(),
            Some(2 * VERTEX_STRIDE as u64 + 3 * 4 + 24)
        );
    }

    #[test]
    fn empty_scene_has_zero_gpu_estimate() {
        assert_eq!(Scene::empty(BaseOrientation::Yup).estimated_gpu_bytes(), Some(0));
    }

    #[test]
    fn validation_rejects_bad_indices_and_texture_bytes() {
        let mut scene = Scene {
            meshes: vec![SceneMesh {
                name: "bad".to_string(),
                vertices: vec![Vertex {
                    position: [0.0; 3],
                    normal: [0.0; 3],
                    uv: [0.0; 2],
                }],
                indices: vec![1],
                diffuse: None,
                aabb: Aabb::default(),
            }],
            ..Scene::empty(BaseOrientation::Yup)
        };
        assert!(validate_scene_data(&scene).is_err());

        scene.meshes[0].indices = vec![0];
        scene.meshes[0].diffuse = Some(SceneTexture {
            width: 2,
            height: 2,
            rgba: vec![0; 3],
        });
        assert!(validate_scene_data(&scene).is_err());
    }
}
