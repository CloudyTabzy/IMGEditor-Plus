//! Runtime animation session owned by the 3D viewer handle.
//!
//! The session joins the format-independent core
//! ([`crate::inspector::animation`]) to the live viewer: it owns the
//! immutable asset + clip library, the clip binding, the transport clock,
//! and the reusable pose scratch buffers. Message handling advances the
//! transport and re-evaluates the pose; the renderer only uploads an
//! already-evaluated revision. Drawing never advances time.

use std::sync::Arc;
use std::time::Instant;

use glam::Vec3;

use crate::inspector::animation::binding::{ClipBinding, bind_clip};
use crate::inspector::animation::clip::{AnimationClip, AnimationLibrary, ClipMarker};
use crate::inspector::animation::model::ModelAsset;
use crate::inspector::animation::pose::{
    PoseBuffers, RootMotionPolicy, evaluate_pose, sample_locals,
};
use crate::inspector::animation::transport::{LoopMode, PlaybackState, Transport};
use crate::inspector::animation::{ClipId, PlaybackCapability};

/// User-tunable presentation state for the animation dock.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationPanel {
    pub dock_visible: bool,
    pub follow_root: bool,
    pub show_skeleton: bool,
    pub show_motion_path: bool,
}

impl Default for AnimationPanel {
    fn default() -> Self {
        Self {
            dock_visible: true,
            follow_root: false,
            show_skeleton: false,
            show_motion_path: false,
        }
    }
}

/// One advance result, shaped for the app's message handlers.
#[derive(Clone, Debug, Default)]
pub struct SessionAdvance {
    pub time_changed: bool,
    pub ended: bool,
    pub paused_by_gap: bool,
    pub markers: Vec<ClipMarker>,
}

/// A live animation session: asset + clip + clock + pose.
#[derive(Clone, Debug)]
pub struct AnimationSession {
    pub asset: Arc<ModelAsset>,
    pub library: Arc<AnimationLibrary>,
    pub clip: Option<ClipId>,
    pub binding: Option<ClipBinding>,
    pub transport: Transport,
    pub pose: PoseBuffers,
    pub capability: PlaybackCapability,
    pub root_policy: RootMotionPolicy,
    pub display_offset: Vec3,
    pub uploaded_revision: u64,
    pub panel: AnimationPanel,
    pub last_marker: Option<(String, Instant)>,
    pub demo: bool,
}

impl AnimationSession {
    pub fn new(
        asset: Arc<ModelAsset>,
        library: Arc<AnimationLibrary>,
        display_offset: Vec3,
        demo: bool,
        now: Instant,
    ) -> Self {
        let pose = PoseBuffers::new(&asset);
        let capability = capability_for(&asset, &library, None, None);
        let mut session = Self {
            asset,
            library,
            clip: None,
            binding: None,
            transport: Transport::default(),
            pose,
            capability,
            root_policy: RootMotionPolicy::Source,
            display_offset,
            uploaded_revision: u64::MAX,
            panel: AnimationPanel::default(),
            last_marker: None,
            demo,
        };
        if let Some(first) = session.library.clips.first().map(|clip| clip.id) {
            session.select_clip(first, now);
        } else {
            session.evaluate();
        }
        session
    }

    pub fn clip(&self) -> Option<&AnimationClip> {
        self.clip.and_then(|id| self.library.clip(id))
    }

    pub fn clip_name(&self) -> Option<&str> {
        self.clip().map(|clip| clip.name.as_str())
    }

    pub fn markers(&self) -> &[ClipMarker] {
        self.clip()
            .map(|clip| clip.markers.as_slice())
            .unwrap_or(&[])
    }

    pub fn is_playing(&self) -> bool {
        self.transport.is_playing()
    }

    /// `true` while the transport holds a clip that can advance.
    pub fn can_play(&self) -> bool {
        self.transport.can_play()
    }

    pub fn state(&self) -> &PlaybackState {
        self.transport.state()
    }

    /// Select a clip: bind its tracks, reset the transport to a paused
    /// start, and evaluate the opening pose.
    pub fn select_clip(&mut self, id: ClipId, now: Instant) -> bool {
        let Some(clip) = self.library.clip(id) else {
            return false;
        };
        self.clip = Some(id);
        let binding = bind_clip(&self.asset, clip);
        self.capability = capability_for(&self.asset, &self.library, Some(clip), Some(&binding));
        self.binding = Some(binding);
        self.transport.set_clip(clip, now);
        self.evaluate();
        self.can_play()
    }

    /// Sample the current time and deform the geometry into the pose
    /// buffers. Called by every transport transition and every advance.
    pub fn evaluate(&mut self) {
        match (self.clip, self.binding.as_ref()) {
            (Some(id), Some(binding)) => {
                if let Some(clip) = self.library.clip(id) {
                    sample_locals(
                        clip,
                        binding,
                        &self.asset,
                        self.transport.shown_time() as f32,
                        &mut self.pose.locals,
                    );
                }
            }
            _ => {
                for (index, node) in self.asset.nodes.iter().enumerate() {
                    self.pose.locals[index] = node.local;
                }
            }
        }
        evaluate_pose(
            &self.asset,
            self.root_policy,
            self.display_offset,
            &mut self.pose,
        );
    }

    /// Advance the transport once per host redraw and return the crossed
    /// marker events. Only an actual time change re-evaluates the pose.
    pub fn advance(&mut self, now: Instant) -> SessionAdvance {
        let before = self.transport.shown_time();
        let advance = self.transport.advance(now);
        let markers = if let Some(clip) = self.clip() {
            Transport::crossed_markers(
                &clip.markers,
                before,
                advance.time,
                advance.wraps,
                self.transport.range(),
            )
        } else {
            Vec::new()
        };
        let time_changed = (advance.time - before).abs() > 1e-12;
        if time_changed || advance.paused_by_gap {
            self.evaluate();
        }
        SessionAdvance {
            time_changed,
            ended: advance.ended,
            paused_by_gap: advance.paused_by_gap,
            markers,
        }
    }

    pub fn play(&mut self, now: Instant) {
        self.transport.play(now);
    }

    pub fn pause(&mut self, now: Instant) {
        self.transport.pause(now);
    }

    pub fn toggle_play_pause(&mut self, now: Instant) {
        self.transport.toggle_play_pause(now);
    }

    pub fn stop(&mut self, now: Instant) {
        self.transport.stop(now);
        self.evaluate();
    }

    pub fn seek(&mut self, now: Instant, time: f64) {
        self.transport.seek(now, time);
        self.evaluate();
    }

    pub fn step(&mut self, now: Instant, direction: i32) {
        self.transport.step(now, direction);
        self.evaluate();
    }

    pub fn begin_scrub(&mut self, now: Instant) {
        self.transport.begin_scrub(now);
    }

    pub fn scrub_to(&mut self, time: f64) {
        self.transport.scrub_to(time);
        self.evaluate();
    }

    pub fn end_scrub(&mut self, now: Instant) {
        self.transport.end_scrub(now);
    }

    pub fn suspend(&mut self, now: Instant) {
        self.transport.suspend(now);
    }

    pub fn resume(&mut self, now: Instant) {
        self.transport.resume(now);
    }

    pub fn set_speed(&mut self, now: Instant, speed: f64) {
        self.transport.set_speed(now, speed);
    }

    pub fn set_range(&mut self, now: Instant, start: f64, end: f64) {
        self.transport.set_range(now, start, end);
        self.evaluate();
    }

    pub fn set_loop_mode(&mut self, now: Instant, mode: LoopMode) {
        self.transport.set_loop_mode(now, mode);
    }

    pub fn set_root_policy(&mut self, policy: RootMotionPolicy) {
        if self.root_policy == policy {
            return;
        }
        self.root_policy = policy;
        self.evaluate();
    }

    pub fn set_display_offset(&mut self, offset: Vec3) {
        if self.display_offset == offset {
            return;
        }
        self.display_offset = offset;
        self.evaluate();
    }

    pub fn note_marker(&mut self, marker: &ClipMarker, now: Instant) {
        self.last_marker = Some((marker.label.clone(), now));
    }

    pub fn mark_uploaded(&mut self) {
        self.uploaded_revision = self.pose.revision;
    }
}

fn capability_for(
    asset: &ModelAsset,
    library: &AnimationLibrary,
    clip: Option<&AnimationClip>,
    binding: Option<&ClipBinding>,
) -> PlaybackCapability {
    if clip.is_none() || library.clips.is_empty() {
        return PlaybackCapability::NoAnimation;
    }
    let Some(binding) = binding else {
        return PlaybackCapability::UnboundClips;
    };
    let total = binding.total_count();
    if total == 0 {
        return PlaybackCapability::MetadataOnly;
    }
    let bound = binding.bound_count();
    if bound == 0 {
        return PlaybackCapability::UnboundClips;
    }
    if bound < total {
        return PlaybackCapability::Partial { bound, total };
    }
    let skinned = skinned_nodes(asset);
    let drives_skin = binding
        .tracks
        .iter()
        .any(|track| track.node.is_some_and(|node| skinned.contains(&node.0)));
    if drives_skin {
        PlaybackCapability::ReadySkinned
    } else {
        PlaybackCapability::ReadyRigid
    }
}

fn skinned_nodes(asset: &ModelAsset) -> Vec<u32> {
    let mut nodes = Vec::new();
    for mesh in &asset.meshes {
        if let Some(skin) = &mesh.skin {
            for joint in &skin.joints {
                nodes.push(joint.0);
            }
        }
    }
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspector::animation::fixtures;
    use std::time::Duration;

    fn demo_session() -> AnimationSession {
        let (model, library) = fixtures::demo();
        AnimationSession::new(
            Arc::new(model),
            Arc::new(library),
            Vec3::ZERO,
            true,
            Instant::now(),
        )
    }

    #[test]
    fn new_session_selects_first_clip_paused() {
        let session = demo_session();
        assert_eq!(session.clip_name(), Some("Idle"));
        assert!(matches!(session.state(), PlaybackState::Paused));
        assert!(session.can_play());
        assert!(session.capability.is_playable());
    }

    #[test]
    fn advance_recomputes_the_pose_while_playing() {
        let mut session = demo_session();
        let start = Instant::now();
        session.play(start);
        let before = session.pose.revision;
        session.advance(start + Duration::from_millis(500));
        assert!(
            session.pose.revision > before,
            "advancing a playing transport must re-evaluate the pose"
        );
    }

    #[test]
    fn unknown_clip_selection_is_rejected() {
        let mut session = demo_session();
        assert!(!session.select_clip(ClipId(999), Instant::now()));
    }

    #[test]
    fn switching_clips_rebinds_and_restarts_paused() {
        let mut session = demo_session();
        let wave = session.library.find_by_name("Wave").unwrap().id;
        assert!(session.select_clip(wave, Instant::now()));
        assert_eq!(session.clip_name(), Some("Wave"));
        assert_eq!(session.transport.shown_time(), 0.0);
        assert!(matches!(session.state(), PlaybackState::Paused));
    }
}
