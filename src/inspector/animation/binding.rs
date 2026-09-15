//! Clip-to-model binding: resolve each track's source target identity to
//! a runtime node, keeping ambiguous, unmatched and incompatible targets
//! visible. A partial preview is labelled; unbound channels retain
//! default pose values. Clips are never silently retargeted between
//! different skeletons.

use crate::inspector::animation::clip::AnimationClip;
use crate::inspector::animation::model::ModelAsset;
use crate::inspector::animation::{ClipId, NodeId};

/// How a verified numeric run relates the AGR curve space to the imported
/// node order.
///
/// One shape is verified across the retail corpus: the character stream
/// lists the semantic root's descendants in imported order, starting at the
/// root's first child. The `Dummy` placeholder is never an animation target;
/// sibling wrappers (`JKGirl_Mandy`, `MAINPED` body branches) only shift the
/// normalized numbering (`PLAYER` and `RAT_PED`: `track_i -> track_(i + 1)`;
/// the wrapper-heavy `JKGirl_Mandy`: `track_i -> track_(i + 3)`).
///
/// Recover the fixed source/export ordering used by Bully's character AGR
/// tracks when bind-pose scoring cannot admit an action-only track. The
/// regular calibrator deliberately rejects a curve that never approaches a
/// node's rest rotation; that is correct for unknown rigs, but a self-
/// contained action library can legitimately keep the root and torso far
/// from rest for every clip.
///
/// This is intentionally a narrow adapter contract, not a general numeric
/// retargeter. It is admitted only with its full structural signature: the
/// semantic root is the preserved `Dummy` node under `Scene Root`, targets
/// form a contiguous `track_000...` run, the covered prefix selects unique
/// non-mesh skin candidates, and the first curve targets the placeholder's
/// first child. Anything else returns `None` and leaves the pose calibration
/// in charge.
fn bully_numeric_track_offset(
    model: &ModelAsset,
    library: &crate::inspector::animation::clip::AnimationLibrary,
    targets: &[(String, Vec<crate::inspector::animation::clip::TrackChannel>)],
    candidates: &[(String, NodeId)],
) -> Option<std::collections::HashMap<String, NodeId>> {
    use std::collections::HashSet;

    const MIN_CHARACTER_TRACKS: usize = 8;

    if !model.has_skinning()
        || model.source_orientation != crate::inspector::scene3d::camera::BaseOrientation::Zup
        || !library.provenance.starts_with("Bully AGR")
        || targets.len() < MIN_CHARACTER_TRACKS
    {
        return None;
    }

    // The adapter emits one contiguous numeric target run per clip.
    for (curve_index, (target, _)) in targets.iter().enumerate() {
        if target != &format!("track_{curve_index:03}") {
            return None;
        }
    }

    let candidate_ids: HashSet<NodeId> = candidates.iter().map(|(_, id)| *id).collect();

    let scene_root = model.node_by_name("Scene Root")?.id;
    let root_motion = model.root_motion_node?;
    let root_node = model.node(root_motion)?;
    if root_node.parent != Some(scene_root)
        || root_node.mesh.is_some()
        || !candidate_ids.contains(&root_motion)
        || !model
            .source_name(root_motion)
            .is_some_and(|name| name.eq_ignore_ascii_case("Dummy"))
    {
        return None;
    }

    // The stream covers a prefix of the semantic root's non-mesh subtree,
    // skipping the placeholder itself (trailing helper attachments such as
    // `ARROW` may stay unanimated when the library is shorter).
    let covered: Vec<NodeId> = model
        .nodes
        .iter()
        .filter(|node| node.mesh.is_none() && is_descendant(model, node.id, root_motion))
        .map(|node| node.id)
        .skip(1)
        .take(targets.len())
        .collect();
    if covered.len() != targets.len()
        || model.node(covered[0])?.parent != Some(root_motion)
        || !covered.iter().all(|node| candidate_ids.contains(node))
    {
        return None;
    }

    Some(
        targets
            .iter()
            .zip(&covered)
            .map(|((target, _), node)| (target.clone(), *node))
            .collect(),
    )
}

fn is_descendant(model: &ModelAsset, mut id: NodeId, ancestor: NodeId) -> bool {
    loop {
        if id == ancestor {
            return true;
        }
        match model.node(id).and_then(|node| node.parent) {
            Some(parent) => id = parent,
            None => return false,
        }
    }
}

/// Resolution status of one track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrackBindingStatus {
    /// Exactly one node carries the track's source identity.
    Bound,
    /// Several nodes share the identity; a user-confirmed mapping is
    /// required before this track may drive anything.
    Ambiguous(Vec<NodeId>),
    /// No node carries the identity (different skeleton/namespace).
    Missing,
}

/// Binding record for one clip track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackBinding {
    pub node: Option<NodeId>,
    pub status: TrackBindingStatus,
}

impl TrackBinding {
    pub fn is_bound(&self) -> bool {
        self.node.is_some()
    }
}

/// Validated target-node mapping for one clip/model pair.
#[derive(Clone, Debug)]
pub struct ClipBinding {
    pub clip: ClipId,
    /// Parallel to `clip.tracks`.
    pub tracks: Vec<TrackBinding>,
    /// Human-readable diagnostics for unmatched/ambiguous targets.
    pub diagnostics: Vec<String>,
}

impl ClipBinding {
    pub fn node_for_track(&self, track: usize) -> Option<NodeId> {
        self.tracks.get(track).and_then(|binding| binding.node)
    }

    pub fn bound_count(&self) -> usize {
        self.tracks
            .iter()
            .filter(|binding| binding.is_bound())
            .count()
    }

    pub fn total_count(&self) -> usize {
        self.tracks.len()
    }

    pub fn is_fully_bound(&self) -> bool {
        self.total_count() > 0 && self.bound_count() == self.total_count()
    }
}

/// Resolve a clip's track targets against a model's node names.
/// Resolution preference is exact source identity (case-sensitive name
/// match); multiple or missing matches never fall back to a guess.
pub fn bind_clip(model: &ModelAsset, clip: &AnimationClip) -> ClipBinding {
    let mut tracks = Vec::with_capacity(clip.tracks.len());
    let mut diagnostics = Vec::new();
    for track in &clip.tracks {
        let matches = model.nodes_named(&track.target);
        let binding = match matches.as_slice() {
            [only] => TrackBinding {
                node: Some(*only),
                status: TrackBindingStatus::Bound,
            },
            [] => {
                diagnostics.push(format!(
                    "track target '{}' has no node in model '{}'",
                    track.target, model.name
                ));
                TrackBinding {
                    node: None,
                    status: TrackBindingStatus::Missing,
                }
            }
            many => {
                diagnostics.push(format!(
                    "track target '{}' matches {} nodes in model '{}'; binding needs an explicit choice",
                    track.target,
                    many.len(),
                    model.name
                ));
                TrackBinding {
                    node: None,
                    status: TrackBindingStatus::Ambiguous(many.to_vec()),
                }
            }
        };
        tracks.push(binding);
    }
    ClipBinding {
        clip: clip.id,
        tracks,
        diagnostics,
    }
}

/// Cross-clip binding calibration for one model/library pair. A curve's
/// keys pass through (or rest at) its target bone's rest rotation, but a
/// single clip may pose that bone far from rest for its whole duration;
/// scanning every clip's keys still catches the passage. Built once per
/// session and reused for every clip selection.
#[derive(Clone, Debug, Default)]
pub struct BindingCalibration {
    assignments: std::collections::HashMap<String, crate::inspector::animation::NodeId>,
    pub diagnostics: Vec<String>,
}

/// Minimum angle (degrees) between a target's rotation keys and a node's
/// rest rotation; `None` when either side is missing or unusable.
fn rotation_min_angle(
    channels: &[crate::inspector::animation::clip::TrackChannel],
    rest: &crate::inspector::animation::model::NodeTransform,
) -> Option<f32> {
    use crate::inspector::animation::clip::TrackChannel;

    if !rest.rotation.is_finite() || rest.rotation.length_squared() <= f32::EPSILON {
        return None;
    }
    let rest = rest.rotation.normalize();
    channels
        .iter()
        .filter_map(|channel| match channel {
            TrackChannel::Rotation { values, .. } => Some(values.as_slice()),
            TrackChannel::Translation { .. } | TrackChannel::Scale { .. } => None,
        })
        .flat_map(|values| values.iter())
        .filter_map(|key| {
            if !key.is_finite() || key.length_squared() <= f32::EPSILON {
                return None;
            }
            let key = key.normalize();
            let dot = key.dot(rest).abs().clamp(-1.0, 1.0);
            Some((dot.acos() * 2.0).to_degrees())
        })
        .reduce(f32::min)
}

/// Numeric part of a generated `track_NNN` identity.
fn track_number(name: &str) -> Option<i64> {
    name.strip_prefix("track_")
        .and_then(|rest| rest.parse::<i64>().ok())
}

/// Shared calibration inputs: one representative rotation channel list per
/// AGR target (aggregated over valid clips) and the admissible non-mesh
/// candidate set (skin-derived for skinned models). Used by the live
/// calibration and by the diagnostics-only audit so both measure the same
/// semantics.
fn calibration_inputs(
    model: &ModelAsset,
    library: &crate::inspector::animation::clip::AnimationLibrary,
) -> (
    Vec<(String, Vec<crate::inspector::animation::clip::TrackChannel>)>,
    Vec<(String, NodeId)>,
) {
    use crate::inspector::animation::clip::TrackChannel;
    use std::collections::{BTreeMap, HashSet};

    // A representative rotation channel per target, aggregated over valid
    // clips. BTreeMap keeps both target order and the resulting calibration
    // deterministic when a library was assembled from a hash-based source.
    let mut representative: BTreeMap<String, Vec<TrackChannel>> = BTreeMap::new();
    for clip in &library.clips {
        if clip.validate().is_err() {
            continue;
        }
        for track in &clip.tracks {
            if !matches!(track.channel, TrackChannel::Rotation { .. }) {
                continue;
            }
            representative
                .entry(track.target.clone())
                .or_default()
                .push(track.channel.clone());
        }
    }

    let mut targets: Vec<(String, Vec<TrackChannel>)> = representative.into_iter().collect();
    // Most Bully files use zero-padded track names, but numeric ordering is
    // the format invariant and also keeps non-padded diagnostic fixtures sane.
    targets.sort_by(|(left, _), (right, _)| {
        track_number(left)
            .zip(track_number(right))
            .map(|(left, right)| left.cmp(&right))
            .unwrap_or_else(|| left.cmp(right))
    });

    // A mesh node cannot be an AGR bone target. For a skinned asset, derive
    // the candidate set from skin joints and their ancestors, then retain
    // non-mesh attachment nodes below that skeleton. Bully's player rig has
    // one such post-skeleton TranslationNode (imported as ARROW); excluding
    // it would leave the final AGR curve unbound. Geometry/helper branches
    // below the synthetic Scene Root are not admitted. Scene Root itself is
    // never an animation target.
    let skinned_model = model.meshes.iter().any(|mesh| mesh.skin.is_some());
    let skeleton_candidates = if skinned_model {
        let mut ids = HashSet::new();
        for mesh in &model.meshes {
            let Some(skin) = &mesh.skin else {
                continue;
            };
            for &joint in &skin.joints {
                let mut current = Some(joint);
                while let Some(id) = current {
                    let Some(node) = model.node(id) else {
                        break;
                    };
                    if !node.name.eq_ignore_ascii_case("Scene Root") && node.mesh.is_none() {
                        ids.insert(id);
                    }
                    current = node.parent;
                }
            }
        }
        if !ids.is_empty() {
            // Skin palettes do not reference rigid attachment nodes, but AGR
            // can still animate them. Walk each non-mesh node's ancestry
            // against the original skin-derived set so a body wrapper under
            // Scene Root cannot become an accidental candidate merely because
            // it appears near the skeleton in the imported node list.
            let skeleton_ids = ids.clone();
            for node in &model.nodes {
                if node.mesh.is_some()
                    || node.name.eq_ignore_ascii_case("Scene Root")
                    || ids.contains(&node.id)
                {
                    continue;
                }
                let mut current = node.parent;
                let under_skeleton = loop {
                    let Some(id) = current else { break false };
                    if skeleton_ids.contains(&id) {
                        break true;
                    }
                    current = model.node(id).and_then(|parent| parent.parent);
                };
                if under_skeleton {
                    ids.insert(node.id);
                }
            }
        }
        (!ids.is_empty()).then_some(ids)
    } else {
        None
    };
    let candidates: Vec<(String, NodeId)> = model
        .nodes
        .iter()
        .filter(|node| node.mesh.is_none())
        .filter(|node| !node.name.eq_ignore_ascii_case("Scene Root"))
        .filter(|node| {
            skeleton_candidates
                .as_ref()
                .is_none_or(|ids| ids.contains(&node.id))
        })
        .map(|node| (node.name.clone(), node.id))
        .collect();

    (targets, candidates)
}

/// Diagnostics-only corpus aid: for each calibration target, the closest
/// and second-closest candidate rest matches. A "confident unique" match
/// (small `best`, clearly ahead of `second`) pins a target semantically,
/// so the audit can flag structurally valid bindings that contradict such
/// evidence without re-deriving the calibration's candidate logic.
#[cfg(test)]
pub(crate) fn target_rest_matches(
    model: &ModelAsset,
    library: &crate::inspector::animation::clip::AnimationLibrary,
) -> Vec<(String, Option<(NodeId, f32)>, Option<(NodeId, f32)>)> {
    let (targets, candidates) = calibration_inputs(model, library);
    targets
        .iter()
        .map(|(target, channels)| {
            let mut ranked: Vec<(f32, NodeId)> = candidates
                .iter()
                .filter_map(|(_, node)| {
                    let angle = rotation_min_angle(channels, &model.node(*node)?.local)?;
                    Some((angle, *node))
                })
                .collect();
            ranked.sort_by(|left, right| left.0.total_cmp(&right.0));
            let best = ranked.first().map(|(angle, node)| (*node, *angle));
            let second = ranked.get(1).map(|(angle, node)| (*node, *angle));
            (target.clone(), best, second)
        })
        .collect()
}

/// Build the cross-clip calibration: score each target against each node by
/// the closest key-to-rest angle across all clips, then solve one ordered,
/// one-to-one assignment. AGR's packed curve stream follows the source rig's
/// depth-first order, so preserving that order prevents a symmetric helper or
/// sibling from consuming a real limb and changing the inherited parent frame.
pub fn calibrate_bindings(
    model: &ModelAsset,
    library: &crate::inspector::animation::clip::AnimationLibrary,
) -> BindingCalibration {
    use std::collections::HashMap;

    const MAX_BIND_ANGLE_DEG: f32 = 20.0;
    const CANDIDATE_SKIP_COST: f32 = 0.25;
    const TARGET_SKIP_COST: f32 = 24.0;
    const MAX_ASSIGNMENT_CELLS: usize = 4_000_000;

    fn match_cost(
        raw_angle: f32,
        target: &str,
        node_name: &str,
        median_offset: Option<i64>,
    ) -> f32 {
        let penalty = match (median_offset, track_number(target), track_number(node_name)) {
            (Some(expected), Some(curve), Some(bone)) => {
                // Keep adversarial but parseable track names from overflowing
                // while calculating the ordering hint. Real files use small
                // non-negative indices, but diagnostics should remain total.
                let deviation =
                    (bone as f64 - curve as f64 - expected as f64).abs() as f32;
                1.5 * deviation
            }
            _ => 0.0,
        };
        raw_angle + penalty
    }

    let (targets, candidates) = calibration_inputs(model, library);
    if targets.is_empty() {
        return BindingCalibration::default();
    }
    if candidates.is_empty() {
        return BindingCalibration {
            assignments: HashMap::new(),
            diagnostics: vec![format!(
                "could not calibrate {} AGR targets: model has no non-mesh nodes",
                targets.len()
            )],
        };
    }

    let node_names: HashMap<crate::inspector::animation::NodeId, &str> = model
        .nodes
        .iter()
        .map(|node| (node.id, node.name.as_str()))
        .collect();

    // Confident numeric matches establish the source's curve/node offset.
    // Only the single best candidate votes per target; collecting every
    // close candidate made the old median depend on sibling pose symmetry.
    let mut offsets = Vec::new();
    for (target, channels) in &targets {
        let Some((_, _, node)) = candidates
            .iter()
            .enumerate()
            .filter_map(|(index, (_name, node))| {
                let angle = rotation_min_angle(channels, &model.node(*node)?.local)?;
                Some((angle, index, *node))
            })
            .filter(|(angle, _, _)| *angle < 5.0)
            .min_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1.cmp(&right.1))
            })
        else {
            continue;
        };
        if let (Some(curve), Some(bone)) = (
            track_number(target),
            node_names.get(&node).and_then(|name| track_number(name)),
        ) && let Some(offset) = bone.checked_sub(curve) {
            offsets.push(offset);
        }
    }
    let median_offset = if offsets.len() >= 8 {
        offsets.sort_unstable();
        Some(offsets[offsets.len() / 2])
    } else {
        None
    };

    // The source curve stream is ordered. A monotonic dynamic-programming
    // assignment preserves that invariant while allowing attachment nodes
    // (for example ARROW) and skipping unrelated model nodes. This is
    // deliberately global: a greedy local match can consume a sibling's
    // rest-like pose and leave the leg chain with the wrong parent frame.
    let rows = targets.len() + 1;
    let columns = candidates.len() + 1;
    let cell_count = rows.saturating_mul(columns);
    if cell_count > MAX_ASSIGNMENT_CELLS {
        return BindingCalibration {
            assignments: HashMap::new(),
            diagnostics: vec![format!(
                "could not calibrate {} AGR targets against {} model nodes: assignment matrix is too large",
                targets.len(),
                candidates.len()
            )],
        };
    }

    let mut costs = vec![f32::INFINITY; cell_count];
    let mut choices = vec![0_u8; cell_count];
    let cell = |row: usize, column: usize| row * columns + column;
    costs[cell(targets.len(), candidates.len())] = 0.0;
    for row in (0..targets.len()).rev() {
        let index = cell(row, candidates.len());
        costs[index] = TARGET_SKIP_COST + costs[cell(row + 1, candidates.len())];
        choices[index] = 1;
    }
    for column in (0..candidates.len()).rev() {
        let index = cell(targets.len(), column);
        costs[index] = CANDIDATE_SKIP_COST + costs[cell(targets.len(), column + 1)];
        choices[index] = 2;
    }

    for row in (0..targets.len()).rev() {
        for column in (0..candidates.len()).rev() {
            let (target, channels) = &targets[row];
            let (node_name, _) = &candidates[column];
            let mut best = TARGET_SKIP_COST + costs[cell(row + 1, column)];
            let mut choice = 1_u8;

            let skip_candidate = CANDIDATE_SKIP_COST + costs[cell(row, column + 1)];
            if skip_candidate < best {
                best = skip_candidate;
                choice = 2;
            }

            if let Some(node) = model.node(candidates[column].1)
                && let Some(raw_angle) = rotation_min_angle(channels, &node.local)
                && raw_angle <= MAX_BIND_ANGLE_DEG
            {
                let match_value =
                    match_cost(raw_angle, target, node_name, median_offset)
                        + costs[cell(row + 1, column + 1)];
                // Prefer a valid match on exact ties. This avoids dropping
                // the first curve when all channels are at identity in a
                // sparse test clip.
                if match_value <= best {
                    best = match_value;
                    choice = 0;
                }
            }
            costs[cell(row, column)] = best;
            choices[cell(row, column)] = choice;
        }
    }

    let mut assignments = HashMap::new();
    let mut row = 0;
    let mut column = 0;
    while row < targets.len() && column < candidates.len() {
        match choices[cell(row, column)] {
            0 => {
                assignments.insert(targets[row].0.clone(), candidates[column].1);
                row += 1;
                column += 1;
            }
            1 => {
                row += 1;
            }
            2 => column += 1,
            _ => break,
        }
    }

    // A compact Bully action library may never visit the bind pose for its
    // root/torso curves, and on wrapper-heavy rigs the pose matcher can bind
    // sibling bones by chance while rejecting the actual root/limb run. The
    // verified importer contract (semantic `Dummy` placeholder, stream
    // starting at its first child, every covered node a unique skin
    // candidate) is the export order whenever its signature holds, so it
    // supersedes the pose result; a disagreement without that signature
    // leaves the generic calibration intact.
    if let Some(numeric) = bully_numeric_track_offset(model, library, &targets, &candidates) {
        assignments = numeric;
    }

    let diagnostics = targets
        .iter()
        .filter(|(target, _)| !assignments.contains_key(target))
        .map(|(target, _)| {
            format!(
                "AGR target '{}' was not admitted during ordered calibration",
                target
            )
        })
        .collect();

    BindingCalibration {
        assignments,
        diagnostics,
    }
}

/// Bind a clip through a session calibration; targets the calibration left
/// unassigned stay unbound with a diagnostic rather than guessing.
pub fn bind_clip_with_calibration(
    model: &ModelAsset,
    clip: &AnimationClip,
    calibration: &BindingCalibration,
) -> ClipBinding {
    let mut tracks = Vec::with_capacity(clip.tracks.len());
    let mut diagnostics = calibration.diagnostics.clone();
    for track in &clip.tracks {
        let binding = match calibration.assignments.get(&track.target) {
            Some(&node) => TrackBinding {
                node: Some(node),
                status: TrackBindingStatus::Bound,
            },
            None => {
                diagnostics.push(format!(
                    "track target '{}' has no calibrated node in model '{}'",
                    track.target, model.name
                ));
                TrackBinding {
                    node: None,
                    status: TrackBindingStatus::Missing,
                }
            }
        };
        tracks.push(binding);
    }
    ClipBinding {
        clip: clip.id,
        tracks,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspector::animation::clip::{Interpolation, PropertyTrack, TrackChannel};
    use crate::inspector::animation::fixtures;
    use crate::inspector::animation::model::{
        MeshAsset, NodeTransform, SceneNode, SkinBinding, VertexSkin,
    };
    use crate::inspector::scene3d::camera::BaseOrientation;
    use crate::inspector::scene3d::mesh::Vertex;
    use glam::{Mat4, Quat, Vec3};

    fn track(target: &str) -> PropertyTrack {
        PropertyTrack {
            target: target.into(),
            channel: TrackChannel::Translation {
                times: vec![0.0, 1.0],
                values: vec![Vec3::ZERO, Vec3::X],
            },
            interpolation: Interpolation::Linear,
        }
    }

    fn rotation_track(target: &str, rotation: Quat) -> PropertyTrack {
        PropertyTrack {
            target: target.into(),
            channel: TrackChannel::Rotation {
                times: vec![0.0, 1.0],
                values: vec![rotation, rotation],
            },
            interpolation: Interpolation::Linear,
        }
    }

    #[test]
    fn exact_identity_binds() {
        let (model, library) = fixtures::two_joint_strip();
        let clip = library.find_by_name("bend").unwrap();
        let binding = bind_clip(&model, clip);
        assert!(binding.is_fully_bound());
        assert!(binding.diagnostics.is_empty());
    }

    #[test]
    fn missing_identity_is_reported_not_guessed() {
        let (model, _) = fixtures::two_joint_strip();
        let clip = AnimationClip {
            id: ClipId(3),
            name: "foreign".into(),
            duration: 1.0,
            tracks: vec![track("Pelvis_L")],
            source_rate: None,
            markers: Vec::new(),
            provenance: "test".into(),
        };
        let binding = bind_clip(&model, &clip);
        assert_eq!(binding.bound_count(), 0);
        assert_eq!(binding.tracks[0].status, TrackBindingStatus::Missing);
        assert_eq!(binding.diagnostics.len(), 1);
    }

    #[test]
    fn shared_names_are_ambiguous() {
        let nodes = vec![
            SceneNode {
                id: NodeId(0),
                parent: None,
                name: "root".into(),
                local: NodeTransform::IDENTITY,
                mesh: None,
            },
            SceneNode {
                id: NodeId(1),
                parent: Some(NodeId(0)),
                name: "arm".into(),
                local: NodeTransform::IDENTITY,
                mesh: None,
            },
            SceneNode {
                id: NodeId(2),
                parent: Some(NodeId(0)),
                name: "arm".into(),
                local: NodeTransform::IDENTITY,
                mesh: None,
            },
        ];
        let model = ModelAsset::new(
            "dup".into(),
            "test".into(),
            nodes,
            Vec::<MeshAsset>::new(),
            Mat4::IDENTITY,
            BaseOrientation::Yup,
            None,
        )
        .unwrap();
        let clip = AnimationClip {
            id: ClipId(4),
            name: "shared".into(),
            duration: 1.0,
            tracks: vec![track("arm")],
            source_rate: None,
            markers: Vec::new(),
            provenance: "test".into(),
        };
        let binding = bind_clip(&model, &clip);
        assert!(matches!(
            binding.tracks[0].status,
            TrackBindingStatus::Ambiguous(ref nodes) if nodes.len() == 2
        ));
        assert!(binding.tracks[0].node.is_none());
    }

    #[test]
    fn ordered_calibration_keeps_skeleton_and_attachment_order() {
        let nodes = vec![
            SceneNode {
                id: NodeId(0),
                parent: None,
                name: "Scene Root".into(),
                local: NodeTransform::IDENTITY,
                mesh: None,
            },
            SceneNode {
                id: NodeId(1),
                parent: Some(NodeId(0)),
                name: "track_000".into(),
                local: NodeTransform::IDENTITY,
                mesh: None,
            },
            SceneNode {
                id: NodeId(2),
                parent: Some(NodeId(1)),
                name: "track_001".into(),
                local: NodeTransform {
                    rotation: Quat::from_rotation_z(0.2),
                    ..NodeTransform::IDENTITY
                },
                mesh: None,
            },
            SceneNode {
                id: NodeId(3),
                parent: Some(NodeId(1)),
                name: "track_002".into(),
                local: NodeTransform {
                    rotation: Quat::from_rotation_z(0.4),
                    ..NodeTransform::IDENTITY
                },
                mesh: None,
            },
            SceneNode {
                id: NodeId(4),
                parent: Some(NodeId(0)),
                name: "shape_004".into(),
                local: NodeTransform::IDENTITY,
                mesh: Some(0),
            },
        ];
        let mut influence = VertexSkin::new();
        influence.push((0, 1.0));
        let model = ModelAsset::new(
            "ordered calibration".into(),
            "synthetic:ordered-calibration".into(),
            nodes,
            vec![MeshAsset {
                name: "body".into(),
                texture_name: None,
                vertices: vec![Vertex {
                    position: [0.0, 0.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                }],
                indices: Vec::new(),
                diffuse: None,
                skin: Some(SkinBinding {
                    joints: vec![NodeId(2)],
                    inverse_bind: vec![Mat4::IDENTITY],
                    weights: vec![influence],
                }),
            }],
            Mat4::IDENTITY,
            BaseOrientation::Yup,
            None,
        )
        .unwrap();
        let mut clip = AnimationClip {
            id: ClipId(5),
            name: "ordered".into(),
            duration: 1.0,
            tracks: vec![
                rotation_track("track_000", Quat::from_rotation_z(0.2)),
                rotation_track("track_001", Quat::from_rotation_z(0.4)),
            ],
            source_rate: None,
            markers: Vec::new(),
            provenance: "synthetic".into(),
        };
        clip.normalize_rotation_keys();
        clip.validate().unwrap();
        let library = crate::inspector::animation::clip::AnimationLibrary {
            name: "ordered".into(),
            clips: vec![clip.clone()],
            provenance: "synthetic".into(),
        };

        let first = bind_clip_with_calibration(
            &model,
            &clip,
            &calibrate_bindings(&model, &library),
        );
        let second = bind_clip_with_calibration(
            &model,
            &clip,
            &calibrate_bindings(&model, &library),
        );
        assert_eq!(first.node_for_track(0), Some(NodeId(2)));
        assert_eq!(first.node_for_track(1), Some(NodeId(3)));
        assert_eq!(first.node_for_track(0), second.node_for_track(0));
        assert_eq!(first.node_for_track(1), second.node_for_track(1));
    }

    #[test]
    fn bully_action_only_tracks_recover_the_imported_numeric_offset() {
        // Two tracks are deliberately held 90 degrees away from every
        // rest rotation. The strict pose matcher must skip them, while the
        // structural Bully adapter rule must recover their source order.
        let mut nodes = vec![SceneNode {
            id: NodeId(0),
            parent: None,
            name: "Scene Root".into(),
            local: NodeTransform::IDENTITY,
            mesh: None,
        }];
        for index in 1..=8 {
            nodes.push(SceneNode {
                id: NodeId(index + 1),
                parent: Some(NodeId(index)),
                name: format!("track_{index:03}"),
                local: NodeTransform {
                    rotation: Quat::from_rotation_z(index as f32 * 0.7853982),
                    ..NodeTransform::IDENTITY
                },
                mesh: None,
            });
        }
        nodes.insert(
            1,
            SceneNode {
                id: NodeId(1),
                parent: Some(NodeId(0)),
                name: "track_000".into(),
                local: NodeTransform::IDENTITY,
                mesh: None,
            },
        );
        let mut influence = VertexSkin::new();
        influence.push((0, 1.0));
        nodes.push(SceneNode {
            id: NodeId(10),
            parent: Some(NodeId(2)),
            name: "shape_010".into(),
            local: NodeTransform::IDENTITY,
            mesh: Some(0),
        });
        let mut source_names: Vec<Option<String>> =
            nodes.iter().map(|node| Some(node.name.clone())).collect();
        // The ordinary contract is anchored to the placeholder's original
        // `Dummy` identity, which the NIF adapter preserves in source names.
        source_names[1] = Some("Dummy".into());
        let mut model = ModelAsset::new(
            "Bully action-only fixture".into(),
            "fixture:player.nif".into(),
            nodes,
            vec![MeshAsset {
                name: "body".into(),
                texture_name: None,
                vertices: vec![Vertex {
                    position: [0.0, 0.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                }],
                indices: Vec::new(),
                diffuse: None,
                skin: Some(SkinBinding {
                    joints: vec![NodeId(2)],
                    inverse_bind: vec![Mat4::IDENTITY],
                    weights: vec![influence],
                }),
            }],
            Mat4::IDENTITY,
            BaseOrientation::Zup,
            Some(NodeId(1)),
        )
        .unwrap();
        model.set_source_names(source_names);

        let mut tracks = Vec::new();
        for index in 0..8 {
            let rest = Quat::from_rotation_z((index + 1) as f32 * 0.7853982);
            let rotation = if matches!(index, 0 | 4) {
                rest * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
            } else {
                rest
            };
            tracks.push(rotation_track(&format!("track_{index:03}"), rotation));
        }
        let mut clip = AnimationClip {
            id: ClipId(6),
            name: "action-only".into(),
            duration: 1.0,
            tracks,
            source_rate: None,
            markers: Vec::new(),
            provenance: "Bully AGR variant 1002".into(),
        };
        clip.normalize_rotation_keys();
        clip.validate().unwrap();
        let library = crate::inspector::animation::clip::AnimationLibrary {
            name: "Hang_Workout.agr".into(),
            clips: vec![clip.clone()],
            provenance: "Bully AGR (PC, experimental reader)".into(),
        };

        let binding = bind_clip_with_calibration(
            &model,
            &clip,
            &calibrate_bindings(&model, &library),
        );
        assert_eq!(binding.bound_count(), 8);
        assert!(binding.diagnostics.is_empty());
        for index in 0..8 {
            assert_eq!(
                binding.node_for_track(index),
                Some(NodeId(index as u32 + 2)),
                "curve {index} must preserve the one-node importer offset"
            );
        }
    }

    #[test]
    fn wrapper_rig_stream_skips_the_placeholder() {
        // JKGirl_Mandy shape: sibling wrapper branches precede the semantic
        // `Dummy` root in the imported order. The stream still starts at the
        // root's first child — the placeholder is never an animation target —
        // so curve i binds to track_(i + 3) instead of the player's +1. The
        // strict pose matcher cannot admit the action-only curves, so the
        // verified run must replace its partial guess.
        let mut nodes = vec![
            SceneNode {
                id: NodeId(0),
                parent: None,
                name: "Scene Root".into(),
                local: NodeTransform::IDENTITY,
                mesh: None,
            },
            SceneNode {
                id: NodeId(1),
                parent: Some(NodeId(0)),
                name: "track_000".into(),
                local: NodeTransform::IDENTITY,
                mesh: None,
            },
            SceneNode {
                id: NodeId(2),
                parent: Some(NodeId(1)),
                name: "track_001".into(),
                local: NodeTransform::IDENTITY,
                mesh: None,
            },
            SceneNode {
                id: NodeId(3),
                parent: Some(NodeId(2)),
                name: "shape_003".into(),
                local: NodeTransform::IDENTITY,
                mesh: Some(0),
            },
            SceneNode {
                id: NodeId(4),
                parent: Some(NodeId(0)),
                name: "track_002".into(),
                local: NodeTransform::IDENTITY,
                mesh: None,
            },
        ];
        for index in 3..=10 {
            nodes.push(SceneNode {
                id: NodeId(index + 2),
                parent: Some(NodeId(if index == 3 { 4 } else { index + 1 })),
                name: format!("track_{index:03}"),
                local: NodeTransform {
                    rotation: Quat::from_rotation_z(index as f32 * 0.5),
                    ..NodeTransform::IDENTITY
                },
                mesh: None,
            });
        }
        let mut influence = VertexSkin::new();
        influence.push((0, 1.0));
        let mut source_names: Vec<Option<String>> =
            nodes.iter().map(|node| Some(node.name.clone())).collect();
        source_names[1] = Some("JKGirl_Mandy".into());
        source_names[2] = Some("__NDL_MultiMtl_Node".into());
        source_names[4] = Some("Dummy".into());
        let mut model = ModelAsset::new(
            "Bully wrapper fixture".into(),
            "fixture:mandy.nif".into(),
            nodes,
            vec![MeshAsset {
                name: "body".into(),
                texture_name: None,
                vertices: vec![Vertex {
                    position: [0.0, 0.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                }],
                indices: Vec::new(),
                diffuse: None,
                skin: Some(SkinBinding {
                    joints: vec![NodeId(5)],
                    inverse_bind: vec![Mat4::IDENTITY],
                    weights: vec![influence],
                }),
            }],
            Mat4::IDENTITY,
            BaseOrientation::Zup,
            Some(NodeId(4)),
        )
        .unwrap();
        model.set_source_names(source_names);

        let mut tracks = Vec::new();
        for index in 0..8 {
            let rest = Quat::from_rotation_z((index + 3) as f32 * 0.5);
            let rotation = if matches!(index, 2 | 5) {
                rest * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
            } else {
                rest
            };
            tracks.push(rotation_track(&format!("track_{index:03}"), rotation));
        }
        let mut clip = AnimationClip {
            id: ClipId(7),
            name: "wrapper".into(),
            duration: 1.0,
            tracks,
            source_rate: None,
            markers: Vec::new(),
            provenance: "Bully AGR variant 1001".into(),
        };
        clip.normalize_rotation_keys();
        clip.validate().unwrap();
        let library = crate::inspector::animation::clip::AnimationLibrary {
            name: "1_08_MandPuke.agr".into(),
            clips: vec![clip.clone()],
            provenance: "Bully AGR (PC, experimental reader)".into(),
        };

        let binding = bind_clip_with_calibration(
            &model,
            &clip,
            &calibrate_bindings(&model, &library),
        );
        assert_eq!(binding.bound_count(), 8);
        assert!(binding.diagnostics.is_empty());
        for index in 0..8 {
            assert_eq!(
                binding.node_for_track(index),
                Some(NodeId(index as u32 + 5)),
                "wrapper curve {index} must bind to track_{:03}",
                index + 3
            );
        }
    }

    #[test]
    fn numeric_recovery_requires_the_verified_dummy_identity() {
        // Same shape as the action-only fixture, but the placeholder keeps
        // its generated name: neither contract may fire, so the strict pose
        // calibration governs and the action-only curves stay honestly
        // unbound instead of binding to a neighbor.
        let mut nodes = vec![SceneNode {
            id: NodeId(0),
            parent: None,
            name: "Scene Root".into(),
            local: NodeTransform::IDENTITY,
            mesh: None,
        }];
        for index in 0..=8 {
            nodes.push(SceneNode {
                id: NodeId(index + 1),
                parent: Some(NodeId(index)),
                name: format!("track_{index:03}"),
                local: NodeTransform {
                    rotation: Quat::from_rotation_z((index + 1) as f32 * 0.7853982),
                    ..NodeTransform::IDENTITY
                },
                mesh: None,
            });
        }
        let mut influence = VertexSkin::new();
        influence.push((0, 1.0));
        nodes.push(SceneNode {
            id: NodeId(10),
            parent: Some(NodeId(2)),
            name: "shape_010".into(),
            local: NodeTransform::IDENTITY,
            mesh: Some(0),
        });
        let mut model = ModelAsset::new(
            "anonymous placeholder fixture".into(),
            "fixture:anonymous.nif".into(),
            nodes,
            vec![MeshAsset {
                name: "body".into(),
                texture_name: None,
                vertices: vec![Vertex {
                    position: [0.0, 0.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                }],
                indices: Vec::new(),
                diffuse: None,
                skin: Some(SkinBinding {
                    joints: vec![NodeId(2)],
                    inverse_bind: vec![Mat4::IDENTITY],
                    weights: vec![influence],
                }),
            }],
            Mat4::IDENTITY,
            BaseOrientation::Zup,
            Some(NodeId(1)),
        )
        .unwrap();
        let source_names: Vec<Option<String>> = model
            .nodes
            .iter()
            .map(|node| Some(node.name.clone()))
            .collect();
        model.set_source_names(source_names);

        let mut tracks = Vec::new();
        for index in 0..8 {
            let rest = Quat::from_rotation_z((index + 1) as f32 * 0.7853982);
            let rotation = if matches!(index, 0 | 4) {
                rest * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
            } else {
                rest
            };
            tracks.push(rotation_track(&format!("track_{index:03}"), rotation));
        }
        let mut clip = AnimationClip {
            id: ClipId(8),
            name: "anonymous".into(),
            duration: 1.0,
            tracks,
            source_rate: None,
            markers: Vec::new(),
            provenance: "Bully AGR variant 1002".into(),
        };
        clip.normalize_rotation_keys();
        clip.validate().unwrap();
        let library = crate::inspector::animation::clip::AnimationLibrary {
            name: "Anonymous.agr".into(),
            clips: vec![clip.clone()],
            provenance: "Bully AGR (PC, experimental reader)".into(),
        };

        let binding = bind_clip_with_calibration(
            &model,
            &clip,
            &calibrate_bindings(&model, &library),
        );
        assert_eq!(
            binding.bound_count(),
            6,
            "only rest-matching curves may bind without the verified identity"
        );
        assert!(binding.node_for_track(0).is_none());
        assert!(binding.node_for_track(4).is_none());
        assert!(!binding.diagnostics.is_empty());
    }
}
