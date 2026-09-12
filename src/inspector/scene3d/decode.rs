//! NIF → [`Scene`] pipeline.
//!
//! Reuses [`crate::inspector::viewer3d::collect_meshes`] for scene-graph
//! traversal, strip-to-triangle conversion, inherited transforms, and
//! per-mesh texture selection, then performs three additional
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
//! `texture_resolver` closure so archive I/O stays outside this decoder.

use glam::Mat4;
use thiserror::Error;

use crate::inspector::nif::NifFile;
use crate::inspector::scene3d::camera::BaseOrientation;
use crate::inspector::scene3d::mesh::{Aabb, SceneMesh, SceneTexture, Vertex};
use crate::inspector::scene3d::scene::Scene;
use crate::inspector::viewer3d::{MeshData, collect_meshes};
use crate::parser::col::ColFile;
use crate::parser::dff::DffMesh;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("NIF has no renderable geometry")]
    NoGeometry,
}

const MAX_COLLISION_PREVIEW_VERTICES: usize = 2_000_000;
const MAX_COLLISION_PREVIEW_TRIANGLES: usize = 4_000_000;
const COLLISION_SPHERE_SEGMENTS: usize = 16;
const COLLISION_SPHERE_RINGS: usize = 8;

#[derive(Default)]
struct CollisionMeshBuilder {
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

impl CollisionMeshBuilder {
    fn can_append(&self, vertex_count: usize, triangle_count: usize) -> bool {
        self.positions
            .len()
            .checked_add(vertex_count)
            .is_some_and(|count| count <= MAX_COLLISION_PREVIEW_VERTICES)
            && self
                .indices
                .len()
                .checked_add(triangle_count.saturating_mul(3))
                .is_some_and(|count| count <= MAX_COLLISION_PREVIEW_TRIANGLES.saturating_mul(3))
    }

    fn append_triangle_mesh(&mut self, positions: &[[f32; 3]], indices: &[u32]) {
        let triangle_count = indices.len() / 3;
        if positions.is_empty()
            || triangle_count == 0
            || !positions.iter().all(|position| position.iter().all(|value| value.is_finite()))
            || !self.can_append(positions.len(), triangle_count)
        {
            return;
        }
        let Ok(offset) = u32::try_from(self.positions.len()) else {
            return;
        };
        let Ok(position_count) = u32::try_from(positions.len()) else {
            return;
        };
        if offset.checked_add(position_count).is_none() {
            return;
        }
        self.positions.extend_from_slice(positions);
        for triangle in indices.chunks_exact(3) {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]];
            if a == b
                || a == c
                || b == c
                || a as usize >= positions.len()
                || b as usize >= positions.len()
                || c as usize >= positions.len()
            {
                continue;
            }
            let Some(a) = offset.checked_add(a) else {
                continue;
            };
            let Some(b) = offset.checked_add(b) else {
                continue;
            };
            let Some(c) = offset.checked_add(c) else {
                continue;
            };
            self.indices.extend_from_slice(&[a, b, c]);
        }
    }

    fn append_box(&mut self, collision_box: &crate::parser::col::ColBox) {
        if !collision_box
            .min
            .iter()
            .chain(collision_box.max.iter())
            .all(|value| value.is_finite())
            || !self.can_append(8, 12)
        {
            return;
        }
        let min = [
            collision_box.min[0].min(collision_box.max[0]),
            collision_box.min[1].min(collision_box.max[1]),
            collision_box.min[2].min(collision_box.max[2]),
        ];
        let max = [
            collision_box.min[0].max(collision_box.max[0]),
            collision_box.min[1].max(collision_box.max[1]),
            collision_box.min[2].max(collision_box.max[2]),
        ];
        let Ok(base) = u32::try_from(self.positions.len()) else {
            return;
        };
        self.positions.extend_from_slice(&[
            [min[0], min[1], min[2]],
            [max[0], min[1], min[2]],
            [max[0], max[1], min[2]],
            [min[0], max[1], min[2]],
            [min[0], min[1], max[2]],
            [max[0], min[1], max[2]],
            [max[0], max[1], max[2]],
            [min[0], max[1], max[2]],
        ]);
        const FACES: [[u32; 3]; 12] = [
            [0, 1, 2],
            [0, 2, 3],
            [4, 6, 5],
            [4, 7, 6],
            [0, 4, 5],
            [0, 5, 1],
            [1, 5, 6],
            [1, 6, 2],
            [2, 6, 7],
            [2, 7, 3],
            [3, 7, 4],
            [3, 4, 0],
        ];
        for [a, b, c] in FACES {
            self.indices.extend_from_slice(&[
                base + a,
                base + b,
                base + c,
            ]);
        }
    }

    fn append_sphere(&mut self, sphere: &crate::parser::col::ColSphere) {
        let vertex_count = (COLLISION_SPHERE_RINGS + 1)
            .saturating_mul(COLLISION_SPHERE_SEGMENTS + 1);
        let triangle_count = COLLISION_SPHERE_RINGS.saturating_mul(COLLISION_SPHERE_SEGMENTS * 2);
        if !sphere.radius.is_finite()
            || sphere.radius <= 0.0
            || !sphere
                .center
                .iter()
                .all(|value| value.is_finite())
            || !self.can_append(vertex_count, triangle_count)
        {
            return;
        }
        let Ok(base) = u32::try_from(self.positions.len()) else {
            return;
        };
        let two_pi = std::f32::consts::TAU;
        for ring in 0..=COLLISION_SPHERE_RINGS {
            let v = ring as f32 / COLLISION_SPHERE_RINGS as f32;
            let theta = v * std::f32::consts::PI;
            let y = theta.cos();
            let ring_radius = theta.sin();
            for segment in 0..=COLLISION_SPHERE_SEGMENTS {
                let u = segment as f32 / COLLISION_SPHERE_SEGMENTS as f32;
                let phi = u * two_pi;
                let normal = [ring_radius * phi.cos(), y, ring_radius * phi.sin()];
                self.positions.push([
                    sphere.center[0] + sphere.radius * normal[0],
                    sphere.center[1] + sphere.radius * normal[1],
                    sphere.center[2] + sphere.radius * normal[2],
                ]);
            }
        }
        let stride = COLLISION_SPHERE_SEGMENTS + 1;
        for ring in 0..COLLISION_SPHERE_RINGS {
            for segment in 0..COLLISION_SPHERE_SEGMENTS {
                let a = base + (ring * stride + segment) as u32;
                let b = a + 1;
                let d = base + ((ring + 1) * stride + segment) as u32;
                let c = d + 1;
                self.indices.extend_from_slice(&[a, b, c, a, c, d]);
            }
        }
    }

    fn into_mesh_data(self, name: String) -> Option<MeshData> {
        if self.positions.is_empty() || self.indices.is_empty() {
            return None;
        }
        let mut normals = vec![[0.0; 3]; self.positions.len()];
        for triangle in self.indices.chunks_exact(3) {
            let [a, b, c] = [
                self.positions[triangle[0] as usize],
                self.positions[triangle[1] as usize],
                self.positions[triangle[2] as usize],
            ];
            let edge_a = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let edge_b = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let normal = [
                edge_a[1] * edge_b[2] - edge_a[2] * edge_b[1],
                edge_a[2] * edge_b[0] - edge_a[0] * edge_b[2],
                edge_a[0] * edge_b[1] - edge_a[1] * edge_b[0],
            ];
            for &index in triangle {
                let normal_out = &mut normals[index as usize];
                normal_out[0] += normal[0];
                normal_out[1] += normal[1];
                normal_out[2] += normal[2];
            }
        }
        for normal in &mut normals {
            let length = (normal[0] * normal[0]
                + normal[1] * normal[1]
                + normal[2] * normal[2])
                .sqrt();
            if length > 1e-6 {
                normal[0] /= length;
                normal[1] /= length;
                normal[2] /= length;
            } else {
                *normal = [0.0, 1.0, 0.0];
            }
        }
        let vertex_count = self.positions.len();
        Some(MeshData {
            name,
            texture_name: None,
            positions: self.positions,
            normals,
            uvs: vec![[0.0, 0.0]; vertex_count],
            indices: self.indices,
        })
    }
}

/// Build a [`Scene`] from an already-parsed NIF.
///
/// `texture_resolver` is called once per mesh that exposes a diffuse name.
/// Callers that pass a no-op closure intentionally receive untextured meshes.
pub fn build_scene_from_nif<F>(
    nif: &NifFile,
    base_orientation: BaseOrientation,
    texture_resolver: F,
) -> Result<Scene, DecodeError>
where
    F: Fn(&str) -> Option<crate::inspector::scene3d::mesh::SceneTexture>,
{
    let raw_meshes = collect_meshes(nif);
    if raw_meshes.is_empty() {
        return Err(DecodeError::NoGeometry);
    }
    let mut meshes = Vec::with_capacity(raw_meshes.len());
    let mut scene_aabb: Option<Aabb> = None;
    for raw in &raw_meshes {
        let diffuse = raw.texture_name.as_deref().and_then(&texture_resolver);
        let mesh = mesh_from_data(raw, base_orientation, diffuse);
        scene_aabb = Some(match scene_aabb {
            Some(aabb) => aabb.merged(mesh.aabb),
            None => mesh.aabb,
        });
        meshes.push(mesh);
    }

    Ok(Scene {
        meshes,
        aabb: scene_aabb.unwrap_or_default(),
        ambient: [0.42, 0.44, 0.48],
        key_light: [0.65, 0.85, 0.55],
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

/// Build a Scene from RenderWare DFF meshes.
///
/// GTA PC DFF coordinates are Z-up, so the caller normally supplies
/// BaseOrientation::Zup. The texture resolver is called once for each
/// material-split mesh that exposes a diffuse TXD name.
pub fn build_scene_from_dff<F>(
    dff_meshes: &[DffMesh],
    base_orientation: BaseOrientation,
    texture_resolver: F,
) -> Result<Scene, DecodeError>
where
    F: Fn(&str) -> Option<SceneTexture>,
{
    if dff_meshes.is_empty() {
        return Err(DecodeError::NoGeometry);
    }

    let mut meshes = Vec::with_capacity(dff_meshes.len());
    let mut scene_aabb: Option<Aabb> = None;
    for raw in dff_meshes {
        if raw.positions.is_empty() || raw.indices.is_empty() {
            continue;
        }
        let data = MeshData {
            name: raw.name.clone(),
            texture_name: raw.texture_name.clone(),
            positions: raw.positions.clone(),
            normals: raw.normals.clone(),
            uvs: raw.uvs.clone(),
            indices: raw.indices.clone(),
        };
        let diffuse = data.texture_name.as_deref().and_then(&texture_resolver);
        let mesh = mesh_from_data(&data, base_orientation, diffuse);
        scene_aabb = Some(match scene_aabb {
            Some(aabb) => aabb.merged(mesh.aabb),
            None => mesh.aabb,
        });
        meshes.push(mesh);
    }

    if meshes.is_empty() {
        return Err(DecodeError::NoGeometry);
    }

    Ok(Scene {
        meshes,
        aabb: scene_aabb.unwrap_or_default(),
        ambient: [0.42, 0.44, 0.48],
        key_light: [0.65, 0.85, 0.55],
        base_orientation,
    })
}

/// Convenience: parse DFF bytes, then build the embedded viewer scene.
pub fn parse_and_build_scene_from_dff<F>(
    bytes: &[u8],
    base_orientation: BaseOrientation,
    texture_resolver: F,
) -> Result<Scene, DecodeError>
where
    F: Fn(&str) -> Option<SceneTexture>,
{
    let meshes = crate::parser::dff::parse_dff(bytes).map_err(|_| DecodeError::NoGeometry)?;
    build_scene_from_dff(&meshes, base_orientation, texture_resolver)
}

/// Build a Scene from standalone GTA collision geometry.
///
/// COL files are Z-up RenderWare collision data rather than textured render
/// assets. Triangle meshes are copied directly, while their collision boxes
/// and spheres are tessellated into bounded preview geometry so files that
/// contain only primitives remain visible in the generic GPU renderer.
pub fn build_scene_from_col(
    col: &ColFile,
    base_orientation: BaseOrientation,
) -> Result<Scene, DecodeError> {
    let raw_meshes = collision_mesh_data(col);
    if raw_meshes.is_empty() {
        return Err(DecodeError::NoGeometry);
    }

    let mut meshes = Vec::new();
    let mut scene_aabb: Option<Aabb> = None;

    for data in raw_meshes {
        let mesh = mesh_from_data(&data, base_orientation, None);
        scene_aabb = Some(match scene_aabb {
            Some(aabb) => aabb.merged(mesh.aabb),
            None => mesh.aabb,
        });
        meshes.push(mesh);
    }

    Ok(Scene {
        meshes,
        aabb: scene_aabb.unwrap_or_default(),
        ambient: [0.42, 0.44, 0.48],
        key_light: [0.65, 0.85, 0.55],
        base_orientation,
    })
}

/// Convert parsed collision entries into raw preview meshes. The helper is
/// shared by the embedded scene builder and the external PLY fallback so
/// primitive-only Bully records are rendered consistently in both paths.
pub(crate) fn collision_mesh_data(col: &ColFile) -> Vec<MeshData> {
    let mut meshes = Vec::new();

    for entry in &col.entries {
        let mut builder = CollisionMeshBuilder::default();
        builder.append_triangle_mesh(&entry.vertices, &entry.indices);
        for collision_box in &entry.boxes {
            builder.append_box(collision_box);
        }
        for sphere in &entry.spheres {
            builder.append_sphere(sphere);
        }
        append_collision_mesh_data(
            &mut meshes,
            builder,
            format!("{} collision", display_collision_name(entry)),
        );

        if !entry.shadow_indices.is_empty() {
            let mut shadow = CollisionMeshBuilder::default();
            shadow.append_triangle_mesh(&entry.shadow_vertices, &entry.shadow_indices);
            append_collision_mesh_data(
                &mut meshes,
                shadow,
                format!("{} shadow", display_collision_name(entry)),
            );
        }
    }

    meshes
}

/// Convenience: parse COL bytes, then build the embedded viewer scene.
pub fn parse_and_build_scene_from_col(
    bytes: &[u8],
    base_orientation: BaseOrientation,
) -> Result<Scene, DecodeError> {
    let col = crate::parser::col::parse_col(bytes).map_err(|_| DecodeError::NoGeometry)?;
    build_scene_from_col(&col, base_orientation)
}

fn append_collision_mesh_data(
    meshes: &mut Vec<MeshData>,
    builder: CollisionMeshBuilder,
    name: String,
) {
    let Some(data) = builder.into_mesh_data(name) else {
        return;
    };
    meshes.push(data);
}

fn display_collision_name(entry: &crate::parser::col::ColEntry) -> &str {
    if entry.model_name.is_empty() {
        "unnamed"
    } else {
        &entry.model_name
    }
}

fn mesh_from_data(
    data: &MeshData,
    base_orientation: BaseOrientation,
    diffuse: Option<SceneTexture>,
) -> SceneMesh {
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
        let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
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

    SceneMesh {
        name: data.name.clone(),
        texture_name: data.texture_name.clone(),
        vertices,
        indices,
        diffuse,
        aabb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::ArchiveInfo;
    use crate::parser::read_entry_data;
    use std::cell::RefCell;

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
    fn dff_builder_keeps_geometry_and_resolves_diffuse_texture() {
        let meshes = [DffMesh {
            name: "body".to_string(),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            indices: vec![0, 1, 2],
            material_name: None,
            texture_name: Some("body_d".to_string()),
        }];
        let requested = RefCell::new(Vec::new());

        let scene = build_scene_from_dff(&meshes, BaseOrientation::Zup, |name| {
            requested.borrow_mut().push(name.to_string());
            Some(SceneTexture {
                width: 1,
                height: 1,
                rgba: vec![255, 128, 64, 255],
            })
        })
        .expect("DFF mesh should build into a scene");

        assert_eq!(scene.total_vertices(), 3);
        assert_eq!(scene.total_triangles(), 1);
        assert_eq!(scene.textured_mesh_count(), 1);
        assert_eq!(
            requested.borrow().as_slice(),
            [String::from("body_d")].as_slice()
        );
    }

    #[test]
    fn col_builder_renders_triangles_and_collision_primitives() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"COLL");
        bytes.extend_from_slice(&[0; 4]);
        let mut body = Vec::new();
        body.extend_from_slice(&[0; 24]);
        for value in [
            2.0f32, 0.0, 0.0, 0.0, -2.0, -2.0, -2.0, 2.0, 2.0, 2.0,
        ] {
            body.extend_from_slice(&value.to_le_bytes());
        }
        body.extend_from_slice(&1u32.to_le_bytes());
        body.extend_from_slice(&1.0f32.to_le_bytes());
        for value in [0.0f32, 1.0, 2.0] {
            body.extend_from_slice(&value.to_le_bytes());
        }
        body.extend_from_slice(&[0, 0, 0, 0]);
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&1u32.to_le_bytes());
        for value in [-1.0f32, -1.0, -1.0, 1.0, 1.0, 1.0] {
            body.extend_from_slice(&value.to_le_bytes());
        }
        body.extend_from_slice(&[0, 0, 0, 0]);
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        let body_size = u32::try_from(body.len()).expect("COL fixture fits");
        bytes[4..8].copy_from_slice(&body_size.to_le_bytes());
        bytes.extend_from_slice(&body);

        let scene = parse_and_build_scene_from_col(&bytes, BaseOrientation::Zup)
            .expect("shape-only COL should build a scene");
        assert!(scene.has_geometry());
        assert!(scene.total_triangles() >= 12);
        assert!(scene.aabb.bounding_radius() > 0.0);
    }

    #[test]
    fn decoder_builds_bully_collision_shape_variants_when_present() {
        let Some(stream) = crate::test_paths::bully_stream() else {
            return;
        };
        let archive_path = stream.join("World.img");
        if !archive_path.is_file() {
            return;
        }
        let archive = ArchiveInfo::open(&archive_path)
            .unwrap_or_else(|error| panic!("{} should open: {error}", archive_path.display()));

        for name in ["aquabike.col", "AddBook.col", "AniPillo.col"] {
            let entry = archive
                .entries
                .iter()
                .find(|entry| entry.file_name.eq_ignore_ascii_case(name))
                .unwrap_or_else(|| panic!("{name} should be present in World.img"));
            let bytes = read_entry_data(&archive, entry)
                .unwrap_or_else(|error| panic!("{name} should be readable: {error}"));
            let scene = parse_and_build_scene_from_col(&bytes, BaseOrientation::Zup)
                .unwrap_or_else(|error| panic!("{name} should build a scene: {error:?}"));
            assert!(scene.has_geometry(), "{name} should have renderable geometry");
            assert!(scene.total_triangles() > 0, "{name} should have triangles");
            assert!(scene.aabb.bounding_radius().is_finite());
        }
    }

    #[test]
    fn decoder_builds_real_vc_col_when_present() {
        let Some(root) = crate::test_paths::corpus_root() else {
            return;
        };
        let path = root.join("Grand Theft Auto Vice City/data/maps/airport/airport.col");
        let Ok(bytes) = std::fs::read(path) else {
            return;
        };
        let scene = parse_and_build_scene_from_col(&bytes, BaseOrientation::Zup)
            .expect("Vice City COL should decode");
        assert!(scene.has_geometry());
        assert!(scene.total_triangles() > 0);
        assert!(scene.aabb.bounding_radius().is_finite());
    }

    #[test]
    fn decoder_handles_bully_fixture_when_present() {
        let Some(stream) = crate::test_paths::bully_stream() else {
            return;
        };
        let bytes = match std::fs::read(stream.join("test1/1950Fridge.nif")) {
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

    #[test]
    fn decoder_routes_fixture_diffuse_names_per_mesh_when_present() {
        let Some(stream) = crate::test_paths::bully_stream() else {
            return;
        };
        let root = stream.parent().unwrap_or(stream.as_path());
        let bytes = match std::fs::read(stream.join("test1/1950Fridge.nif")) {
            Ok(b) => b,
            Err(_) => return,
        };
        let ide_map = crate::inspector::texture::IdeMap::build(root);
        let names = RefCell::new(Vec::new());
        let scene = parse_and_build_scene(&bytes, BaseOrientation::Yup, |name| {
            names.borrow_mut().push(name.to_string());
            ide_map
                .locate_external_texture(name)
                .and_then(|path| std::fs::read(path).ok())
                .and_then(|bytes| SceneTexture::from_tga(&bytes))
        })
        .expect("1950Fridge should decode");
        assert!(
            names
                .borrow()
                .iter()
                .any(|name| name.to_ascii_lowercase().contains(".tga")),
            "the scene graph should expose a diffuse texture reference"
        );
        assert!(
            scene.textured_mesh_count() > 0,
            "the fixture diffuse texture should render"
        );
    }

    #[test]
    fn decoder_preserves_bbagbottle_strip_topology_when_present() {
        let Some(root) = crate::test_paths::bully_nif_tools() else {
            return;
        };
        let bytes = match std::fs::read(root.join("1S01_bbagbottle.nif")) {
            Ok(bytes) => bytes,
            Err(_) => return,
        };
        let mut nif = NifFile::parse(&bytes).expect("bbagbottle fixture should parse");
        nif.resolve_string_indices();
        let strips = nif
            .payloads
            .iter()
            .filter_map(|payload| match payload.as_ref()? {
                crate::inspector::nif::BlockPayload::NiTriStripsData(data) => Some(data),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            strips.len(),
            2,
            "the bottle should contain two strip meshes"
        );
        assert!(strips.iter().all(|data| data.base.triangles.is_empty()));
        assert_eq!(
            strips
                .iter()
                .map(|data| data.num_triangles as usize)
                .sum::<usize>(),
            380
        );
        assert!(strips.iter().all(|data| {
            data.has_points
                && !data.points.is_empty()
                && data.strip_lengths.iter().sum::<u16>() as usize == data.points.len()
        }));

        let scene = build_scene_from_nif(&nif, BaseOrientation::Zup, |_| None)
            .expect("bbagbottle should produce a scene");
        assert_eq!(scene.total_vertices(), 262);
        assert_eq!(scene.total_triangles(), 160);
    }
}
