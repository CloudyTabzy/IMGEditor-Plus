//! Pose evaluation: sampling, hierarchy composition, CPU skinning.
//!
//! `sample_locals` + [`evaluate_pose`] are deterministic and independent
//! of GUI clocks: seeking directly to a time produces the same pose as
//! reaching that time during playback. All deformation runs against
//! caller-owned scratch buffers ([`PoseBuffers`]) so steady-state playback
//! allocates nothing.
//!
//! Skinning contract (column vectors, one mesh-local palette convention):
//!
//! ```text
//! Pj(t) = inverse(Gm(t)) * Gj(t) * Bj          (mesh-local joint palette)
//! v_view = display * source_to_view * Gm(t) * sum(weight_j * Pj(t)) * v_bind
//! ```
//!
//! Since `Gm` cancels inside the sum for fully-weighted vertices, the
//! per-joint matrix used here is `display * source_to_view * Gj * Bj`;
//! vertices without influences follow the rigid mesh-node matrix
//! `display * source_to_view * Gm`. Normals use the inverse-transpose of
//! the effective (summed) linear transform, then normalize — the
//! reference shading rule any future GPU path must agree with.

use glam::{Mat3, Mat4, Vec3};

use crate::inspector::scene3d::mesh::{Aabb, Vertex};
use crate::inspector::scene3d::scene::Scene;

use crate::inspector::animation::binding::ClipBinding;
use crate::inspector::animation::clip::{AnimationClip, SampledChannel};
use crate::inspector::animation::model::{ModelAsset, NodeTransform};

/// Root-motion handling for the designated root node's translation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RootMotionPolicy {
    /// Play the authored root translation; each loop cycle restarts the
    /// cycle's displacement.
    #[default]
    Source,
    /// Remove the horizontal motion (perpendicular to the model-space
    /// ground normal) while retaining meaningful vertical motion such as
    /// a jump. Only offered once a root track/binding is identified.
    InPlace,
}

/// Reusable scratch plus the latest evaluated result. Per instance: two
/// instances may share a model but never share pose buffers.
#[derive(Clone, Debug)]
pub struct PoseBuffers {
    /// Sampled local transforms (after root-motion policy), per node.
    pub locals: Vec<NodeTransform>,
    /// Model-space world matrices, per node.
    pub world: Vec<Mat4>,
    /// Per-mesh joint palettes (`display * source_to_view * Gj * Bj`),
    /// parallel to `model.meshes`; empty for rigid meshes.
    pub palettes: Vec<Vec<Mat4>>,
    /// Deformed vertices in display space (view space + display offset),
    /// per mesh, same order and length as the asset's vertices.
    pub out_vertices: Vec<Vec<Vertex>>,
    /// Per-node world origin in display space (skeleton overlay, root
    /// follow, bone labels).
    pub node_positions_view: Vec<Vec3>,
    /// Posed bounds over all deformed vertices, in display space.
    pub posed_bounds: Option<Aabb>,
    /// Monotonically increasing pose revision.
    pub revision: u64,
    /// Non-finite/singular normal fallbacks hit during the last evaluate.
    pub degenerate_normal_count: u32,
    /// `true` when any node world matrix mirrors (negative determinant);
    /// the renderer must not cull backfaces for such a pose.
    pub mirrored: bool,
}

impl PoseBuffers {
    pub fn new(model: &ModelAsset) -> Self {
        Self {
            locals: model.default_locals(),
            world: vec![Mat4::IDENTITY; model.nodes.len()],
            palettes: model
                .meshes
                .iter()
                .map(|mesh| {
                    vec![Mat4::IDENTITY; mesh.skin.as_ref().map_or(0, |skin| skin.joints.len())]
                })
                .collect(),
            out_vertices: model
                .meshes
                .iter()
                .map(|mesh| mesh.vertices.clone())
                .collect(),
            node_positions_view: vec![Vec3::ZERO; model.nodes.len()],
            posed_bounds: None,
            revision: 0,
            degenerate_normal_count: 0,
            mirrored: false,
        }
    }
}

/// Root-motion extraction modifies only the sampled locals; call this
/// before composing the hierarchy.
pub fn apply_root_motion_policy(
    model: &ModelAsset,
    policy: RootMotionPolicy,
    locals: &mut [NodeTransform],
) {
    if policy != RootMotionPolicy::InPlace {
        return;
    }
    let Some(root) = model.root_motion_node else {
        return;
    };
    let index = root.0 as usize;
    let rest = model.nodes[index].local.translation;
    let sampled = locals[index].translation;
    let up = model.ground_normal_model();
    let delta = sampled - rest;
    // Keep only the vertical component (along the ground normal); remove
    // the horizontal travel such as walk forward displacement.
    let vertical = up * delta.dot(up);
    locals[index].translation = rest + vertical;
}

/// Sample every bound track of `clip` at clip-local `time` into `locals`,
/// starting from the model's default local transforms. Absent channels
/// keep the default pose values; unbound tracks are skipped (a partial
/// preview stays labelled via the binding, never silently retargeted).
pub fn sample_locals(
    clip: &AnimationClip,
    binding: &ClipBinding,
    model: &ModelAsset,
    time: f32,
    locals: &mut [NodeTransform],
) {
    debug_assert_eq!(locals.len(), model.nodes.len());
    for (index, node) in model.nodes.iter().enumerate() {
        locals[index] = node.local;
    }
    let time = time.clamp(0.0, clip.duration.max(0.0));
    for (track_index, track) in clip.tracks.iter().enumerate() {
        let Some(node) = binding.node_for_track(track_index) else {
            continue;
        };
        let local = &mut locals[node.0 as usize];
        match AnimationClip::sample_track(track, time) {
            SampledChannel::Translation(value) => local.translation = value,
            SampledChannel::Rotation(value) => local.rotation = value,
            SampledChannel::Scale(value) => local.scale = value,
        }
    }
}

/// Blend two sampled local poses (clip switching/crossfade). Blending
/// happens in local space before hierarchy composition; missing channels
/// in either input already carry default pose values from `sample_locals`.
pub fn blend_locals(a: &[NodeTransform], b: &[NodeTransform], t: f32, out: &mut [NodeTransform]) {
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), out.len());
    let t = t.clamp(0.0, 1.0);
    for ((&from, &to), blended) in a.iter().zip(b.iter()).zip(out.iter_mut()) {
        *blended = NodeTransform {
            translation: from.translation.lerp(to.translation, t),
            rotation: from.rotation.slerp(to.rotation, t).normalize(),
            scale: from.scale.lerp(to.scale, t),
        };
    }
}

/// Evaluate the full pose: compose the hierarchy, derive skin palettes,
/// deform every mesh into display space, and refresh bounds and node
/// positions. `display_offset` is the presentation recentering computed
/// once from rest bounds (or explicit recenter), never from the pose.
pub fn evaluate_pose(
    model: &ModelAsset,
    root_policy: RootMotionPolicy,
    display_offset: Vec3,
    buffers: &mut PoseBuffers,
) {
    apply_root_motion_policy(model, root_policy, &mut buffers.locals);
    model.compose_world_into(&buffers.locals, &mut buffers.world);

    let view = Mat4::from_translation(display_offset) * model.source_to_view;
    buffers.degenerate_normal_count = 0;
    buffers.mirrored = false;
    for &id in &model.eval_order {
        let index = id.0 as usize;
        if buffers.world[index].determinant() < 0.0 {
            buffers.mirrored = true;
        }
        buffers.node_positions_view[index] =
            view.transform_point3(buffers.world[index].transform_point3(Vec3::ZERO));
    }

    let mut bounds: Option<Aabb> = None;
    for (mesh_index, mesh) in model.meshes.iter().enumerate() {
        let node_world = model
            .mesh_node(mesh_index)
            .map(|node| buffers.world[node.0 as usize])
            .unwrap_or(Mat4::IDENTITY);
        let rigid = view * node_world;
        let out = &mut buffers.out_vertices[mesh_index];
        match &mesh.skin {
            Some(skin) => {
                let palette = &mut buffers.palettes[mesh_index];
                for (slot, &joint) in skin.joints.iter().enumerate() {
                    palette[slot] =
                        view * buffers.world[joint.0 as usize] * skin.inverse_bind[slot];
                }
                let palette = &buffers.palettes[mesh_index];
                for (vertex_index, (source, out)) in
                    mesh.vertices.iter().zip(out.iter_mut()).enumerate()
                {
                    let influences = &skin.weights[vertex_index];
                    deform_vertex(
                        source,
                        influences,
                        palette,
                        rigid,
                        &mut buffers.degenerate_normal_count,
                        out,
                    );
                }
            }
            None => {
                for (source, out) in mesh.vertices.iter().zip(out.iter_mut()) {
                    deform_rigid_vertex(source, rigid, &mut buffers.degenerate_normal_count, out);
                }
            }
        }
        for vertex in out.iter() {
            bounds = Some(match bounds {
                Some(bounds) => bounds.merged(Aabb {
                    min: vertex.position,
                    max: vertex.position,
                }),
                None => Aabb {
                    min: vertex.position,
                    max: vertex.position,
                },
            });
        }
    }
    buffers.posed_bounds = bounds;
    buffers.revision = buffers.revision.wrapping_add(1);
}

fn deform_vertex(
    source: &Vertex,
    influences: &[(u32, f32)],
    palette: &[Mat4],
    rigid: Mat4,
    degenerate_normals: &mut u32,
    out: &mut Vertex,
) {
    out.uv = source.uv;
    if influences.is_empty() {
        // Zero-influence vertices follow the mesh node rigidly.
        deform_rigid_vertex(source, rigid, degenerate_normals, out);
        return;
    }
    let position = Vec3::from(source.position);
    let normal = Vec3::from(source.normal);
    let mut blended_position = Vec3::ZERO;
    let mut blended_linear = Mat3::ZERO;
    let mut dominant = 0usize;
    let mut dominant_weight = f32::MIN;
    for &(slot, weight) in influences {
        let transform = palette[slot as usize];
        blended_position += weight * transform.transform_point3(position);
        blended_linear += weight * linear_part(transform);
        if weight > dominant_weight {
            dominant_weight = weight;
            dominant = slot as usize;
        }
    }
    out.position = blended_position.to_array();
    out.normal = blend_normal(
        normal,
        blended_linear,
        linear_part(palette[dominant]),
        linear_part(rigid),
        degenerate_normals,
    )
    .to_array();
}

fn deform_rigid_vertex(
    source: &Vertex,
    rigid: Mat4,
    degenerate_normals: &mut u32,
    out: &mut Vertex,
) {
    out.position = rigid
        .transform_point3(Vec3::from(source.position))
        .to_array();
    out.uv = source.uv;
    let linear = linear_part(rigid);
    out.normal = normal_through(linear, Vec3::from(source.normal), degenerate_normals).to_array();
}

fn linear_part(transform: Mat4) -> Mat3 {
    Mat3::from_mat4(transform)
}

/// Reference normal rule: inverse-transpose of the effective linear
/// transform, normalized. Near-singular transforms fall back to the
/// dominant joint's linear part, then to the rigid mesh transform, and
/// finally keep the bind normal — each fallback is counted so the
/// diagnostic report can name it.
fn blend_normal(
    bind_normal: Vec3,
    blended: Mat3,
    dominant: Mat3,
    rigid: Mat3,
    degenerate_normals: &mut u32,
) -> Vec3 {
    const MIN_DET: f32 = 1e-10;
    if blended.determinant().abs() > MIN_DET {
        return normal_through(blended, bind_normal, degenerate_normals);
    }
    *degenerate_normals += 1;
    if dominant.determinant().abs() > MIN_DET {
        return normal_through(dominant, bind_normal, degenerate_normals);
    }
    if rigid.determinant().abs() > MIN_DET {
        return normal_through(rigid, bind_normal, degenerate_normals);
    }
    bind_normal
}

fn normal_through(linear: Mat3, normal: Vec3, degenerate_normals: &mut u32) -> Vec3 {
    let transformed = linear.inverse().transpose() * normal;
    match transformed.try_normalize() {
        Some(normal) if normal.is_finite() => normal,
        _ => {
            *degenerate_normals += 1;
            Vec3::Y
        }
    }
}

/// Evaluate the rest pose (default locals, source root motion, no display
/// offset) and flatten it into the existing static [`Scene`] container.
/// Mesh order matches `model.meshes`, which keeps the renderer's mesh
/// cache indexable identically for the static and animated paths.
pub fn rest_scene(model: &ModelAsset) -> Scene {
    let mut buffers = PoseBuffers::new(model);
    evaluate_pose(model, RootMotionPolicy::Source, Vec3::ZERO, &mut buffers);
    scene_from_pose(model, &buffers)
}

/// Flatten an already-evaluated pose into the static [`Scene`] container.
/// Shared by the rest scene, the animation viewer, and headless test
/// renders so every consumer sees identical geometry.
pub fn scene_from_pose(model: &ModelAsset, buffers: &PoseBuffers) -> Scene {
    let mut meshes = Vec::with_capacity(model.meshes.len());
    for (mesh_index, mesh) in model.meshes.iter().enumerate() {
        let vertices = buffers.out_vertices[mesh_index].clone();
        let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.position).collect();
        meshes.push(crate::inspector::scene3d::mesh::SceneMesh {
            name: mesh.name.clone(),
            texture_name: mesh.texture_name.clone(),
            vertices,
            indices: mesh.indices.clone(),
            diffuse: mesh.diffuse.clone(),
            aabb: Aabb::from_points(&positions).unwrap_or_default(),
        });
    }
    let mut scene = Scene::empty(model.source_orientation);
    scene.meshes = meshes;
    scene.aabb = buffers.posed_bounds.unwrap_or_default();
    scene
}

/// Constant grounding offset for one clip: the negated lowest excursion of
/// any deformed vertex along the model ground normal, sampled uniformly over
/// the clip. Character clips are authored rotation-only; prone and lying
/// clips then pivot at standing height and the contact points (hands, feet)
/// hang in the air. Planting the clip's lowest excursion on the floor
/// mirrors the runtime's actor grounding, is a sub-centimetre no-op for
/// standing clips, and - being one constant per clip - cannot bob between
/// frames the way per-frame grounding would.
pub fn clip_ground_offset(
    model: &ModelAsset,
    clip: &AnimationClip,
    binding: &ClipBinding,
    samples: usize,
    buffers: &mut PoseBuffers,
) -> Vec3 {
    let up = model.ground_normal_model();
    let mut lowest = f32::INFINITY;
    let samples = samples.max(2);
    for step in 0..samples {
        let time = clip.duration * step as f32 / (samples - 1) as f32;
        sample_locals(clip, binding, model, time, &mut buffers.locals);
        evaluate_pose(model, RootMotionPolicy::Source, Vec3::ZERO, buffers);
        for mesh_vertices in &buffers.out_vertices {
            for vertex in mesh_vertices {
                let position = Vec3::from_array(vertex.position);
                lowest = lowest.min(position.dot(up));
            }
        }
    }
    if lowest.is_finite() {
        -up * lowest
    } else {
        Vec3::ZERO
    }
}

/// Sampled motion envelope of a clip over `[start, end]`: the union of
/// posed bounds at uniformly spaced sample times. This is an estimate -
/// uniform sampling can miss extrema during rotations - so it is labelled
/// "sampled" and never used alone to cull moving geometry.
pub fn clip_envelope(
    model: &ModelAsset,
    clip: &AnimationClip,
    binding: &ClipBinding,
    range: (f32, f32),
    samples: usize,
    root_policy: RootMotionPolicy,
    display_offset: Vec3,
) -> Option<Aabb> {
    let (start, end) = range;
    if !start.is_finite() || !end.is_finite() || start > end {
        return None;
    }
    let samples = samples.clamp(2, 256);
    let mut buffers = PoseBuffers::new(model);
    let mut envelope: Option<Aabb> = None;
    for step in 0..samples {
        let t = if samples == 1 {
            start
        } else {
            start + (end - start) * (step as f32 / (samples - 1) as f32)
        };
        sample_locals(clip, binding, model, t, &mut buffers.locals);
        evaluate_pose(model, root_policy, display_offset, &mut buffers);
        if let Some(bounds) = buffers.posed_bounds {
            envelope = Some(match envelope {
                Some(acc) => acc.merged(bounds),
                None => bounds,
            });
        }
    }
    envelope
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspector::animation::ClipId;
    use crate::inspector::animation::binding::bind_clip;
    use crate::inspector::animation::clip::{Interpolation, PropertyTrack, TrackChannel};
    use crate::inspector::animation::fixtures;

    fn approx_vec3(a: Vec3, b: Vec3) -> bool {
        a.abs_diff_eq(b, 1e-4)
    }

    #[test]
    fn rest_pose_matches_default_pose_evaluation() {
        let (model, library) = fixtures::two_joint_strip();
        let clip = library.clip(ClipId(0)).unwrap();
        let binding = bind_clip(&model, clip);
        let mut buffers = PoseBuffers::new(&model);
        sample_locals(clip, &binding, &model, 0.0, &mut buffers.locals);
        evaluate_pose(&model, RootMotionPolicy::Source, Vec3::ZERO, &mut buffers);
        let rest = rest_scene(&model);
        for (a, b) in buffers.out_vertices[0]
            .iter()
            .zip(rest.meshes[0].vertices.iter())
        {
            assert!(
                approx_vec3(a.position.into(), b.position.into()),
                "rest flatten must equal pose evaluation at rest"
            );
        }
    }

    #[test]
    fn parent_rotation_moves_child_prop() {
        let (model, library) = fixtures::parent_child_prop();
        let clip = library.find_by_name("rotate-parent").unwrap();
        let binding = bind_clip(&model, clip);
        let mut buffers = PoseBuffers::new(&model);
        // At t=1.0 the parent is rotated 90° about Z: the child prop,
        // locally offset +Y by 2 from the parent, ends up at parent
        // position + R * (0,2,0).
        sample_locals(clip, &binding, &model, 1.0, &mut buffers.locals);
        evaluate_pose(&model, RootMotionPolicy::Source, Vec3::ZERO, &mut buffers);
        let child = model.node_by_name("prop").unwrap().id;
        let position = buffers.node_positions_view[child.0 as usize];
        assert!(
            approx_vec3(position, Vec3::new(-2.0, 1.0, 0.0)),
            "child at {position:?}, expected (-2, 1, 0)"
        );
    }

    #[test]
    fn two_joint_bend_matches_analytical_positions() {
        let (model, library) = fixtures::two_joint_strip();
        let clip = library.find_by_name("bend").unwrap();
        let binding = bind_clip(&model, clip);
        let mut buffers = PoseBuffers::new(&model);
        // 90° bend of the upper joint at t=1.0.
        sample_locals(clip, &binding, &model, 1.0, &mut buffers.locals);
        evaluate_pose(&model, RootMotionPolicy::Source, Vec3::ZERO, &mut buffers);
        let out = &buffers.out_vertices[0];
        for (expected, vertex) in fixtures::two_joint_strip_bent_positions()
            .iter()
            .zip(out.iter())
        {
            assert!(
                approx_vec3((*expected).into(), vertex.position.into()),
                "vertex at {:?}, expected {expected:?}",
                vertex.position
            );
        }
        // Bounds must contain every deformed vertex.
        let bounds = buffers.posed_bounds.unwrap();
        for vertex in out {
            for axis in 0..3 {
                assert!(vertex.position[axis] >= bounds.min[axis] - 1e-4);
                assert!(vertex.position[axis] <= bounds.max[axis] + 1e-4);
            }
        }
    }

    #[test]
    fn missing_channels_keep_default_pose_values() {
        let (model, library) = fixtures::parent_child_prop();
        let clip = library.find_by_name("rotate-parent").unwrap();
        let binding = bind_clip(&model, clip);
        let mut buffers = PoseBuffers::new(&model);
        sample_locals(clip, &binding, &model, 0.5, &mut buffers.locals);
        // The clip only rotates the parent; the child's local offset and
        // every scale stay at the authored defaults.
        let child = model.node_by_name("prop").unwrap().id;
        let local = buffers.locals[child.0 as usize];
        assert_eq!(
            local.translation,
            model.nodes[child.0 as usize].local.translation
        );
        assert_eq!(local.scale, Vec3::ONE);
    }

    #[test]
    fn in_place_policy_removes_horizontal_but_keeps_vertical() {
        let (model, library) = fixtures::walker();
        let clip = library.find_by_name("walk").unwrap();
        let binding = bind_clip(&model, clip);
        let mut buffers = PoseBuffers::new(&model);
        sample_locals(clip, &binding, &model, 0.5, &mut buffers.locals);
        evaluate_pose(&model, RootMotionPolicy::Source, Vec3::ZERO, &mut buffers);
        let root = model.root_motion_node.unwrap();
        let travelled = buffers.node_positions_view[root.0 as usize];

        sample_locals(clip, &binding, &model, 0.5, &mut buffers.locals);
        evaluate_pose(&model, RootMotionPolicy::InPlace, Vec3::ZERO, &mut buffers);
        let stayed = buffers.node_positions_view[root.0 as usize];
        assert!(
            stayed.z.abs() < 1e-4,
            "in-place must remove forward travel, got {stayed:?}"
        );
        assert!(
            (stayed.y - travelled.y).abs() < 1e-4,
            "vertical bob must survive in-place: {stayed:?} vs {travelled:?}"
        );
        assert!(travelled.z > 0.1, "source motion must actually travel");
    }

    #[test]
    fn envelope_covers_walk_travel() {
        let (model, library) = fixtures::walker();
        let clip = library.find_by_name("walk").unwrap();
        let binding = bind_clip(&model, clip);
        let envelope = clip_envelope(
            &model,
            clip,
            &binding,
            (0.0, clip.duration),
            16,
            RootMotionPolicy::Source,
            Vec3::ZERO,
        )
        .unwrap();
        assert!(envelope.max[2] >= 1.4, "envelope must reach travel end");
    }

    #[test]
    fn zup_fixture_enters_view_space_through_one_transform() {
        let (model, library) = fixtures::zup_prop();
        let clip = library.find_by_name("rise").unwrap();
        let binding = bind_clip(&model, clip);
        let mut buffers = PoseBuffers::new(&model);
        sample_locals(clip, &binding, &model, 1.0, &mut buffers.locals);
        evaluate_pose(&model, RootMotionPolicy::Source, Vec3::ZERO, &mut buffers);
        let node = model.node_by_name("platform").unwrap().id;
        let position = buffers.node_positions_view[node.0 as usize];
        // Source +Z (up in a Z-up source) must appear as view +Y.
        assert!(
            approx_vec3(position, Vec3::new(0.0, 2.0, 0.0)),
            "Z-up source height must become view Y, got {position:?}"
        );
    }

    #[test]
    fn unbound_tracks_are_skipped_not_retargeted() {
        let (model, _library) = fixtures::two_joint_strip();
        // A clip authored against a different skeleton must not bind.
        let foreign = AnimationClip {
            id: ClipId(9),
            name: "foreign".into(),
            duration: 1.0,
            tracks: vec![PropertyTrack {
                target: "NoSuchNode".into(),
                channel: TrackChannel::Translation {
                    times: vec![0.0, 1.0],
                    values: vec![Vec3::ZERO, Vec3::X],
                },
                interpolation: Interpolation::Linear,
            }],
            source_rate: None,
            markers: Vec::new(),
            provenance: "test".into(),
        };
        let binding = bind_clip(&model, &foreign);
        let mut buffers = PoseBuffers::new(&model);
        sample_locals(&foreign, &binding, &model, 0.5, &mut buffers.locals);
        evaluate_pose(&model, RootMotionPolicy::Source, Vec3::ZERO, &mut buffers);
        let rest = rest_scene(&model);
        for (a, b) in buffers.out_vertices[0]
            .iter()
            .zip(rest.meshes[0].vertices.iter())
        {
            assert!(approx_vec3(a.position.into(), b.position.into()));
        }
    }

    #[test]
    fn mirrored_pose_is_flagged() {
        let (mut model, _library) = fixtures::parent_child_prop();
        let parent = model.node_by_name("arm").unwrap().id.0 as usize;
        model.nodes[parent].local.scale = Vec3::new(-1.0, 1.0, 1.0);
        let mut buffers = PoseBuffers::new(&model);
        evaluate_pose(&model, RootMotionPolicy::Source, Vec3::ZERO, &mut buffers);
        assert!(buffers.mirrored);
    }

    #[test]
    fn node_ids_stay_stable() {
        let (model, _) = fixtures::two_joint_strip();
        let spine = model.node_by_name("spine").unwrap().id;
        assert_eq!(model.node(spine).unwrap().name, "spine");
    }
}
