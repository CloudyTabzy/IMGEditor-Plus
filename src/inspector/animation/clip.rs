//! Animation clips: typed property tracks, time metadata and validation.
//!
//! A clip is immutable after validation. Each channel owns its key times;
//! a clip never fabricates shared or uniform timestamps, and channels
//! absent from a track leave the node's default local component
//! untouched. Times are canonical seconds (`f32`); the transport works in
//! `f64` and converts at the sampling boundary.

use std::collections::HashMap;

use glam::{Quat, Vec3};

use crate::inspector::animation::ClipId;

/// Interpolation between two adjacent keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Interpolation {
    /// Hold the previous key's value until the next key.
    Step,
    /// Linear blend; rotations use shortest-arc slerp.
    #[default]
    Linear,
}

/// One animated property of one target node, with its own key times.
#[derive(Clone, Debug, PartialEq)]
pub enum TrackChannel {
    Translation { times: Vec<f32>, values: Vec<Vec3> },
    Rotation { times: Vec<f32>, values: Vec<Quat> },
    Scale { times: Vec<f32>, values: Vec<Vec3> },
}

impl TrackChannel {
    pub fn times(&self) -> &[f32] {
        match self {
            TrackChannel::Translation { times, .. }
            | TrackChannel::Rotation { times, .. }
            | TrackChannel::Scale { times, .. } => times,
        }
    }

    pub fn key_count(&self) -> usize {
        self.times().len()
    }

    /// Discriminant used to detect two tracks writing the same property
    /// of the same node, which has no defined composition.
    fn kind(&self) -> &'static str {
        match self {
            TrackChannel::Translation { .. } => "translation",
            TrackChannel::Rotation { .. } => "rotation",
            TrackChannel::Scale { .. } => "scale",
        }
    }

    fn value_len(&self) -> usize {
        match self {
            TrackChannel::Translation { values, .. } => values.len(),
            TrackChannel::Rotation { values, .. } => values.len(),
            TrackChannel::Scale { values, .. } => values.len(),
        }
    }
}

/// One animated property track: a source target identity plus a channel.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyTrack {
    /// Source identity of the target node (adapter-owned namespace: NIF
    /// node name, RenderWare frame name, or a fixture name). Resolution
    /// to a runtime node happens in `binding`.
    pub target: String,
    pub channel: TrackChannel,
    pub interpolation: Interpolation,
}

/// Timed inspection marker. Markers are observation data only: playback
/// reports crossed markers but never executes behaviour for them.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipMarker {
    pub time: f32,
    pub label: String,
}

/// Rational display/sample rate carried from the source format. When a
/// source does not declare one, the viewer uses a clearly labelled
/// preview rate (30 fps) for frame stepping and the frame readout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceRate {
    pub numerator: u32,
    pub denominator: u32,
}

impl SourceRate {
    pub const PREVIEW_30: Self = Self {
        numerator: 30,
        denominator: 1,
    };

    pub fn fps(self) -> f64 {
        if self.denominator == 0 {
            return 30.0;
        }
        let fps = self.numerator as f64 / self.denominator as f64;
        if fps.is_finite() && fps > 0.0 {
            fps
        } else {
            30.0
        }
    }
}

/// An immutable, validated animation clip.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationClip {
    pub id: ClipId,
    pub name: String,
    /// Canonical duration in seconds: the last key time across all
    /// tracks. A zero-duration clip produces a static pose.
    pub duration: f32,
    pub tracks: Vec<PropertyTrack>,
    pub source_rate: Option<SourceRate>,
    pub markers: Vec<ClipMarker>,
    /// Freeform provenance note (adapter name, fixture identity).
    pub provenance: String,
}

/// Clip validation failure. Adapters surface these as diagnostics; the
/// runtime never plays an invalid clip.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ClipError {
    #[error("clip duration {0} is not finite and non-negative")]
    InvalidDuration(f32),
    #[error("track '{target}' {channel} has {times} key times but {values} values")]
    KeyCountMismatch {
        target: String,
        channel: &'static str,
        times: usize,
        values: usize,
    },
    #[error("track '{target}' {channel} has no keys")]
    EmptyChannel {
        target: String,
        channel: &'static str,
    },
    #[error("track '{target}' {channel} key {index} has non-finite time")]
    NonFiniteTime {
        target: String,
        channel: &'static str,
        index: usize,
    },
    #[error(
        "track '{target}' {channel} key {index} is not strictly ascending; duplicate or descending timestamps need an explicit adapter rule"
    )]
    UnorderedTimes {
        target: String,
        channel: &'static str,
        index: usize,
    },
    #[error("track '{target}' {channel} key {index} has a non-finite value")]
    NonFiniteValue {
        target: String,
        channel: &'static str,
        index: usize,
    },
    #[error("track '{target}' rotation key {index} is a zero-norm quaternion")]
    InvalidQuaternion { target: String, index: usize },
    #[error(
        "tracks {first} and {second} both write {channel} of '{target}'; duplicate writers have no defined composition"
    )]
    DuplicateWriter {
        target: String,
        channel: &'static str,
        first: usize,
        second: usize,
    },
    #[error("marker '{label}' has non-finite or negative time")]
    InvalidMarker { label: String },
}

/// A sampled channel value, typed to match its track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SampledChannel {
    Translation(Vec3),
    Rotation(Quat),
    Scale(Vec3),
}

impl AnimationClip {
    pub fn validate(&self) -> Result<(), ClipError> {
        if !self.duration.is_finite() || self.duration < 0.0 {
            return Err(ClipError::InvalidDuration(self.duration));
        }
        let mut writers: HashMap<(&str, &str), usize> = HashMap::new();
        for (track_index, track) in self.tracks.iter().enumerate() {
            let channel = track.channel.kind();
            let times = track.channel.times();
            let values = track.channel.value_len();
            if times.len() != values {
                return Err(ClipError::KeyCountMismatch {
                    target: track.target.clone(),
                    channel,
                    times: times.len(),
                    values,
                });
            }
            if times.is_empty() {
                return Err(ClipError::EmptyChannel {
                    target: track.target.clone(),
                    channel,
                });
            }
            for (index, &time) in times.iter().enumerate() {
                if !time.is_finite() {
                    return Err(ClipError::NonFiniteTime {
                        target: track.target.clone(),
                        channel,
                        index,
                    });
                }
                if index > 0 && time <= times[index - 1] {
                    return Err(ClipError::UnorderedTimes {
                        target: track.target.clone(),
                        channel,
                        index,
                    });
                }
            }
            match &track.channel {
                TrackChannel::Translation { values, .. } | TrackChannel::Scale { values, .. } => {
                    for (index, value) in values.iter().enumerate() {
                        if !value.is_finite() {
                            return Err(ClipError::NonFiniteValue {
                                target: track.target.clone(),
                                channel,
                                index,
                            });
                        }
                    }
                }
                TrackChannel::Rotation { values, .. } => {
                    for (index, value) in values.iter().enumerate() {
                        if !value.is_finite() {
                            return Err(ClipError::NonFiniteValue {
                                target: track.target.clone(),
                                channel,
                                index,
                            });
                        }
                        if value.length() <= 1e-6 {
                            return Err(ClipError::InvalidQuaternion {
                                target: track.target.clone(),
                                index,
                            });
                        }
                    }
                }
            }
            if let Some(first) = writers.insert((track.target.as_str(), channel), track_index) {
                return Err(ClipError::DuplicateWriter {
                    target: track.target.clone(),
                    channel,
                    first,
                    second: track_index,
                });
            }
        }
        for marker in &self.markers {
            if !marker.time.is_finite() || marker.time < 0.0 {
                return Err(ClipError::InvalidMarker {
                    label: marker.label.clone(),
                });
            }
        }
        Ok(())
    }

    /// Normalize rotation keys in place. Source data may carry slightly
    /// denormalized quaternions; slerp expects unit inputs. Validation
    /// still rejects zero-norm keys, so this never rescues garbage.
    pub fn normalize_rotation_keys(&mut self) {
        for track in &mut self.tracks {
            if let TrackChannel::Rotation { values, .. } = &mut track.channel {
                for value in values.iter_mut() {
                    *value = value.normalize();
                }
            }
        }
    }

    /// `true` when the clip can never move: no tracks or a zero duration.
    /// The transport disables Play for static clips but still allows
    /// inspecting their single pose.
    pub fn is_static(&self) -> bool {
        self.duration <= 0.0 || self.tracks.is_empty()
    }

    /// Sample one track at clip-local `time` seconds. The caller clamps
    /// the time into the valid range; sampling holds the first/last key
    /// value outside the keyed interval and never invents identity keys
    /// for absent channels.
    pub fn sample_track(track: &PropertyTrack, time: f32) -> SampledChannel {
        match &track.channel {
            TrackChannel::Translation { times, values } => SampledChannel::Translation(
                sample_keys(times, values, time, track.interpolation, |a, b, t| {
                    a.lerp(b, t)
                }),
            ),
            TrackChannel::Rotation { times, values } => SampledChannel::Rotation(sample_keys(
                times,
                values,
                time,
                track.interpolation,
                |a, b, t| a.slerp(b, t).normalize(),
            )),
            TrackChannel::Scale { times, values } => SampledChannel::Scale(sample_keys(
                times,
                values,
                time,
                track.interpolation,
                |a, b, t| a.lerp(b, t),
            )),
        }
    }
}

/// Key-window search and interpolation. Deterministic: seeking directly
/// to a time and reaching it during playback produce the same value.
/// Exact-key selection picks the key whose timestamp equals `time`.
fn sample_keys<T: Copy + Default>(
    times: &[f32],
    values: &[T],
    time: f32,
    interpolation: Interpolation,
    blend: impl Fn(T, T, f32) -> T,
) -> T {
    debug_assert_eq!(times.len(), values.len());
    // Invalid clips are rejected at admission, so an empty channel here
    // means a caller bypassed validation; hold the channel default rather
    // than panic.
    let Some(&first) = values.first() else {
        return T::default();
    };
    // Count of keys with time <= t; the current key is one before it.
    let after = times.partition_point(|&key| key <= time);
    if after == 0 {
        // Before the first key: hold the first value.
        return first;
    }
    let current = after - 1;
    if current + 1 >= times.len() || interpolation == Interpolation::Step {
        return values[current];
    }
    let t0 = times[current];
    let t1 = times[current + 1];
    if time <= t0 {
        return values[current];
    }
    // Validated strictly ascending times guarantee t1 > t0 and, because
    // of the partition above, time < t1: no zero-interval division and
    // the factor always lies in (0, 1).
    blend(
        values[current],
        values[current + 1],
        (time - t0) / (t1 - t0),
    )
}

/// A package of clips decoded from one source (or produced by a fixture).
/// Large packages may decode clips lazily; the library only promises
/// stable [`ClipId`] identities and package metadata.
#[derive(Clone, Debug)]
pub struct AnimationLibrary {
    pub name: String,
    pub clips: Vec<AnimationClip>,
    pub provenance: String,
}

impl AnimationLibrary {
    pub fn clip(&self, id: ClipId) -> Option<&AnimationClip> {
        self.clips.iter().find(|clip| clip.id == id)
    }

    pub fn find_by_name(&self, name: &str) -> Option<&AnimationClip> {
        self.clips.iter().find(|clip| clip.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_channel_samples_to_the_channel_default() {
        // Invalid clips are rejected at admission; this guards the sampler
        // itself against a panic if an unvalidated channel is sampled.
        let track = PropertyTrack {
            target: "n".into(),
            channel: TrackChannel::Translation {
                times: Vec::new(),
                values: Vec::new(),
            },
            interpolation: Interpolation::Linear,
        };
        assert_eq!(
            AnimationClip::sample_track(&track, 0.5),
            SampledChannel::Translation(Vec3::ZERO)
        );
    }

    fn rot_z(degrees: f32) -> Quat {
        Quat::from_rotation_z(degrees.to_radians())
    }

    fn linear_clip(times: Vec<f32>, values: Vec<Vec3>) -> AnimationClip {
        let duration = *times.last().unwrap_or(&0.0);
        AnimationClip {
            id: ClipId(0),
            name: "test".into(),
            duration,
            tracks: vec![PropertyTrack {
                target: "Node".into(),
                channel: TrackChannel::Translation { times, values },
                interpolation: Interpolation::Linear,
            }],
            source_rate: None,
            markers: Vec::new(),
            provenance: "unit test".into(),
        }
    }

    #[test]
    fn sample_exact_key_returns_key_value() {
        let clip = linear_clip(
            vec![0.0, 1.0, 2.0],
            vec![Vec3::ZERO, Vec3::X, Vec3::new(2.0, 0.0, 0.0)],
        );
        let sampled = AnimationClip::sample_track(&clip.tracks[0], 1.0);
        assert_eq!(sampled, SampledChannel::Translation(Vec3::X));
    }

    #[test]
    fn sample_linear_midpoint_blends() {
        let clip = linear_clip(vec![0.0, 2.0], vec![Vec3::ZERO, Vec3::new(4.0, 0.0, 0.0)]);
        let sampled = AnimationClip::sample_track(&clip.tracks[0], 1.0);
        assert_eq!(
            sampled,
            SampledChannel::Translation(Vec3::new(2.0, 0.0, 0.0))
        );
    }

    #[test]
    fn sample_step_holds_previous_key() {
        let mut clip = linear_clip(vec![0.0, 1.0], vec![Vec3::ZERO, Vec3::X]);
        clip.tracks[0].interpolation = Interpolation::Step;
        let sampled = AnimationClip::sample_track(&clip.tracks[0], 0.999);
        assert_eq!(sampled, SampledChannel::Translation(Vec3::ZERO));
        let sampled = AnimationClip::sample_track(&clip.tracks[0], 1.0);
        assert_eq!(sampled, SampledChannel::Translation(Vec3::X));
    }

    #[test]
    fn sample_outside_keyed_interval_holds_boundary_values() {
        let clip = linear_clip(vec![0.5, 1.0], vec![Vec3::ZERO, Vec3::X]);
        assert_eq!(
            AnimationClip::sample_track(&clip.tracks[0], 0.0),
            SampledChannel::Translation(Vec3::ZERO)
        );
        assert_eq!(
            AnimationClip::sample_track(&clip.tracks[0], 2.0),
            SampledChannel::Translation(Vec3::X)
        );
    }

    #[test]
    fn rotation_slerp_takes_shortest_arc() {
        let track = PropertyTrack {
            target: "Node".into(),
            channel: TrackChannel::Rotation {
                times: vec![0.0, 1.0],
                values: vec![rot_z(170.0), rot_z(-170.0)],
            },
            interpolation: Interpolation::Linear,
        };
        let SampledChannel::Rotation(q) = AnimationClip::sample_track(&track, 0.5) else {
            panic!("expected rotation");
        };
        // Shortest arc crosses ±180°, passing through 180° (never through 0°).
        let (_, _, z) = q.to_euler(glam::EulerRot::XYZ);
        assert!(
            z.to_degrees().abs() > 170.0,
            "midpoint should stay near ±180°, got {}",
            z.to_degrees()
        );
    }

    #[test]
    fn validation_rejects_descending_and_duplicate_times() {
        let mut clip = linear_clip(vec![0.0, 0.5, 0.5], vec![Vec3::ZERO; 3]);
        assert!(matches!(
            clip.validate(),
            Err(ClipError::UnorderedTimes { index: 2, .. })
        ));
        clip = linear_clip(vec![1.0, 0.5], vec![Vec3::ZERO; 2]);
        assert!(matches!(
            clip.validate(),
            Err(ClipError::UnorderedTimes { .. })
        ));
    }

    #[test]
    fn validation_rejects_bad_counts_and_quats() {
        let mut clip = linear_clip(vec![0.0, 1.0], vec![Vec3::ZERO]);
        assert!(matches!(
            clip.validate(),
            Err(ClipError::KeyCountMismatch { .. })
        ));
        clip.tracks[0].channel = TrackChannel::Rotation {
            times: vec![0.0],
            values: vec![Quat::from_xyzw(0.0, 0.0, 0.0, 0.0)],
        };
        clip.duration = 0.0;
        assert!(matches!(
            clip.validate(),
            Err(ClipError::InvalidQuaternion { .. })
        ));
    }

    #[test]
    fn validation_rejects_duplicate_writers() {
        let track = || PropertyTrack {
            target: "Node".into(),
            channel: TrackChannel::Scale {
                times: vec![0.0],
                values: vec![Vec3::ONE],
            },
            interpolation: Interpolation::Linear,
        };
        let clip = AnimationClip {
            id: ClipId(0),
            name: "dup".into(),
            duration: 0.0,
            tracks: vec![track(), track()],
            source_rate: None,
            markers: Vec::new(),
            provenance: "unit test".into(),
        };
        assert!(matches!(
            clip.validate(),
            Err(ClipError::DuplicateWriter { .. })
        ));
    }

    #[test]
    fn one_key_clip_is_static_and_holds_its_pose() {
        let clip = linear_clip(vec![0.0], vec![Vec3::X]);
        assert!(clip.is_static());
        assert_eq!(
            AnimationClip::sample_track(&clip.tracks[0], 0.0),
            SampledChannel::Translation(Vec3::X)
        );
    }

    #[test]
    fn source_rate_preview_fallback() {
        assert_eq!(SourceRate::PREVIEW_30.fps(), 30.0);
        assert_eq!(
            SourceRate {
                numerator: 0,
                denominator: 0
            }
            .fps(),
            30.0
        );
        assert_eq!(
            SourceRate {
                numerator: 30000,
                denominator: 1001
            }
            .fps(),
            30000.0 / 1001.0
        );
    }
}
