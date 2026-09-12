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
        assert_eq!(
            binding.tracks[0].status,
            TrackBindingStatus::Missing
        );
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
