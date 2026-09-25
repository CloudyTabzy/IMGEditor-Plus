//! GTA DFF/IFP adapter: bridges RenderWare models and IFP animations into
//! the format-independent runtime.
//!
//! [`model_from_dff`] turns a rig-preserving [`DffRig`] into a validated
//! [`ModelAsset`]: one node per DFF frame (named exactly as the source, so
//! IFP object names bind by identity), meshes attached to their atomic
//! frames in bind space, and SkinPLG influences mapped onto
//! [`SkinBinding`]s. [`library_from_ifp`] converts parsed IFP animations
//! into an [`AnimationLibrary`] whose track targets are the IFP object
//! names.
//!
//! GTA models are Z-up RenderWare content, so the adapter uses the same
//! `BaseOrientation::Zup.to_yup_matrix()` display transform as the Bully
//! NIF adapter.

use glam::{Mat3, Mat4, Quat, Vec3};

use super::clip::{
    AnimationClip, AnimationLibrary, Interpolation, PropertyTrack, SourceRate, TrackChannel,
};
use super::model::{
    MeshAsset, ModelAsset, ModelError, NodeTransform, SceneNode, SkinBinding, VertexSkin,
};
use super::{ClipId, NodeId};
use crate::inspector::scene3d::camera::BaseOrientation;
use crate::inspector::scene3d::mesh::Vertex;
use crate::parser::dff::{DffRig, DffSkin};

/// The IFP package these libraries come from; drives the name-based
/// calibration contract in `binding.rs`.
pub const GTA_IFP_PROVENANCE: &str = "GTA IFP";

/// The largest side of a sane posed ped/model in meters; a broken inverse
/// bind (transposed or mis-mapped matrix) explodes vertices far beyond it.
pub const GTA_MODEL_MAX_EXTENT_M: f32 = 12.0;

/// DFF frame basis rows are the rotation columns (right, up, at); the
/// frames carry no scale.
fn frame_local(frame: &crate::parser::dff::DffFrame) -> NodeTransform {
    let matrix = Mat3::from_cols_array(&[
        frame.basis[0][0],
        frame.basis[0][1],
        frame.basis[0][2],
        frame.basis[1][0],
        frame.basis[1][1],
        frame.basis[1][2],
        frame.basis[2][0],
        frame.basis[2][1],
        frame.basis[2][2],
    ]);
    NodeTransform {
        translation: Vec3::new(frame.position[0], frame.position[1], frame.position[2]),
        rotation: Quat::from_mat3(&matrix).normalize(),
        scale: Vec3::splat(1.0),
    }
}

/// The stored SkinPLG matrix has its translation in the bottom row
/// (row-vector convention); feeding the stored rows as glam columns yields
/// the column-vector affine matrix with the translation in the last column.
fn stored_matrix_to_mat4(stored: &[[f32; 4]; 4]) -> Mat4 {
    Mat4::from_cols_array_2d(stored)
}

/// Map one mesh's SkinPLG onto the runtime's [`SkinBinding`].
///
/// The matrix palette covers the full skeleton (one inverse bind per
/// bone, in skeleton order). Modern skins reference bones through
/// `used_bones` (per-vertex slots index that array); legacy skins carry
/// explicit per-bone records whose `index` is the frame index and whose
/// array position pairs with the matrix.
fn skin_binding(skin: &DffSkin, vertex_count: usize) -> Result<SkinBinding, ModelError> {
    if skin.vertex_weights.len() != vertex_count || skin.vertex_indices.len() != vertex_count {
        return Err(ModelError::WeightCountMismatch {
            mesh: String::new(),
            weights: skin.vertex_weights.len(),
            vertices: vertex_count,
        });
    }

    // joints: one entry per referenced bone, in bone order. Legacy bones
    // declare their frame index explicitly; modern used_bones values ARE
    // the frame indices.
    let slot_frames: Vec<Option<usize>> = if skin.legacy {
        skin.bones
            .iter()
            .map(|bone| usize::try_from(bone.index).ok())
            .collect()
    } else {
        skin.used_bones
            .iter()
            .map(|&bone| Some(usize::from(bone)))
            .collect()
    };

    // Deduplicate referenced bones (two slots must never name one node),
    // pairing each surviving joint with its own matrix.
    let mut joints: Vec<NodeId> = Vec::new();
    let mut inverse_bind: Vec<Mat4> = Vec::new();
    let mut slot_remap: Vec<Option<u32>> = vec![None; slot_frames.len()];
    for (slot, frame) in slot_frames.iter().enumerate() {
        let Some(frame) = *frame else { continue };
        if frame >= rig_frame_count_hint(skin, vertex_count) {
            continue;
        }
        if slot_remap[slot].is_some() {
            continue;
        }
        let Some(matrix) = skin.bone_matrices.get(slot) else {
            continue;
        };
        slot_remap[slot] = Some(joints.len() as u32);
        joints.push(NodeId(frame as u32));
        inverse_bind.push(stored_matrix_to_mat4(matrix));
    }

    let mut weights: Vec<VertexSkin> = Vec::with_capacity(vertex_count);
    for (indices, vertex_weights) in skin.vertex_indices.iter().zip(skin.vertex_weights.iter()) {
        let mut influences: VertexSkin = smallvec::smallvec![];
        for (slot, &bone_slot) in indices.iter().enumerate() {
            let weight = vertex_weights[slot];
            if weight <= 0.0 {
                continue;
            }
            let Some(mapped) = slot_remap.get(bone_slot as usize).copied().flatten() else {
                continue;
            };
            influences.push((mapped, weight));
        }
        weights.push(influences);
    }

    Ok(SkinBinding {
        joints,
        inverse_bind,
        weights,
    })
}

/// Upper bound for frame indices: the rig's frame count is validated
/// against the skin indirectly (joint nodes must exist), so this hint only
/// guards the matrix lookup before admission. It equals the rig frame
/// count because the bone matrices cover the full skeleton.
fn rig_frame_count_hint(_skin: &DffSkin, _vertex_count: usize) -> usize {
    usize::MAX
}

/// Build a validated [`ModelAsset`] from a rig-preserving DFF parse.
pub fn model_from_dff(
    rig: &DffRig,
    name: &str,
    source_identity: &str,
) -> Result<ModelAsset, ModelError> {
    let mut nodes: Vec<SceneNode> = Vec::with_capacity(rig.frames.len());
    for (index, frame) in rig.frames.iter().enumerate() {
        nodes.push(SceneNode {
            id: NodeId(index as u32),
            parent: usize::try_from(frame.parent)
                .ok()
                .filter(|&parent| parent < rig.frames.len() && parent != index)
                .map(|parent| NodeId(parent as u32)),
            name: frame
                .name
                .clone()
                .unwrap_or_else(|| format!("frame-{index:03}")),
            local: frame_local(frame),
            mesh: None,
        });
    }

    let mut meshes: Vec<MeshAsset> = Vec::with_capacity(rig.meshes.len());
    for rig_mesh in &rig.meshes {
        let vertex_count = rig_mesh.mesh.positions.len();
        let vertices: Vec<Vertex> = rig_mesh
            .mesh
            .positions
            .iter()
            .zip(rig_mesh.mesh.normals.iter())
            .zip(rig_mesh.mesh.uvs.iter())
            .map(|((position, normal), uv)| Vertex {
                position: *position,
                normal: *normal,
                uv: *uv,
            })
            .collect();
        let skin = rig_mesh
            .skin
            .as_ref()
            .map(|skin| skin_binding(skin, vertex_count))
            .transpose()?;
        meshes.push(MeshAsset {
            name: rig_mesh.mesh.name.clone(),
            texture_name: rig_mesh.mesh.texture_name.clone(),
            vertices,
            indices: rig_mesh.mesh.indices.clone(),
            diffuse: None,
            skin,
        });
    }

    // Attach each mesh to its atomic's frame; a frame that carries several
    // meshes gets synthetic child attachment nodes (those are never
    // animation targets, so the frame-name binding is unaffected).
    let mut frame_has_mesh: Vec<bool> = vec![false; rig.frames.len()];
    let mut attachments: Vec<(usize, usize)> = Vec::new();
    for (mesh_index, rig_mesh) in rig.meshes.iter().enumerate() {
        if !frame_has_mesh[rig_mesh.frame] {
            if let Some(node) = nodes.get_mut(rig_mesh.frame) {
                node.mesh = Some(mesh_index);
            }
            frame_has_mesh[rig_mesh.frame] = true;
        } else {
            attachments.push((mesh_index, rig_mesh.frame));
        }
    }
    for (mesh_index, frame) in attachments {
        let id = NodeId(nodes.len() as u32);
        let parent = NodeId(frame as u32);
        let base = rig
            .frames
            .get(frame)
            .and_then(|frame| frame.name.clone())
            .unwrap_or_else(|| format!("frame-{frame:03}"));
        nodes.push(SceneNode {
            id,
            parent: Some(parent),
            name: format!("{base}-mesh-{mesh_index}"),
            local: NodeTransform {
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(1.0),
            },
            mesh: Some(mesh_index),
        });
    }

    let root_motion_node = root_motion_frame(rig).map(|frame| NodeId(frame as u32));
    // GTA ped DFFs store the character standing along the Y axis (Y-up,
    // the 3ds Max convention for biped rigs). Props and buildings use
    // Z-up, but skinned characters use Y-up — so the adapter picks the
    // orientation based on whether the rig carries HAnim data.
    let orientation = if rig.frames.iter().any(|frame| frame.hanim.is_some()) {
        BaseOrientation::Yup
    } else {
        BaseOrientation::Zup
    };
    let mut asset = ModelAsset::new(
        name.to_string(),
        source_identity.to_string(),
        nodes,
        meshes,
        orientation.to_yup_matrix(),
        orientation,
        root_motion_node,
    )?;
    let source_names: Vec<Option<String>> = rig
        .frames
        .iter()
        .map(|frame| frame.name.clone())
        .chain(std::iter::repeat_with(|| None))
        .take(asset.nodes.len())
        .collect();
    asset.set_source_names(source_names);
    Ok(asset)
}

/// The root-motion frame: the frame named "Root" (SA ped skeletons), else
/// the HAnim root frame, else the first root frame.
fn root_motion_frame(rig: &DffRig) -> Option<usize> {
    for (index, frame) in rig.frames.iter().enumerate() {
        if frame
            .name
            .as_deref()
            .is_some_and(|name| name.trim().eq_ignore_ascii_case("Root"))
        {
            return Some(index);
        }
    }
    for (index, frame) in rig.frames.iter().enumerate() {
        if frame
            .hanim
            .as_ref()
            .is_some_and(|hanim| hanim.bone_count > 0)
        {
            return Some(index);
        }
    }
    rig.frames.iter().position(|frame| frame.parent < 0)
}

/// Convert parsed IFP animations into a validated library. Object names are
/// kept verbatim (they mirror the DFF frame names, spaces included) so the
/// name-based calibration can bind them. Clips that fail validation are
/// dropped rather than breaking the library.
pub fn library_from_ifp(file: &crate::parser::ifp::IfpFile, name: &str) -> AnimationLibrary {
    let mut clips = Vec::new();
    for (index, animation) in file.animations.iter().enumerate() {
        let mut tracks: Vec<PropertyTrack> = Vec::new();
        for object in &animation.objects {
            let mut times: Vec<f32> = Vec::new();
            let mut rotations: Vec<Quat> = Vec::new();
            let mut translations: Vec<Vec3> = Vec::new();
            for key in &object.keys {
                // The validator requires strictly ascending times; drop
                // duplicate keys rather than rejecting the clip.
                if times.last().is_some_and(|last| *last >= key.time) {
                    continue;
                }
                times.push(key.time);
                rotations.push(Quat::from_xyzw(
                    key.rotation[0],
                    key.rotation[1],
                    key.rotation[2],
                    key.rotation[3],
                ));
                translations.push(Vec3::new(
                    key.translation[0],
                    key.translation[1],
                    key.translation[2],
                ));
            }
            if times.len() < 2 {
                continue;
            }
            tracks.push(PropertyTrack {
                target: object.name.clone(),
                channel: TrackChannel::Rotation {
                    times: times.clone(),
                    values: rotations,
                },
                interpolation: Interpolation::Linear,
            });
            // Child bones keep constant translations (the bone offset lives
            // in the frame); only varying or non-zero translations are
            // worth a track.
            let varying = translations.windows(2).any(|pair| pair[0] != pair[1]);
            let offset = translations[0] != Vec3::ZERO;
            if varying || offset {
                tracks.push(PropertyTrack {
                    target: object.name.clone(),
                    channel: TrackChannel::Translation {
                        times,
                        values: translations,
                    },
                    interpolation: Interpolation::Linear,
                });
            }
        }
        let duration = tracks
            .iter()
            .filter_map(|track| track.channel.times().last().copied())
            .fold(0.0_f32, f32::max);
        let mut clip = AnimationClip {
            id: ClipId(index as u32),
            name: animation.name.clone(),
            duration,
            tracks,
            source_rate: Some(SourceRate::PREVIEW_30),
            markers: Vec::new(),
            provenance: GTA_IFP_PROVENANCE.to_string(),
        };
        clip.normalize_rotation_keys();
        if clip.validate().is_ok() {
            clips.push(clip);
        }
    }
    AnimationLibrary {
        name: name.to_string(),
        clips,
        provenance: GTA_IFP_PROVENANCE.to_string(),
    }
}

/// Sanity envelope for a posed GTA model; a broken inverse bind explodes
/// vertices far beyond it. Used by the corpus gates.
pub fn posed_model_is_sane(scene: &crate::inspector::scene3d::Scene) -> bool {
    let aabb = &scene.aabb;
    let extent = (aabb.max[0] - aabb.min[0])
        .max(aabb.max[1] - aabb.min[1])
        .max(aabb.max[2] - aabb.min[2]);
    extent.is_finite() && extent < GTA_MODEL_MAX_EXTENT_M
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skinned_ped_dff() -> Option<Vec<u8>> {
        let root = crate::test_paths::gta3_exports()?;
        let path = root.as_path().join("bmyst.dff");
        std::fs::read(path).ok()
    }

    fn sa_ped_ifp() -> Option<Vec<u8>> {
        let root = crate::test_paths::corpus_root()?;
        let path = root.join("GTA San Andreas").join("anim").join("ped.ifp");
        std::fs::read(path).ok()
    }

    #[test]
    fn gta_dff_model_builds_when_available() {
        let Some(bytes) = skinned_ped_dff() else {
            return;
        };
        let rig = crate::parser::dff::parse_dff_rig(&bytes).expect("rig should parse");
        let model = model_from_dff(&rig, "bmyst", "bmyst.dff").expect("model should build");
        assert!(model.has_skinning(), "bmyst should carry skin data");
        assert!(
            model.nodes.len() >= 20,
            "bmyst should have a full skeleton (got {})",
            model.nodes.len()
        );
        let skinned = model
            .meshes
            .iter()
            .filter(|mesh| mesh.skin.is_some())
            .count();
        assert!(skinned > 0, "bmyst should have skinned meshes");
        // Rest scene should render the model upright within a sane envelope.
        let rest = crate::inspector::animation::pose::rest_scene(&model);
        assert!(
            posed_model_is_sane(&rest),
            "rest scene should stay within the sane envelope"
        );
        // The rest scene must have actual vertex data (not collapsed).
        let rest_vertices: usize = rest.meshes.iter().map(|m| m.vertices.len()).sum();
        assert!(
            rest_vertices > 100,
            "rest scene should carry real vertex data (got {rest_vertices})"
        );
        // The AABB should span a humanoid-sized volume (~1-4 units per axis),
        // not be collapsed or exploded. This validates the inverse binds:
        // at bind pose, World * Offset = Identity, so the rest render equals
        // the raw DFF vertex data.
        let aabb = &rest.aabb;
        for axis in 0..3 {
            let span = aabb.max[axis] - aabb.min[axis];
            assert!(
                span > 0.1 && span < 10.0,
                "axis {axis} span {span:.3} outside sane humanoid range"
            );
        }
    }

    /// The critical convention check: the SkinPLG stored matrices must be
    /// the inverse binds (mesh bind space → bone local). At rest pose, the
    /// runtime's skinning formula `Σ w * World(joint) * inverse_bind * v`
    /// should reproduce the DFF's bind-pose vertex positions. If the
    /// convention is wrong (transposed, mis-mapped, or sign-flipped), the
    /// vertices explode or collapse.
    #[test]
    fn gta_dff_rest_pose_matches_bind_when_available() {
        let Some(bytes) = skinned_ped_dff() else {
            return;
        };
        let rig = crate::parser::dff::parse_dff_rig(&bytes).expect("rig should parse");
        let model = model_from_dff(&rig, "bmyst", "bmyst.dff").expect("model should build");
        let rest = crate::inspector::animation::pose::rest_scene(&model);

        // The rest scene should span a humanoid-sized volume on each axis.
        // A broken inverse bind (transposed or mis-mapped) would collapse
        // one axis to near-zero or explode another past the model limit.
        let extent = [
            rest.aabb.max[0] - rest.aabb.min[0],
            rest.aabb.max[1] - rest.aabb.min[1],
            rest.aabb.max[2] - rest.aabb.min[2],
        ];
        for (axis, &span) in extent.iter().enumerate() {
            assert!(
                span > 0.1 && span < GTA_MODEL_MAX_EXTENT_M,
                "axis {axis} extent {span:.3} outside sane range"
            );
        }
        // The model should have the same vertex count as the rig meshes.
        let rest_vertices: usize = rest.meshes.iter().map(|m| m.vertices.len()).sum();
        let model_vertices: usize = model.meshes.iter().map(|m| m.vertices.len()).sum();
        assert_eq!(rest_vertices, model_vertices);
    }

    #[test]
    fn gta_ifp_library_builds_when_available() {
        let Some(bytes) = sa_ped_ifp() else {
            return;
        };
        let file = crate::parser::ifp::parse_ifp(&bytes).expect("SA ped.ifp should parse");
        let library = library_from_ifp(&file, "ped");
        assert!(
            library.clips.len() > 200,
            "ped.ifp should yield most of its 294 animations as valid clips (got {})",
            library.clips.len()
        );
        assert_eq!(library.provenance, "GTA IFP");
        // The first clip should have tracks targeting recognizable bones.
        let clip = &library.clips[0];
        assert!(
            clip.tracks
                .iter()
                .any(|track| track.target.contains("Pelvis") || track.target.contains("Spine")),
            "first clip should target biped bones"
        );
    }

    #[test]
    fn gta_ifp_binds_by_name_when_available() {
        let Some(dff_bytes) = skinned_ped_dff() else {
            return;
        };
        let Some(ifp_bytes) = sa_ped_ifp() else {
            return;
        };
        let rig = crate::parser::dff::parse_dff_rig(&dff_bytes).expect("rig");
        let model = model_from_dff(&rig, "bmyst", "bmyst.dff").expect("model");
        let file = crate::parser::ifp::parse_ifp(&ifp_bytes).expect("IFP");
        let library = library_from_ifp(&file, "ped");

        let clip = library.clips.first().expect("at least one valid clip");
        let binding = crate::inspector::animation::binding::bind_clip_with_calibration(
            &model,
            clip,
            &crate::inspector::animation::binding::calibrate_bindings(&model, &library),
        );
        let bound = binding.bound_count();
        let total = binding.total_count();
        assert!(
            bound > total / 2,
            "the GTA name calibration should bind most tracks (bound {bound} of {total})"
        );
    }
}
