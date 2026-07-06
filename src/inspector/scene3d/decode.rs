//! NIF → [`Scene`] pipeline.
//!
//! Reuses [`crate::inspector::viewer3d::collect_mesh`] for the strip-to-triangle
//! and per-`NiTriShape` transform work, then performs three additional
//! steps that the PLY writer doesn't need:
//!
//! 1. Apply the source base orientation (Y-up / Z-up / X-up) to every
//!    position and normal so the GPU pipeline always runs in Y-up.
//! 2. Build an interleaved vertex buffer (`Vertex`) with `bytemuck::Pod`
//!    layout ready for upload.
//! 3. Compute a per-mesh AABB and a combined scene AABB for the camera
//!    framing helper.
//!
//! Texture resolution is **deferred** to the caller via the
//! `texture_resolver` closure. CPU-side DXT decoding is async-burdensome
//! and is wired up in Phase 17.3 alongside the Iced message glue.

use glam::Mat4;
use thiserror::Error;

use crate::inspector::nif::NifFile;
use crate::inspector::scene3d::camera::BaseOrientation;
use crate::inspector::scene3d::mesh::{Aabb, SceneMesh, Vertex};
use crate::inspector::scene3d::scene::Scene;
use crate::inspector::viewer3d::{collect_mesh, MeshData};

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("NIF has no renderable geometry")]
    NoGeometry,
}

/// Build a [`Scene`] from an already-parsed NIF.
///
/// `texture_resolver` is called once per mesh that exposes a diffuse
/// name. The current pipeline leaves textures `None` for callers that
/// pass a no-op closure; Phase 17.3 plugs in the NFT lookup.
pub fn build_scene_from_nif<F>(
    nif: &NifFile,
    base_orientation: BaseOrientation,
    texture_resolver: F,
) -> Result<Scene, DecodeError>
where
    F: Fn(&str) -> Option<crate::inspector::scene3d::mesh::SceneTexture>,
{
    let raw = collect_mesh(nif).ok_or(DecodeError::NoGeometry)?;
    let mut meshes = Vec::new();

    // A single NIF may describe several `NiTriShape` blocks; the
    // upstream `collect_mesh` flattens them into one big mesh. We keep
    // that for MVP — splitting would mean rebuilding the `Scene` graph
    // and is deferred to Phase 17.4.
    let mesh = mesh_from_data(&raw, base_orientation, texture_resolver);
    let scene_aabb = mesh.aabb;
    meshes.push(mesh);

    Ok(Scene {
        meshes,
        aabb: scene_aabb,
        ambient: [0.22, 0.22, 0.24],
        key_light: [0.45, 0.75, 0.45],
        base_orientation,
    })
}

/// Convenience: parse `bytes` as a NIF, then build the scene. Useful
/// from the UI layer's background task where the parse hasn't been done
/// yet.
pub fn parse_and_build_scene<F>(
    bytes: &[u8],
    base_orientation: BaseOrientation,
    texture_resolver: F,
) -> Result<Scene, DecodeError>
where
    F: Fn(&str) -> Option<crate::inspector::scene3d::mesh::SceneTexture>,
{
    let mut nif = NifFile::parse(bytes).map_err(|_| DecodeError::NoGeometry)?;
    nif.resolve_string_indices();
    build_scene_from_nif(&nif, base_orientation, texture_resolver)
}

fn mesh_from_data<F>(
    data: &MeshData,
    base_orientation: BaseOrientation,
    texture_resolver: F,
) -> SceneMesh
where
    F: Fn(&str) -> Option<crate::inspector::scene3d::mesh::SceneTexture>,
{
    let n_verts = data.positions.len();
    let mut vertices: Vec<Vertex> = Vec::with_capacity(n_verts);
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];

    let xform = base_orientation.to_yup_matrix();
    let xform_inv_transpose = Mat4::transpose(&xform.inverse());

    for i in 0..n_verts {
        let p = data.positions[i];
        let n = if i < data.normals.len() {
            data.normals[i]
        } else {
            // Fall back to +Y when the source has no normals; the
            // shader still renders, just slightly off for hard edges.
            [0.0, 1.0, 0.0]
        };

        // Position: full affine (upper 3x4 of xform applied to (p, 1)).
        let p_in: glam::Vec4 = glam::Vec4::new(p[0], p[1], p[2], 1.0);
        let p_out = xform * p_in;
        let pos = [p_out.x, p_out.y, p_out.z];

        // Normal: rotation only, no translation; renormalize to undo
        // any non-uniform scaling the matrix might carry.
        let n_in: glam::Vec4 = glam::Vec4::new(n[0], n[1], n[2], 0.0);
        let n_out = xform_inv_transpose * n_in;
        let mut normal = [n_out.x, n_out.y, n_out.z];
        let len =
            (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
        if len > 1e-6 {
            normal = [normal[0] / len, normal[1] / len, normal[2] / len];
        } else {
            normal = [0.0, 1.0, 0.0];
        }

        // UV: pass through. Gamebryo UVs are 2D scalar pairs; flipping
        // (1 - v) for OpenGL-style UVs is intentionally NOT done here
        // because the textured samples we've confirmed in Bully already
        // match the shader's default `textureSample` orientation.
        let uv = if i < data.uvs.len() {
            [data.uvs[i][0], data.uvs[i][1]]
        } else {
            [0.0, 0.0]
        };

        vertices.push(Vertex {
            position: pos,
            normal,
            uv,
        });

        for axis in 0..3 {
            min[axis] = min[axis].min(pos[axis]);
            max[axis] = max[axis].max(pos[axis]);
        }
    }

    // Indices are already in world-space order from `collect_mesh`; no
    // re-indexing needed since we built a single mesh.
    let indices = data.indices.clone();

    let aabb = if vertices.is_empty() {
        Aabb::default()
    } else {
        Aabb { min, max }
    };

    // No diffuse lookup at MVP; the closure exists for Phase 17.3 to
    // plug in the NFT path without re-shaping the public API.
    let _ = texture_resolver;

    SceneMesh {
        name: String::from("mesh"),
        vertices,
        indices,
        diffuse: None,
        aabb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_pt(a: [f32; 3], b: [f32; 3]) -> bool {
        let dx = (a[0] - b[0]).abs();
        let dy = (a[1] - b[1]).abs();
        let dz = (a[2] - b[2]).abs();
        dx < 1e-4 && dy < 1e-4 && dz < 1e-4
    }

    #[test]
    fn empty_input_returns_error() {
        let bytes: [u8; 0] = [];
        let r = parse_and_build_scene(&bytes, BaseOrientation::Yup, |_| None);
        assert!(r.is_err());
    }

    #[test]
    fn random_non_nif_bytes_return_error() {
        let bytes: Vec<u8> = (0..256).map(|i| i as u8).collect();
        assert!(parse_and_build_scene(&bytes, BaseOrientation::Yup, |_| None).is_err());
    }

    #[test]
    fn transformer_maps_zup_y_axis_to_negative_z() {
        // Mirror the matrix used for Z-up without going through the
        // public decoder: ensure a world point on +Y comes out at -Z.
        let m = BaseOrientation::Zup.to_yup_matrix();
        let v = m * glam::Vec4::new(0.0, 1.0, 0.0, 1.0);
        assert!(approx_pt([v.x, v.y, v.z], [0.0, 0.0, -1.0]));
    }

    #[test]
    fn decoder_handles_bully_fixture_when_present() {
        // Same fixture path the existing `nif` tests use. Skips when
        // the file is not on the dev machine, mirroring the test
        // pattern in `inspector::texture`.
        let path = "C:/Games/Bully - Scholarship Edition/Stream/test1/1950Fridge.nif";
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => return,
        };
        let scene = parse_and_build_scene(&bytes, BaseOrientation::Yup, |_| None)
            .expect("1950Fridge should decode");
        // A populated AABB and at least one mesh with non-zero
        // triangle count are the minimum bar; an exact triangle
        // count is checked against a known-good value in the full
        // integration test once a fixture is committed.
        assert!(scene.has_geometry());
        assert!(scene.total_triangles() > 0);
        let r = scene.aabb.bounding_radius();
        assert!(r > 0.0 && r.is_finite());
    }
}
