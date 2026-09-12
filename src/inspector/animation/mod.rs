//! Format-independent animation player core (infrastructure plan AV0–AV7).
//!
//! This module owns the typed, immutable assets and the deterministic pose
//! evaluator shared by future Bully AGR/CAT and GTA DFF/IFP adapters:
//!
//! - [`clip`] — `AnimationClip` property tracks, validation, sampling.
//! - [`model`] — `ModelAsset` node hierarchy, mesh attachments, skins.
//! - [`binding`] — clip-track → scene-node resolution with diagnostics.
//! - [`pose`] — deterministic pose evaluation, hierarchy composition and
//!   the CPU reference skinning path.
//! - [`transport`] — play/pause/seek/step/loop transport semantics with an
//!   injectable clock.
//! - [`fixtures`] — synthetic models and clips with analytical expected
//!   results; the development demo content and the test corpus. No game
//!   data is embedded anywhere in this module.
//!
//! None of these modules depend on Iced or wgpu: the UI advances the
//! transport clock and hands a time to the evaluator, and the GPU layer
//! only consumes evaluated poses. Drawing never advances time.
//!
//! See `docs/animation-viewer-infrastructure-plan.md` for the design.

pub mod binding;
pub mod clip;
pub mod fixtures;
pub mod model;
pub mod pose;
pub mod transport;

#[allow(unused_imports)]
pub use binding::{ClipBinding, TrackBinding, TrackBindingStatus, bind_clip};
#[allow(unused_imports)]
pub use clip::{
    AnimationClip, AnimationLibrary, ClipMarker, Interpolation, PropertyTrack, SourceRate,
    TrackChannel,
};
#[allow(unused_imports)]
pub use model::{MeshAsset, ModelAsset, NodeTransform, SceneNode, SkinBinding, VertexSkin};
#[allow(unused_imports)]
pub use pose::{PoseBuffers, RootMotionPolicy};
#[allow(unused_imports)]
pub use transport::{LoopMode, PlaybackState, Transport};

/// Stable identity of one node inside a [`ModelAsset`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

/// Stable identity of one clip inside an [`AnimationLibrary`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClipId(pub u32);

/// What the active model + clip selection can do. The UI reads this to
/// decide which controls to enable and which diagnostic to show; parsing
/// a resource successfully is not the same as making it playable.
#[derive(Clone, Debug, PartialEq)]
pub enum PlaybackCapability {
    /// The model carries no animation data at all.
    NoAnimation,
    /// Animation metadata was decoded but no usable tracks exist.
    MetadataOnly,
    /// Clips exist but none of their tracks resolved to model nodes.
    UnboundClips,
    /// All bound tracks drive rigid node transforms only.
    ReadyRigid,
    /// At least one bound track drives a node used by a skin.
    ReadySkinned,
    /// Some tracks resolved; unbound channels keep default pose values
    /// and the preview is labelled partial.
    Partial { bound: usize, total: usize },
    /// The source semantics cannot be previewed; `reason` explains why.
    Unsupported { reason: String },
}

impl PlaybackCapability {
    /// `true` when a transport can meaningfully play (fully or partially).
    pub fn is_playable(&self) -> bool {
        matches!(
            self,
            PlaybackCapability::ReadyRigid
                | PlaybackCapability::ReadySkinned
                | PlaybackCapability::Partial { .. }
        )
    }

    /// Short user-facing label for the animation dock's rig badge.
    pub fn badge(&self) -> String {
        match self {
            PlaybackCapability::NoAnimation => "no animation data".to_string(),
            PlaybackCapability::MetadataOnly => "metadata only".to_string(),
            PlaybackCapability::UnboundClips => "no tracks bound".to_string(),
            PlaybackCapability::ReadyRigid => "rig: matched".to_string(),
            PlaybackCapability::ReadySkinned => "rig: matched (skinned)".to_string(),
            PlaybackCapability::Partial { bound, total } => {
                format!("partial preview: {bound}/{total} tracks bound")
            }
            PlaybackCapability::Unsupported { reason } => format!("unsupported: {reason}"),
        }
    }
}
