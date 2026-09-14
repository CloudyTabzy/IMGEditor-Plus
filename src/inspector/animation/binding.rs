//! Clip-to-model binding: resolve each track's source target identity to
//! a runtime node, keeping ambiguous, unmatched and incompatible targets
//! visible. A partial preview is labelled; unbound channels retain
//! default pose values. Clips are never silently retargeted between
//! different skeletons.

use crate::inspector::animation::clip::AnimationClip;
use crate::inspector::animation::model::ModelAsset;
use crate::inspector::animation::{ClipId, NodeId};

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

/// Build the cross-clip calibration: score each target against each node by
/// the closest key-to-rest angle across all clips, then assign greedily from
/// the best score down (one node serves one target).
pub fn calibrate_bindings(
    model: &ModelAsset,
    library: &crate::inspector::animation::clip::AnimationLibrary,
) -> BindingCalibration {
    use crate::inspector::animation::clip::TrackChannel;

    const MAX_BIND_ANGLE_DEG: f32 = 20.0;

    fn channel_min_angle(
        channel: &TrackChannel,
        rest: &crate::inspector::animation::model::NodeTransform,
    ) -> Option<f32> {
        match channel {
            TrackChannel::Rotation { values, .. } => values
                .iter()
                .map(|key| {
                    let dot = key.x * rest.rotation.x
                        + key.y * rest.rotation.y
                        + key.z * rest.rotation.z
                        + key.w * rest.rotation.w;
                    dot.abs().clamp(-1.0, 1.0).acos().to_degrees() * 2.0
                })
                .reduce(f32::min),
            TrackChannel::Translation { values, .. } => values
                .iter()
                .map(|key| (key - rest.translation).length().to_degrees())
                .reduce(f32::min),
            TrackChannel::Scale { .. } => None,
        }
    }

    // Representative rotation channel per target, aggregated over clips.
    let mut representative: std::collections::HashMap<String, Vec<TrackChannel>> =
        std::collections::HashMap::new();
    for clip in &library.clips {
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for track in &clip.tracks {
            if !seen.insert(track.target.as_str()) {
                continue;
            }
            let entry = representative.entry(track.target.clone()).or_default();
            if matches!(track.channel, TrackChannel::Rotation { .. })
                || entry.is_empty()
            {
                entry.push(track.channel.clone());
            }
        }
    }

    let mut candidates: Vec<(f32, String, crate::inspector::animation::NodeId)> = Vec::new();
    for (target, channels) in &representative {
        for node in &model.nodes {
            // Shapes follow their owning bone rigidly; they are never
            // animation targets in the bone naming convention.
            if node.name.starts_with("shape_") {
                continue;
            }
            let best = channels
                .iter()
                .filter_map(|channel| channel_min_angle(channel, &node.local))
                .reduce(f32::min);
            if let Some(angle) = best {
                candidates.push((angle, target.clone(), node.id));
            }
        }
    }

    // Order prior: confident matches vote on the export's track offset
    // (curve index vs node track number); the export orders bones almost
    // monotonically, so off-offset candidates are usually a pose-lucky
    // curve stealing another bone's node.
    let track_number = |name: &str| {
        name.strip_prefix("track_")
            .and_then(|rest| rest.parse::<i64>().ok())
    };
    let node_names: std::collections::HashMap<crate::inspector::animation::NodeId, &str> = model
        .nodes
        .iter()
        .map(|node| (node.id, node.name.as_str()))
        .collect();
    let mut offsets: Vec<i64> = candidates
        .iter()
        .filter(|(angle, _, _)| *angle < 5.0)
        .filter_map(|(_, target, node)| {
            match (track_number(target), node_names.get(node).and_then(|name| track_number(name)))
            {
                (Some(curve), Some(bone)) => Some(bone - curve),
                _ => None,
            }
        })
        .collect();
    let median_offset = if offsets.len() >= 8 {
        offsets.sort_unstable();
        Some(offsets[offsets.len() / 2])
    } else {
        None
    };

    // Score: raw angle plus an order penalty; keep the raw angle for the
    // admission threshold so the prior only breaks ties, never admits.
    let mut scored: Vec<(f32, f32, String, crate::inspector::animation::NodeId)> = candidates
        .into_iter()
        .map(|(angle, target, node)| {
            let penalty = match (
                median_offset,
                track_number(&target),
                node_names.get(&node).and_then(|name| track_number(name)),
            ) {
                (Some(expected), Some(curve), Some(bone)) => {
                    1.5 * (bone - curve - expected).abs() as f32
                }
                _ => 0.0,
            };
            (angle + penalty, angle, target, node)
        })
        .collect();
    scored.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut used_nodes: std::collections::HashSet<crate::inspector::animation::NodeId> =
        std::collections::HashSet::new();
    let mut calibration = BindingCalibration::default();
    for (_, raw_angle, target, node) in scored {
        if raw_angle > MAX_BIND_ANGLE_DEG {
            break;
        }
        if calibration.assignments.contains_key(&target) || used_nodes.contains(&node) {
            continue;
        }
        calibration.assignments.insert(target, node);
        used_nodes.insert(node);
    }
    calibration
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
    use crate::inspector::animation::model::{MeshAsset, NodeTransform, SceneNode};
    use crate::inspector::scene3d::camera::BaseOrientation;
    use glam::{Mat4, Vec3};

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
}
