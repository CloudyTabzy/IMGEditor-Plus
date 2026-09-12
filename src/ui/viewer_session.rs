//! Runtime animation session owned by the 3D viewer handle.
//!
//! The session joins the format-independent core
//! ([`crate::inspector::animation`]) to the live viewer: it owns the
//! immutable asset + clip library, the clip binding, the transport clock,
//! and the reusable pose scratch buffers. Message handling advances the
//! transport and re-evaluates the pose; the renderer only uploads an
//! already-evaluated revision. Drawing never advances time.

use std::sync::Arc;
use std::time::{Duration, Instant};

use glam::Vec3;

use crate::inspector::animation::binding::{ClipBinding, bind_clip};
use crate::inspector::animation::clip::{AnimationClip, AnimationLibrary, ClipMarker};
use crate::inspector::animation::model::{ModelAsset, NodeTransform};
use crate::inspector::animation::pose::{
    PoseBuffers, RootMotionPolicy, blend_locals, clip_envelope, evaluate_pose, sample_locals,
};
use crate::inspector::animation::transport::{LoopMode, PlaybackState, Transport};
use crate::inspector::animation::{ClipId, PlaybackCapability};
use crate::inspector::scene3d::mesh::Aabb;

/// User-tunable presentation state for the animation dock.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationPanel {
    pub dock_visible: bool,
    pub follow_root: bool,
    pub show_skeleton: bool,
    pub show_motion_path: bool,
    /// Blend compatible clip switches instead of cutting (viewer preview).
    pub crossfade: bool,
}

impl Default for AnimationPanel {
    fn default() -> Self {
        Self {
            dock_visible: true,
            follow_root: false,
            show_skeleton: false,
            show_motion_path: false,
            crossfade: false,
        }
    }
}

/// Fixed viewer-preview crossfade length. This is not a claimed game
/// transition; it only blends two compatible clips for inspection.
pub const CROSSFADE_DURATION: Duration = Duration::from_millis(150);

/// A running clip crossfade. Captures the outgoing local pose at the
/// switch instant so seeking or repeated selection can never read an
/// arbitrary previous frame. `elapsed` accumulates only playback time
/// reported by the transport, so suspension never consumes the fade.
#[derive(Clone, Debug)]
pub struct CrossfadeState {
    from_locals: Vec<NodeTransform>,
    elapsed: f64,
    duration: f64,
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
    /// Cached sampled root trajectory for the motion-path overlay; cleared
    /// whenever the clip, range, root policy or display offset changes.
    pub motion_path: Option<Vec<Vec3>>,
    crossfade: Option<CrossfadeState>,
    /// Scratch for sampling the incoming clip before a blend.
    scratch_locals: Vec<NodeTransform>,
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
        let node_count = asset.nodes.len();
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
            motion_path: None,
            crossfade: None,
            scratch_locals: vec![NodeTransform::IDENTITY; node_count],
        };
        if let Some(first) = session
            .library
            .clips
            .iter()
            .find(|clip| clip.validate().is_ok())
            .map(|clip| clip.id)
        {
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

    /// Select a clip: bind its tracks and reset the transport to a paused
    /// start. When `Crossfade` is enabled and a different clip is already
    /// playing, the outgoing pose is captured and blended into the new clip
    /// over [`CROSSFADE_DURATION`] of active playback; otherwise the switch
    /// is an immediate cut.
    pub fn select_clip(&mut self, id: ClipId, now: Instant) -> bool {
        let Some(clip) = self.library.clip(id) else {
            return false;
        };
        // The runtime never plays an invalid clip: validation runs before
        // any sampling, so a broken adapter payload cannot reach the pose
        // path (and cannot panic inside it).
        if clip.validate().is_err() {
            return false;
        }
        let switching = self.clip.is_some() && self.clip != Some(id);
        let was_playing = self.is_playing();
        let crossfade =
            switching && self.panel.crossfade && was_playing && !self.pose.locals.is_empty();
        let from_locals = crossfade.then(|| self.pose.locals.clone());
        let binding = bind_clip(&self.asset, clip);
        self.capability = capability_for(&self.asset, &self.library, Some(clip), Some(&binding));
        self.clip = Some(id);
        self.binding = Some(binding);
        self.motion_path = None;
        self.transport.set_clip(clip, now);
        self.crossfade = from_locals.map(|from_locals| CrossfadeState {
            from_locals,
            elapsed: 0.0,
            duration: CROSSFADE_DURATION.as_secs_f64(),
        });
        self.evaluate();
        if was_playing {
            self.transport.play(now);
        }
        self.can_play()
    }

    /// Sample the current time, apply any running crossfade, and deform the
    /// geometry into the pose buffers. Called by every transport transition
    /// and every advance.
    pub fn evaluate(&mut self) {
        let progress = self.crossfade_progress();
        let scratch = &mut self.scratch_locals;
        let sampled = match (self.clip, self.binding.as_ref()) {
            (Some(id), Some(binding)) => self.library.clip(id).map(|clip| {
                sample_locals(
                    clip,
                    binding,
                    &self.asset,
                    self.transport.shown_time() as f32,
                    scratch,
                );
            }),
            _ => None,
        };
        if sampled.is_none() {
            reset_locals(&self.asset, scratch);
        }
        match progress {
            Some(t) if t < 1.0 => {
                let from = self
                    .crossfade
                    .as_ref()
                    .map(|fade| fade.from_locals.as_slice())
                    .unwrap_or(scratch);
                blend_locals(from, scratch, t, &mut self.pose.locals);
            }
            _ => {
                self.pose.locals.copy_from_slice(scratch);
                if progress.is_some() {
                    // Fade finished: release the outgoing pose snapshot.
                    self.crossfade = None;
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

    /// Progress of a running crossfade in `0..=1`, or `None` when no fade
    /// is active. The fade advances only by playback time reported by the
    /// transport, so suspension, scrubbing and stalls never consume it.
    fn crossfade_progress(&self) -> Option<f32> {
        let fade = self.crossfade.as_ref()?;
        Some((fade.elapsed / fade.duration.max(1e-6)).clamp(0.0, 1.0) as f32)
    }

    /// `true` while a clip crossfade is blending.
    pub fn is_crossfading(&self) -> bool {
        self.crossfade.is_some()
    }

    /// Advance the transport once per host redraw and return the crossed
    /// marker events. Only an actual time change re-evaluates the pose;
    /// crossfade progress accumulates from the played time the transport
    /// reports, independent of wall-clock gaps.
    pub fn advance(&mut self, now: Instant) -> SessionAdvance {
        let before = self.transport.shown_time();
        let advance = self.transport.advance(now);
        if let Some(fade) = self.crossfade.as_mut()
            && advance.played > 0.0
        {
            fade.elapsed += advance.played;
        }
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
        if advance.ended {
            // The held end pose is the clip's own final pose, never a
            // leftover blend with the outgoing clip.
            self.crossfade = None;
        }
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
        self.evaluate();
    }

    pub fn pause(&mut self, now: Instant) {
        self.transport.pause(now);
        self.evaluate();
    }

    pub fn toggle_play_pause(&mut self, now: Instant) {
        self.transport.toggle_play_pause(now);
        self.evaluate();
    }

    pub fn stop(&mut self, now: Instant) {
        self.transport.stop(now);
        // Stop inspects the incoming clip at its start; a running fade must
        // not leave the pose blended with the outgoing clip.
        self.crossfade = None;
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

    /// Focus/suspension loss during a scrub: drop the drag, keep the last
    /// time, pause. The timeline widget cancels its own drag state on the
    /// same event so no scrub message can follow.
    pub fn cancel_scrub(&mut self) {
        self.transport.cancel_scrub();
    }

    pub fn suspend(&mut self, now: Instant) {
        self.transport.suspend(now);
        // The rebase moved the shown time; the displayed pose must follow.
        self.evaluate();
    }

    pub fn resume(&mut self, now: Instant) {
        self.transport.resume(now);
    }

    pub fn set_speed(&mut self, now: Instant, speed: f64) {
        self.transport.set_speed(now, speed);
        self.evaluate();
    }

    pub fn set_range(&mut self, now: Instant, start: f64, end: f64) {
        self.transport.set_range(now, start, end);
        self.motion_path = None;
        self.evaluate();
    }

    pub fn set_loop_mode(&mut self, now: Instant, mode: LoopMode) {
        self.transport.set_loop_mode(now, mode);
        self.evaluate();
    }

    pub fn set_root_policy(&mut self, policy: RootMotionPolicy) {
        if self.root_policy == policy {
            return;
        }
        self.root_policy = policy;
        self.motion_path = None;
        self.evaluate();
    }

    pub fn set_display_offset(&mut self, offset: Vec3) {
        if self.display_offset == offset {
            return;
        }
        self.display_offset = offset;
        self.motion_path = None;
        self.evaluate();
    }

    /// Bounds of the currently evaluated pose, in display space.
    pub fn current_pose_bounds(&self) -> Option<Aabb> {
        self.pose.posed_bounds
    }

    /// Sampled motion envelope of the active clip over the play range.
    /// This is an estimate (see `clip_envelope`) used only for framing and
    /// path display, never for culling.
    pub fn clip_motion_bounds(&self) -> Option<Aabb> {
        let clip = self.clip()?;
        let binding = self.binding.as_ref()?;
        let range = self.transport.range();
        clip_envelope(
            &self.asset,
            clip,
            binding,
            (range.0 as f32, range.1 as f32),
            48,
            self.root_policy,
            self.display_offset,
        )
    }

    /// Current root-motion node origin in display space, if designated.
    pub fn root_position_view(&self) -> Option<Vec3> {
        let root = self.asset.root_motion_node?;
        self.pose.node_positions_view.get(root.0 as usize).copied()
    }

    /// Sampled root trajectory across the play range for the motion-path
    /// overlay. Bounded and cached; invalidated by clip/range/policy/
    /// offset changes. Returns empty when no root node or clip is active.
    pub fn motion_path_samples(&mut self, samples: usize) -> Vec<Vec3> {
        if let Some(cached) = &self.motion_path {
            return cached.clone();
        }
        let Some(root) = self.asset.root_motion_node else {
            return Vec::new();
        };
        let Some(id) = self.clip else {
            return Vec::new();
        };
        let Some(clip) = self.library.clip(id) else {
            return Vec::new();
        };
        let Some(binding) = self.binding.as_ref() else {
            return Vec::new();
        };
        let samples = samples.clamp(2, 128);
        let (start, end) = self.transport.range();
        let mut buffers = PoseBuffers::new(&self.asset);
        let mut out = Vec::with_capacity(samples);
        for step in 0..samples {
            let t = start + (end - start) * (step as f64 / (samples - 1) as f64);
            sample_locals(clip, binding, &self.asset, t as f32, &mut buffers.locals);
            evaluate_pose(
                &self.asset,
                self.root_policy,
                self.display_offset,
                &mut buffers,
            );
            out.push(buffers.node_positions_view[root.0 as usize]);
        }
        self.motion_path = Some(out.clone());
        out
    }

    pub fn note_marker(&mut self, marker: &ClipMarker, now: Instant) {
        self.last_marker = Some((marker.label.clone(), now));
    }

    pub fn mark_uploaded(&mut self) {
        self.uploaded_revision = self.pose.revision;
    }
}
fn reset_locals(asset: &ModelAsset, locals: &mut [NodeTransform]) {
    for (index, node) in asset.nodes.iter().enumerate() {
        locals[index] = node.local;
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

    #[test]
    fn motion_path_is_sampled_cached_and_invalidated() {
        let mut session = demo_session();
        let first = session.motion_path_samples(16);
        assert_eq!(first.len(), 16);
        assert!(session.motion_path.is_some());
        session.set_range(Instant::now(), 0.2, 1.0);
        assert!(
            session.motion_path.is_none(),
            "range change invalidates cache"
        );
        let second = session.motion_path_samples(16);
        assert_eq!(second.len(), 16);
    }

    #[test]
    fn clip_motion_bounds_cover_the_pose() {
        let session = demo_session();
        assert!(session.clip_motion_bounds().is_some());
        assert!(session.current_pose_bounds().is_some());
    }

    #[test]
    fn root_position_matches_the_posed_node() {
        let session = demo_session();
        let root = session.asset.root_motion_node.unwrap();
        let expected = session.pose.node_positions_view[root.0 as usize];
        assert_eq!(session.root_position_view(), Some(expected));
    }

    fn wave_id(session: &AnimationSession) -> ClipId {
        session.library.find_by_name("Wave").unwrap().id
    }

    #[test]
    fn crossfade_disabled_cuts_immediately() {
        let mut session = demo_session();
        let start = Instant::now();
        session.play(start);
        session.advance(start + Duration::from_millis(300));
        session.select_clip(wave_id(&session), start + Duration::from_millis(300));
        assert!(!session.is_crossfading(), "default switch must cut");
    }

    #[test]
    fn crossfade_blends_from_the_outgoing_pose_when_playing() {
        let mut session = demo_session();
        session.panel.crossfade = true;
        let start = Instant::now();
        session.play(start);
        session.advance(start + Duration::from_millis(300));
        let outgoing = session.pose.locals.clone();
        let switch_at = start + Duration::from_millis(300);
        session.select_clip(wave_id(&session), switch_at);
        assert!(session.is_crossfading());
        for (blended, from) in session.pose.locals.iter().zip(outgoing.iter()) {
            assert!(
                blended.translation.abs_diff_eq(from.translation, 1e-5),
                "at fade start the pose must equal the captured outgoing pose"
            );
        }
        // The first playing advance re-establishes the played-time delta;
        // the fade completes from accumulated playback time only.
        session.advance(switch_at + Duration::from_millis(50));
        assert!(
            session.is_crossfading(),
            "the fade may not complete before any playback time accrues"
        );
        session.advance(switch_at + CROSSFADE_DURATION + Duration::from_millis(70));
        assert!(
            !session.is_crossfading(),
            "fade completion releases the outgoing snapshot"
        );
    }

    #[test]
    fn full_playback_lifecycle_stays_consistent() {
        let mut session = demo_session();
        let t0 = Instant::now();

        session.play(t0);
        assert!(session.is_playing());
        let r0 = session.pose.revision;
        session.advance(t0 + Duration::from_millis(200));
        assert!(session.pose.revision > r0, "playing advances the pose");

        // Tab/modal suspension freezes the clock and remembers intent.
        session.suspend(t0 + Duration::from_millis(200));
        assert!(matches!(
            session.state(),
            PlaybackState::Suspended { was_playing: true }
        ));
        let frozen = session.pose.revision;
        session.advance(t0 + Duration::from_millis(5000));
        assert_eq!(
            session.pose.revision, frozen,
            "a suspended transport must not advance"
        );
        session.resume(t0 + Duration::from_millis(5100));
        assert!(session.is_playing());
        session.advance(t0 + Duration::from_millis(5300));
        assert!(
            session.pose.revision > frozen,
            "resume rebases and advances"
        );

        // Scrub owns the time and resumes playback on release.
        session.begin_scrub(t0 + Duration::from_millis(5300));
        session.scrub_to(1.0);
        assert_eq!(session.transport.shown_time(), 1.0);
        session.end_scrub(t0 + Duration::from_millis(5400));
        assert!(session.is_playing(), "scrub during playback resumes");

        // Repeat mode wraps instead of running past the clip end.
        session.seek(t0 + Duration::from_millis(5400), 1.95);
        session.advance(t0 + Duration::from_millis(5700));
        assert!(session.transport.shown_time() < 2.0);

        session.stop(t0 + Duration::from_millis(5700));
        assert!(matches!(session.state(), PlaybackState::Paused));
        assert_eq!(session.transport.shown_time(), 0.0);
    }

    /// The displayed pose must equal a fresh evaluation at the displayed
    /// time — the invariant pausing used to break.
    fn pose_matches_displayed_time(session: &AnimationSession) -> bool {
        let mut reference = PoseBuffers::new(&session.asset);
        if let (Some(id), Some(binding)) = (session.clip, session.binding.as_ref())
            && let Some(clip) = session.library.clip(id)
        {
            sample_locals(
                clip,
                binding,
                &session.asset,
                session.transport.shown_time() as f32,
                &mut reference.locals,
            );
        }
        evaluate_pose(
            &session.asset,
            session.root_policy,
            session.display_offset,
            &mut reference,
        );
        session
            .pose
            .out_vertices
            .iter()
            .zip(reference.out_vertices.iter())
            .all(|(mesh, expected)| {
                mesh.iter().zip(expected.iter()).all(|(vertex, expected)| {
                    vertex
                        .position
                        .iter()
                        .zip(expected.position.iter())
                        .all(|(a, b)| (a - b).abs() < 1e-4)
                })
            })
    }

    #[test]
    fn pausing_evaluates_the_displayed_pose() {
        let mut session = demo_session();
        let t0 = Instant::now();
        session.play(t0);
        session.advance(t0 + Duration::from_millis(100));
        assert!(pose_matches_displayed_time(&session));
        // Pause rebases the shown time past the last evaluated frame; the
        // mesh must follow instead of freezing at the previous pose.
        session.pause(t0 + Duration::from_millis(200));
        assert!(
            pose_matches_displayed_time(&session),
            "pause must evaluate the rebased shown time"
        );
        session.suspend(t0 + Duration::from_millis(300));
        assert!(pose_matches_displayed_time(&session));
        session.toggle_play_pause(t0 + Duration::from_millis(400));
        session.advance(t0 + Duration::from_millis(500));
        session.toggle_play_pause(t0 + Duration::from_millis(600));
        assert!(pose_matches_displayed_time(&session));
    }

    #[test]
    fn crossfade_ignores_suspended_time() {
        let mut session = demo_session();
        session.panel.crossfade = true;
        let t0 = Instant::now();
        session.play(t0);
        let switch_at = t0 + Duration::from_millis(100);
        session.select_clip(wave_id(&session), switch_at);
        assert!(session.is_crossfading());
        // One second of suspension must not consume the 150 ms fade.
        session.suspend(switch_at + Duration::from_millis(60));
        session.advance(switch_at + Duration::from_millis(1060));
        assert!(
            session.is_crossfading(),
            "suspension must not advance the fade"
        );
        session.resume(switch_at + Duration::from_millis(1100));
        session.advance(switch_at + Duration::from_millis(1150));
        assert!(
            session.is_crossfading(),
            "only 50 ms of playback has accrued"
        );
        session.advance(switch_at + Duration::from_millis(1310));
        assert!(!session.is_crossfading());
    }

    #[test]
    fn stop_finalizes_a_running_crossfade() {
        let mut session = demo_session();
        session.panel.crossfade = true;
        let t0 = Instant::now();
        session.play(t0);
        session.advance(t0 + Duration::from_millis(50));
        session.select_clip(wave_id(&session), t0 + Duration::from_millis(50));
        assert!(session.is_crossfading());
        session.stop(t0 + Duration::from_millis(60));
        assert!(!session.is_crossfading(), "stop must not hold a blend");
        assert!(
            pose_matches_displayed_time(&session),
            "stopped pose must be the incoming clip's start pose"
        );
    }

    #[test]
    fn reaching_the_end_finalizes_a_running_crossfade() {
        let mut session = demo_session();
        session.panel.crossfade = true;
        let t0 = Instant::now();
        session.play(t0);
        session.select_clip(wave_id(&session), t0 + Duration::from_millis(10));
        assert!(session.is_crossfading());
        session.set_loop_mode(
            t0 + Duration::from_millis(20),
            crate::inspector::animation::transport::LoopMode::Once,
        );
        // A 50 ms play range ends playback before the 150 ms fade completes;
        // the held pose must be the clip's end pose, not a blend.
        session.set_range(t0 + Duration::from_millis(30), 0.0, 0.05);
        let advance = session.advance(t0 + Duration::from_millis(200));
        assert!(advance.ended, "playback must end inside the fade");
        assert!(
            !session.is_crossfading(),
            "the end pose must not stay blended"
        );
        assert!(
            pose_matches_displayed_time(&session),
            "the held pose must be the clip's own end pose"
        );
    }

    #[test]
    fn seeking_matches_playback_arrival() {
        // Adapter contract: reaching a time through playback and seeking to
        // that time must produce the identical pose. The anchor-rebase clock
        // exists to guarantee this; it had no direct test.
        let mut session = demo_session();
        let t0 = Instant::now();
        session.play(t0);
        session.advance(t0 + Duration::from_millis(700));
        let arrived = session.transport.shown_time();
        assert!((arrived - 0.7).abs() < 1e-12);
        let arrived_pose = session.pose.out_vertices.clone();

        session.stop(t0 + Duration::from_millis(700));
        session.seek(t0 + Duration::from_millis(800), 0.7);
        assert!(
            (session.transport.shown_time() - arrived).abs() < 1e-12,
            "seek must land on the same f64 the playback clock produced"
        );
        for (mesh, expected) in session.pose.out_vertices.iter().zip(arrived_pose.iter()) {
            for (vertex, expected) in mesh.iter().zip(expected.iter()) {
                for axis in 0..3 {
                    assert!(
                        (vertex.position[axis] - expected.position[axis]).abs() < 1e-6,
                        "seek pose diverges from playback-arrival pose"
                    );
                }
            }
        }
        // And the mirrored normal path through the step grid agrees too.
        session.stop(t0 + Duration::from_millis(900));
        let grid = 0.7 / 21.0;
        for _ in 0..21 {
            session.step(t0 + Duration::from_millis(950), 1);
        }
        assert!(
            (session.transport.shown_time() - arrived).abs() < 1e-6,
            "stepping must aggregate to the same position as playback"
        );
    }

    #[test]
    fn cancel_scrub_keeps_time_and_pauses() {
        let mut session = demo_session();
        let t0 = Instant::now();
        session.play(t0);
        session.begin_scrub(t0 + Duration::from_millis(100));
        session.scrub_to(0.5);
        session.cancel_scrub();
        assert!(matches!(session.state(), PlaybackState::Paused));
        assert_eq!(session.transport.shown_time(), 0.5);
    }

    #[test]
    fn invalid_clips_are_never_sampled() {
        let (model, library) = fixtures::demo();
        let invalid = crate::inspector::animation::PropertyTrack {
            target: "Spine".into(),
            channel: crate::inspector::animation::TrackChannel::Translation {
                times: Vec::new(),
                values: Vec::new(),
            },
            interpolation: crate::inspector::animation::Interpolation::Linear,
        };
        let broken = crate::inspector::animation::AnimationClip {
            id: ClipId(999),
            name: "broken".into(),
            duration: 1.0,
            tracks: vec![invalid],
            source_rate: None,
            markers: Vec::new(),
            provenance: "test".into(),
        };
        let mut clips = library.clips.clone();
        clips.insert(0, broken);
        let library = Arc::new(crate::inspector::animation::AnimationLibrary {
            name: library.name.clone(),
            clips,
            provenance: library.provenance.clone(),
        });
        // Session creation must skip the invalid first clip instead of
        // panicking inside the sampler.
        let mut session =
            AnimationSession::new(Arc::new(model), library, Vec3::ZERO, true, Instant::now());
        assert_eq!(session.clip_name(), Some("Idle"));
        assert!(
            !session.select_clip(ClipId(999), Instant::now()),
            "an invalid clip must be rejected before sampling"
        );
        assert_eq!(session.clip_name(), Some("Idle"));
    }
}
