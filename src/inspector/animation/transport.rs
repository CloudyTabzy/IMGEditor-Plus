//! Transport: play/pause/stop/seek/step/range/loop/speed/suspend with an
//! anchored time model.
//!
//! ```text
//! unwrapped_time = anchored_clip_time + elapsed_active_host_time * playback_speed
//! sample_time    = apply_play_range_and_loop_policy(unwrapped_time)
//! ```
//!
//! On pause, seek, speed change or resume the current position is
//! evaluated first and the anchor rebased — playback never accumulates a
//! guessed per-frame increment. The clock is injected: every transition
//! takes the current `Instant`, so tests are deterministic and drawing
//! never advances time.

use std::time::Instant;

use crate::inspector::animation::clip::{AnimationClip, ClipMarker, SourceRate};

/// A redraw gap longer than this while playing is treated as an
/// unexplained stall: playback pauses and rebases at the last shown time
/// instead of jumping through many loops.
pub const LONG_GAP: std::time::Duration = std::time::Duration::from_millis(250);

/// Loop policy for the play range.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoopMode {
    /// Play `[start, end]` once, then hold the end pose in `Ended`.
    Once,
    /// Sample the half-open interval `[start, end)`; a paused seek to
    /// `end` still inspects the last pose.
    #[default]
    Repeat,
}

/// Explicit transport states. Loading/binding status lives elsewhere;
/// this is purely the time owner.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PlaybackState {
    /// No clip bound.
    #[default]
    Empty,
    /// Ready and holding an exact time/pose.
    Paused,
    /// Advancing with the host clock.
    Playing,
    /// A timeline drag owns the time; remembers whether playback was
    /// active so `end_scrub` can resume it.
    Scrubbing { was_playing: bool },
    /// Playback intent remembered across tab/modal/focus suspension.
    Suspended { was_playing: bool },
    /// `Once` playback reached the range end; Play restarts at the start.
    Ended,
    /// Evaluation/binding failed; `String` explains.
    Failed(String),
}

/// Result of one [`Transport::advance`] call.
#[derive(Clone, Debug, Default)]
pub struct Advance {
    /// The sample time to evaluate and display.
    pub time: f64,
    /// Full loop wraps crossed during this advance (Repeat mode).
    pub wraps: u32,
    /// `Once` playback reached the range end this advance.
    pub ended: bool,
    /// Playback auto-paused because the redraw gap exceeded [`LONG_GAP`].
    pub paused_by_gap: bool,
}

/// The animation transport. All times are canonical seconds; the source
/// rate (when known) drives frame stepping and the frame readout.
#[derive(Clone, Debug)]
pub struct Transport {
    state: PlaybackState,
    duration: f64,
    range: (f64, f64),
    loop_mode: LoopMode,
    speed: f64,
    /// Frames per second for stepping and the frame readout.
    step_rate: f64,
    /// Whether `step_rate` came from the source format (`true`) or is
    /// the labelled 30 fps preview rate (`false`).
    rate_from_source: bool,
    anchor_clip: f64,
    anchor_host: Option<Instant>,
    /// Last evaluated/shown time — what the viewport is displaying.
    shown_time: f64,
    last_advance: Option<Instant>,
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            state: PlaybackState::Empty,
            duration: 0.0,
            range: (0.0, 0.0),
            loop_mode: LoopMode::Repeat,
            speed: 1.0,
            step_rate: SourceRate::PREVIEW_30.fps(),
            rate_from_source: false,
            anchor_clip: 0.0,
            anchor_host: None,
            shown_time: 0.0,
            last_advance: None,
        }
    }
}

impl Transport {
    /// Bind a clip: paused at the range start, per the transport table's
    /// "load model or select a new clip" row.
    pub fn set_clip(&mut self, clip: &AnimationClip, now: Instant) {
        self.duration = clip.duration.max(0.0) as f64;
        self.range = (0.0, self.duration);
        self.anchor_clip = 0.0;
        self.shown_time = 0.0;
        self.anchor_host = None;
        self.last_advance = None;
        self.state = PlaybackState::Paused;
        match clip.source_rate {
            Some(rate) => {
                self.step_rate = rate.fps();
                self.rate_from_source = true;
            }
            None => {
                self.step_rate = SourceRate::PREVIEW_30.fps();
                self.rate_from_source = false;
            }
        }
        let _ = now;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn state(&self) -> &PlaybackState {
        &self.state
    }

    pub fn is_playing(&self) -> bool {
        matches!(self.state, PlaybackState::Playing)
    }

    /// `true` while the transport owns a clip that can advance (zero
    /// duration clips produce a static pose with play disabled).
    pub fn can_play(&self) -> bool {
        self.duration > 0.0
            && matches!(
                self.state,
                PlaybackState::Paused
                    | PlaybackState::Playing
                    | PlaybackState::Suspended { .. }
                    | PlaybackState::Ended
            )
    }

    /// Eligibility independent of the transient state (used while
    /// deciding what a scrub/suspend resolves into).
    fn eligible(&self) -> bool {
        self.duration > 0.0
            && !matches!(self.state, PlaybackState::Empty | PlaybackState::Failed(_))
    }

    /// The time the viewport is showing. Between advances this is also
    /// the live sample time while playing.
    pub fn shown_time(&self) -> f64 {
        self.shown_time
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn range(&self) -> (f64, f64) {
        self.range
    }

    pub fn speed(&self) -> f64 {
        self.speed
    }

    pub fn loop_mode(&self) -> LoopMode {
        self.loop_mode
    }

    pub fn step_rate(&self) -> f64 {
        self.step_rate
    }

    pub fn rate_from_source(&self) -> bool {
        self.rate_from_source
    }

    /// Frame index of the shown time at the display rate (readout only).
    pub fn shown_frame(&self) -> u64 {
        (self.shown_time * self.step_rate).floor().max(0.0) as u64
    }

    pub fn total_frames(&self) -> u64 {
        (self.duration * self.step_rate).floor().max(0.0) as u64
    }

    fn unwrapped(&self, now: Instant) -> f64 {
        match self.anchor_host {
            Some(anchor) => {
                self.anchor_clip + now.saturating_duration_since(anchor).as_secs_f64() * self.speed
            }
            None => self.anchor_clip,
        }
    }

    /// Apply the play range and loop policy. Returns the sample time and
    /// how many full range wraps the unwrapped time crossed.
    fn apply_loop(&self, unwrapped: f64) -> (f64, u32) {
        let (start, end) = self.range;
        let span = end - start;
        if span <= 0.0 {
            return (start, 0);
        }
        match self.loop_mode {
            LoopMode::Once => (unwrapped.clamp(start, end), 0),
            LoopMode::Repeat => {
                let rel = unwrapped - start;
                if rel < 0.0 {
                    return (start, 0);
                }
                let wraps_f = (rel / span).floor();
                let sample = start + rel.rem_euclid(span);
                (sample, wraps_f.max(0.0).min(u32::MAX as f64) as u32)
            }
        }
    }

    /// Evaluate the current position and rebase the anchor. Every
    /// transition that changes speed/range/state goes through here so
    /// the displayed time stays continuous.
    fn rebase(&mut self, now: Instant) {
        let (sample, _) = self.apply_loop(self.unwrapped(now));
        self.anchor_clip = sample;
        self.shown_time = sample;
        self.anchor_host = if self.is_playing() { Some(now) } else { None };
        self.last_advance = None;
    }

    /// Advance once per redraw while playing. Returns the time to
    /// evaluate; non-playing states simply hold their shown time.
    pub fn advance(&mut self, now: Instant) -> Advance {
        if !self.is_playing() {
            return Advance {
                time: self.shown_time,
                ..Advance::default()
            };
        }
        if let Some(previous) = self.last_advance
            && now.saturating_duration_since(previous) > LONG_GAP
        {
            // Stall policy: pause at the last shown time instead of
            // jumping through many loops after a freeze.
            self.anchor_clip = self.shown_time;
            self.anchor_host = None;
            self.state = PlaybackState::Paused;
            self.last_advance = None;
            return Advance {
                time: self.shown_time,
                paused_by_gap: true,
                ..Advance::default()
            };
        }
        self.last_advance = Some(now);
        let (sample, wraps) = self.apply_loop(self.unwrapped(now));
        if self.loop_mode == LoopMode::Once && self.unwrapped(now) >= self.range.1 {
            self.shown_time = self.range.1;
            self.anchor_clip = self.range.1;
            self.anchor_host = None;
            self.state = PlaybackState::Ended;
            self.last_advance = None;
            return Advance {
                time: self.range.1,
                ended: true,
                ..Advance::default()
            };
        }
        let advance = Advance {
            time: sample,
            wraps,
            ..Advance::default()
        };
        self.shown_time = sample;
        advance
    }

    /// Play, or — at the end in `Once` mode — restart at the range start.
    pub fn play(&mut self, now: Instant) {
        if !self.can_play() {
            return;
        }
        if matches!(self.state, PlaybackState::Ended) {
            self.anchor_clip = self.range.0;
            self.shown_time = self.range.0;
        }
        self.state = PlaybackState::Playing;
        self.anchor_host = Some(now);
        self.last_advance = None;
    }

    /// Pause, keeping the exact time and pose.
    pub fn pause(&mut self, now: Instant) {
        if self.is_playing() {
            self.rebase(now);
        }
        if !matches!(self.state, PlaybackState::Empty | PlaybackState::Failed(_)) {
            self.state = PlaybackState::Paused;
        }
    }

    /// Toggle play/pause (the Space shortcut and the Play button).
    pub fn toggle_play_pause(&mut self, now: Instant) {
        if self.is_playing() {
            self.pause(now);
        } else {
            self.play(now);
        }
    }

    /// Stop: pause and seek to the range start. The camera is untouched.
    pub fn stop(&mut self, now: Instant) {
        if matches!(self.state, PlaybackState::Empty | PlaybackState::Failed(_)) {
            return;
        }
        self.state = PlaybackState::Paused;
        self.anchor_host = None;
        self.anchor_clip = self.range.0;
        self.shown_time = self.range.0;
        self.last_advance = None;
        let _ = now;
    }

    /// Seek, clamped to the valid range; the pose is evaluated by the
    /// caller immediately after. Seeking while playing keeps playing
    /// with a rebased anchor; from `Ended` it resumes inspection paused.
    pub fn seek(&mut self, now: Instant, time: f64) {
        if matches!(self.state, PlaybackState::Empty | PlaybackState::Failed(_)) {
            return;
        }
        let clamped = time.clamp(self.range.0, self.range.1);
        if matches!(self.state, PlaybackState::Ended) {
            self.state = PlaybackState::Paused;
        }
        self.anchor_clip = clamped;
        self.shown_time = clamped;
        if self.is_playing() {
            self.anchor_host = Some(now);
        }
        self.last_advance = None;
    }

    /// Begin a timeline scrub: remember whether playback was active and
    /// suspend advancement while the drag owns the time.
    pub fn begin_scrub(&mut self, now: Instant) {
        let was_playing = self.is_playing();
        if was_playing {
            self.rebase(now);
        }
        if matches!(self.state, PlaybackState::Empty | PlaybackState::Failed(_)) {
            return;
        }
        if matches!(self.state, PlaybackState::Ended) {
            self.state = PlaybackState::Paused;
        }
        let was_playing = was_playing;
        self.state = PlaybackState::Scrubbing { was_playing };
        self.anchor_host = None;
        self.last_advance = None;
    }

    /// Move the playhead during a scrub (clamped to the range).
    pub fn scrub_to(&mut self, time: f64) {
        if !matches!(self.state, PlaybackState::Scrubbing { .. }) {
            return;
        }
        let clamped = time.clamp(self.range.0, self.range.1);
        self.anchor_clip = clamped;
        self.shown_time = clamped;
    }

    /// End a scrub: rebase, and resume only if playback was active and
    /// is still eligible.
    pub fn end_scrub(&mut self, now: Instant) {
        let PlaybackState::Scrubbing { was_playing } = self.state else {
            return;
        };
        if was_playing && self.eligible() {
            self.state = PlaybackState::Playing;
            self.anchor_host = Some(now);
        } else {
            self.state = PlaybackState::Paused;
            self.anchor_host = None;
        }
        self.last_advance = None;
    }

    /// Lost focus during a scrub: cancel the drag cleanly, retain the
    /// last time, and pause.
    pub fn cancel_scrub(&mut self) {
        if matches!(self.state, PlaybackState::Scrubbing { .. }) {
            self.state = PlaybackState::Paused;
            self.anchor_host = None;
            self.last_advance = None;
        }
    }

    /// Suspend playback because the viewer became ineligible (tab
    /// switch, modal, focus loss, hidden window). Remembers intent.
    pub fn suspend(&mut self, now: Instant) {
        // A scrub interrupted by suspension is cancelled first: no stuck
        // drag survives a focus loss.
        self.cancel_scrub();
        match self.state {
            PlaybackState::Playing => {
                self.rebase(now);
                self.state = PlaybackState::Suspended {
                    was_playing: true,
                };
            }
            PlaybackState::Paused | PlaybackState::Ended => {
                self.state = PlaybackState::Suspended {
                    was_playing: false,
                };
            }
            _ => {}
        }
    }

    /// Resume from suspension; rebases the clock so no elapsed time
    /// leaks in from the suspended interval.
    pub fn resume(&mut self, now: Instant) {
        let PlaybackState::Suspended { was_playing } = self.state else {
            return;
        };
        if was_playing && self.can_play() {
            self.state = PlaybackState::Playing;
            self.anchor_host = Some(now);
        } else if matches!(self.state, PlaybackState::Suspended { .. }) {
            self.state = PlaybackState::Paused;
            self.anchor_host = None;
        }
        self.last_advance = None;
    }

    /// Change playback speed, keeping the current time continuous.
    pub fn set_speed(&mut self, now: Instant, speed: f64) {
        let speed = speed.clamp(0.05, 8.0);
        if self.is_playing() {
            self.rebase(now);
        }
        self.speed = speed;
    }

    /// Change the play range. Validated `start < end`, clamped into the
    /// clip; the current time is clamped and rebased.
    pub fn set_range(&mut self, now: Instant, start: f64, end: f64) {
        if !start.is_finite() || !end.is_finite() || start >= end {
            return;
        }
        let start = start.clamp(0.0, self.duration);
        let end = end.clamp(0.0, self.duration);
        if start >= end {
            return;
        }
        self.range = (start, end);
        if self.is_playing() {
            self.rebase(now);
        }
        let clamped = self.shown_time.clamp(start, end);
        self.anchor_clip = clamped;
        self.shown_time = clamped;
    }

    pub fn set_loop_mode(&mut self, now: Instant, mode: LoopMode) {
        if self.loop_mode == mode {
            return;
        }
        if self.is_playing() {
            self.rebase(now);
        }
        self.loop_mode = mode;
    }

    /// Step one display frame on the stepping grid, pausing first. The
    /// grid is `1 / step_rate`; stepping forward from the last grid
    /// point lands exactly on the range end so the final pose stays
    /// inspectable even when the duration is not an integer frame count.
    pub fn step(&mut self, now: Instant, direction: i32) {
        if matches!(
            self.state,
            PlaybackState::Empty | PlaybackState::Failed(_)
        ) {
            return;
        }
        self.pause(now);
        if self.duration <= 0.0 {
            return;
        }
        let grid = 1.0 / self.step_rate;
        let (start, end) = self.range;
        let eps = 1e-9 * (end - start).max(1.0);
        let t = self.shown_time;
        let next = if direction >= 0 {
            let k = ((t - start) / grid).floor();
            let candidate = start + (k + 1.0) * grid;
            if candidate > end - eps { end } else { candidate }
        } else {
            let k = ((t - start) / grid).ceil();
            let candidate = start + (k - 1.0) * grid;
            if candidate < start + eps { start } else { candidate }
        };
        self.anchor_clip = next;
        self.shown_time = next;
        self.anchor_host = None;
        self.last_advance = None;
    }

    /// Marker events crossed during forward playback over
    /// `(from, to]` with `wraps` full range wraps between them. Seeking
    /// never calls this; markers are inspection data, and no behaviour
    /// runs because a marker was crossed. Work is bounded: more than
    /// [`MAX_MARKER_WRAP_REPORT`] wraps collapses to one representative
    /// set per boundary class.
    pub fn crossed_markers(
        markers: &[ClipMarker],
        from: f64,
        to: f64,
        wraps: u32,
        range: (f64, f64),
    ) -> Vec<ClipMarker> {
        const MAX_MARKER_WRAP_REPORT: u32 = 8;
        const MAX_REPORT: usize = 64;
        let mut crossed = Vec::new();
        let (start, end) = range;
        if wraps == 0 {
            for marker in markers {
                let time = marker.time as f64;
                if time > from && time <= to {
                    crossed.push(marker.clone());
                }
            }
            return crossed;
        }
        // Tail of the outgoing cycle: (from, end].
        for marker in markers {
            let time = marker.time as f64;
            if time > from && time <= end {
                crossed.push(marker.clone());
            }
        }
        // Representative full cycles (bounded).
        for _ in 0..wraps.saturating_sub(1).min(MAX_MARKER_WRAP_REPORT) {
            for marker in markers {
                let time = marker.time as f64;
                if time >= start && time < end {
                    crossed.push(marker.clone());
                }
            }
            if crossed.len() >= MAX_REPORT {
                return crossed;
            }
        }
        // Head of the final cycle: [start, to].
        for marker in markers {
            let time = marker.time as f64;
            if time >= start && time <= to {
                crossed.push(marker.clone());
            }
        }
        crossed.truncate(MAX_REPORT);
        crossed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn clip_seconds(seconds: f32) -> AnimationClip {
        AnimationClip {
            id: crate::inspector::animation::ClipId(0),
            name: "t".into(),
            duration: seconds,
            tracks: Vec::new(),
            source_rate: Some(SourceRate::PREVIEW_30),
            markers: Vec::new(),
            provenance: "test".into(),
        }
    }

    fn transport(seconds: f32) -> (Transport, Instant) {
        let start = Instant::now();
        let mut transport = Transport::default();
        transport.set_clip(&clip_seconds(seconds), start);
        (transport, start)
    }

    #[test]
    fn new_clip_starts_paused_at_zero() {
        let (transport, _) = transport(2.0);
        assert_eq!(*transport.state(), PlaybackState::Paused);
        assert_eq!(transport.shown_time(), 0.0);
    }

    #[test]
    fn playback_advances_with_host_clock() {
        let (mut transport, start) = transport(2.0);
        transport.play(start);
        let advance = transport.advance(start + Duration::from_millis(500));
        assert!((advance.time - 0.5).abs() < 1e-9);
        assert!((transport.shown_time() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn pause_keeps_exact_time() {
        let (mut transport, start) = transport(2.0);
        transport.play(start);
        transport.pause(start + Duration::from_millis(333));
        assert!((transport.shown_time() - 0.333).abs() < 1e-9);
        // No advance while paused.
        let advance = transport.advance(start + Duration::from_secs(10));
        assert!((advance.time - 0.333).abs() < 1e-9);
    }

    #[test]
    fn seek_during_playback_rebases_anchor() {
        let (mut transport, start) = transport(4.0);
        transport.play(start);
        transport.seek(start + Duration::from_millis(100), 2.0);
        // 100 ms real time continue from the seek point.
        let advance = transport.advance(start + Duration::from_millis(200));
        assert!((advance.time - 2.1).abs() < 1e-9);
        assert!(transport.is_playing());
    }

    #[test]
    fn once_mode_ends_and_play_restarts() {
        let (mut transport, start) = transport(1.0);
        transport.set_loop_mode(start, LoopMode::Once);
        transport.play(start);
        let advance = transport.advance(start + Duration::from_millis(1500));
        assert!(advance.ended);
        assert_eq!(*transport.state(), PlaybackState::Ended);
        assert_eq!(transport.shown_time(), 1.0);
        transport.play(start + Duration::from_secs(2));
        assert_eq!(transport.shown_time(), 0.0);
        assert!(transport.is_playing());
    }

    #[test]
    fn repeat_wraps_half_open_interval() {
        let (mut transport, start) = transport(1.0);
        transport.play(start);
        // 2.25 s at 1x over a 1 s range: wraps twice, samples 0.25.
        let advance = transport.advance(start + Duration::from_millis(2250));
        assert_eq!(advance.wraps, 2);
        assert!((advance.time - 0.25).abs() < 1e-9);
    }

    #[test]
    fn speed_change_is_time_continuous() {
        let (mut transport, start) = transport(10.0);
        transport.play(start);
        transport.set_speed(start + Duration::from_secs(1), 2.0);
        // At the rebase instant the position is exactly 1.0 s.
        assert!((transport.shown_time() - 1.0).abs() < 1e-9);
        let advance = transport.advance(start + Duration::from_secs(2));
        assert!((advance.time - 3.0).abs() < 1e-9);
    }

    #[test]
    fn long_gap_pauses_at_last_shown_time() {
        let (mut transport, start) = transport(60.0);
        transport.play(start);
        let first = transport.advance(start + Duration::from_millis(100));
        assert!((first.time - 0.1).abs() < 1e-9);
        let second = transport.advance(start + Duration::from_millis(100) + LONG_GAP + Duration::from_millis(1));
        assert!(second.paused_by_gap);
        assert!((second.time - 0.1).abs() < 1e-9);
        assert_eq!(*transport.state(), PlaybackState::Paused);
    }

    #[test]
    fn scrub_lifecycle_suspends_and_resumes() {
        let (mut transport, start) = transport(2.0);
        transport.play(start);
        transport.begin_scrub(start + Duration::from_millis(300));
        transport.scrub_to(1.5);
        assert_eq!(transport.shown_time(), 1.5);
        // Time does not advance during a scrub.
        let advance = transport.advance(start + Duration::from_secs(5));
        assert_eq!(advance.time, 1.5);
        transport.end_scrub(start + Duration::from_secs(5));
        assert!(transport.is_playing());
        let advance = transport.advance(start + Duration::from_millis(5100));
        assert!((advance.time - 1.6).abs() < 1e-9);
    }

    #[test]
    fn scrub_from_pause_stays_paused() {
        let (mut transport, start) = transport(2.0);
        transport.begin_scrub(start);
        transport.scrub_to(0.75);
        transport.end_scrub(start + Duration::from_millis(16));
        assert_eq!(*transport.state(), PlaybackState::Paused);
        assert_eq!(transport.shown_time(), 0.75);
    }

    #[test]
    fn lost_focus_cancels_drag_and_pauses() {
        let (mut transport, start) = transport(2.0);
        transport.play(start);
        transport.begin_scrub(start + Duration::from_millis(100));
        transport.scrub_to(1.0);
        transport.cancel_scrub();
        assert_eq!(*transport.state(), PlaybackState::Paused);
        assert_eq!(transport.shown_time(), 1.0);
    }

    #[test]
    fn suspend_and_resume_rebase_the_clock() {
        let (mut transport, start) = transport(4.0);
        transport.play(start);
        transport.suspend(start + Duration::from_millis(400));
        assert_eq!(
            *transport.state(),
            PlaybackState::Suspended { was_playing: true }
        );
        // Ten suspended seconds must not leak into the clip time.
        transport.resume(start + Duration::from_secs(10));
        assert!(transport.is_playing());
        let advance = transport.advance(start + Duration::from_millis(10100));
        assert!((advance.time - 0.5).abs() < 1e-9);
    }

    #[test]
    fn stop_pauses_and_returns_to_range_start() {
        let (mut transport, start) = transport(2.0);
        transport.set_range(start, 0.5, 1.5);
        transport.play(start);
        transport.advance(start + Duration::from_millis(200));
        transport.stop(start + Duration::from_millis(200));
        assert_eq!(*transport.state(), PlaybackState::Paused);
        assert_eq!(transport.shown_time(), 0.5);
    }

    #[test]
    fn set_range_validates_and_clamps() {
        let (mut transport, start) = transport(2.0);
        transport.set_range(start, 1.5, 0.5);
        assert_eq!(transport.range(), (0.0, 2.0));
        transport.set_range(start, 0.25, 1.75);
        assert_eq!(transport.range(), (0.25, 1.75));
        transport.seek(start, 2.0);
        // Seek is clamped to the active range.
        assert_eq!(transport.shown_time(), 1.75);
    }

    #[test]
    fn step_moves_on_display_frame_grid_and_hits_exact_end() {
        let (mut transport, start) = transport(1.0);
        transport.step(start, 1);
        let grid = 1.0 / 30.0;
        assert!((transport.shown_time() - grid).abs() < 1e-9);
        // Step to the end: the last step lands exactly on the end pose.
        for i in 0..40 {
            transport.step(start + Duration::from_millis(i), 1);
        }
        assert_eq!(transport.shown_time(), 1.0);
        // Stepping back from the exact end lands on the grid below it.
        transport.step(start + Duration::from_millis(100), -1);
        assert!((transport.shown_time() - 29.0 * grid).abs() < 1e-9);
        assert_eq!(*transport.state(), PlaybackState::Paused);
    }

    #[test]
    fn zero_duration_clip_disables_play() {
        let (mut transport, start) = transport(0.0);
        assert!(!transport.can_play());
        transport.play(start);
        assert!(!transport.is_playing());
    }

    #[test]
    fn marker_crossing_forward_only_and_bounded() {
        let markers = vec![
            ClipMarker {
                time: 0.25,
                label: "a".into(),
            },
            ClipMarker {
                time: 0.75,
                label: "b".into(),
            },
        ];
        // Simple forward crossing within one cycle.
        let crossed = Transport::crossed_markers(&markers, 0.2, 0.5, 0, (0.0, 1.0));
        assert_eq!(crossed.len(), 1);
        assert_eq!(crossed[0].label, "a");
        // One wrap: tail (0.9, 1.0] has nothing; head [0, 0.3] has "a".
        let crossed = Transport::crossed_markers(&markers, 0.9, 0.3, 1, (0.0, 1.0));
        assert_eq!(crossed.len(), 1);
        assert_eq!(crossed[0].label, "a");
        // Many wraps are bounded and still include both boundary classes.
        let crossed = Transport::crossed_markers(&markers, 0.9, 0.3, 100, (0.0, 1.0));
        assert!(crossed.len() <= 64);
        assert!(crossed.iter().any(|m| m.label == "a"));
        assert!(crossed.iter().any(|m| m.label == "b"));
    }

    #[test]
    fn irregular_redraw_rates_sample_the_same_active_time() {
        // Two different frame schedules reaching the same host time must
        // produce the same sample time (determinism).
        let (mut a, start) = transport(5.0);
        let (mut b, _) = transport(5.0);
        a.play(start);
        b.play(start);
        // A renders at 30 Hz, B at 144 Hz; both sample at t=1.0 s.
        for frame in 1..=30 {
            a.advance(start + Duration::from_secs_f64(frame as f64 / 30.0));
        }
        for frame in 1..=144 {
            b.advance(start + Duration::from_secs_f64(frame as f64 / 144.0));
        }
        assert!((a.shown_time() - b.shown_time()).abs() < 1e-9);
        assert!((a.shown_time() - 1.0).abs() < 1e-9);
    }
}
