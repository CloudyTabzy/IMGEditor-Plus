//! Bully (PC) AGR reader — experimental, evidence-based.
//!
//! Reverse-engineered from the retail PC corpus by the local probe suite
//! (`bully-probe/`, outside the repository). The container and all four
//! record variants are corpus-validated (554 files, 3,261 clips, zero
//! structural errors).
//!
//! Validated layout:
//!
//! ```text
//! file    = chunk*
//! chunk   = { u32 magic=0x100, u32 variant ∈ 999..=1004,
//!             u32 record_count, u32 0, f32 duration_s }
//!           + preamble: P records   (1002 only; P = data_bytes/8 - count)
//!           + records
//!           + optional 4-byte trailer when data_bytes % 8 == 4
//! ```
//!
//! Record layouts:
//!
//! - **1002** (character change stream, 8 B):
//!   `{ u8 time_cs, u8 track<<3|channel, 3 × i16 }`. Rotation channels store
//!   Gamebryo compact quaternions `(x, y, z)` at 1/32767 with
//!   `w = sqrt(1 - |v|²)`; translation channels are 1/32767 m. Time is a
//!   wrapping u8 centisecond counter resolved with a wrap heuristic.
//! - **999** (object float records, 32 B):
//!   `{ u16 ordinal, u16 time_norm, f32 w, x, y, z, f32 tx, ty, tz }`.
//!   `time_norm / 65535 * duration` lands on exact 30 fps frames (verified
//!   against ANIBALL). Ordinal-zero records are chunk defaults.
//! - **1003** (object compact records, 20 B):
//!   `{ u16 ordinal, u16 time_norm, i16 x, y, z, w, i16 tx, ty, tz, u16 }`.
//!   Same time rule; values at 1/32767.
//! - **1004** (object change stream, 12 B):
//!   `{ u8 frame, u8 track<<3|channel, 3 × i16, u32 tail }`. Dense 30 fps
//!   frame index; the tail is serialized runtime data and is ignored.
//!
//! The runtime-facing key space is normalized: rotation keys are unit-quat
//! `(x, y, z)` components, translation keys are metres.

use std::ops::RangeInclusive;

use glam::{Mat3, Quat, Vec3};

use crate::inspector::animation::ClipId;
use crate::inspector::animation::NodeId;
use crate::inspector::animation::clip::{
    AnimationClip, AnimationLibrary, Interpolation, PropertyTrack, SourceRate, TrackChannel,
};
use crate::inspector::animation::model::{MeshAsset, ModelAsset, NodeTransform, SceneNode};
use crate::inspector::nif::{BlockPayload, NifFile};
use crate::inspector::scene3d::camera::BaseOrientation;
use crate::inspector::scene3d::mesh::Vertex;

/// First u32 of every chunk.
pub const AGR_MAGIC: u32 = 0x0100;

/// Accepted chunk variant words (second u32).
pub const AGR_VARIANTS: RangeInclusive<u32> = 999..=1004;

const CHUNK_HEADER_BYTES: usize = 20;
const RECORD_BYTES: usize = 8;
const COMPACT_QUAT_SCALE: f32 = 32767.0;
/// Time unit: `time_lo` is centiseconds. Unverified in detail; the reader
/// records the uncertainty in the clip diagnostics.
const TIME_UNITS_PER_SECOND: f32 = 100.0;
/// A decrease larger than this is a wrap (u8 range / 2).
const WRAP_THRESHOLD: i32 = 128;

/// AGR admission/parse failure. Every variant is a typed error so malformed
/// fixtures can be rejected without panicking.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum AgrError {
    #[error("AGR data is {len} bytes; the smallest valid chunk header is {CHUNK_HEADER_BYTES}")]
    TooShort { len: usize },
    #[error("chunk at offset {offset} starts with 0x{word:08X}, not the 0x{expected:04X} magic")]
    BadMagic {
        offset: usize,
        word: u32,
        expected: u32,
    },
    #[error("chunk at offset {offset} has unsupported variant {variant} (expected {min}..={max})")]
    BadVariant {
        offset: usize,
        variant: u32,
        min: u32,
        max: u32,
    },
    #[error(
        "chunk at offset {offset} has {data_bytes} data bytes, not a multiple of 8 (after trailer)"
    )]
    MisalignedData { offset: usize, data_bytes: usize },
    #[error("chunk at offset {offset} declares {count} records but only {slots} fit")]
    RecordOverflow {
        offset: usize,
        count: usize,
        slots: usize,
    },
    #[error("chunk at offset {offset} has non-finite or unreasonable duration {duration}")]
    BadDuration { offset: usize, duration: f32 },
}

/// One decoded key: value components plus resolved clip time.
///
/// `values` are channel-space components: for rotation channels the `(x, y, z)`
/// part of a unit quaternion (w is derived); for translation channels metres.
/// Every decoder normalizes into this space so the library conversion and the
/// runtime never see raw record units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AgrKey {
    pub time_s: f32,
    pub values: [f32; 3],
}

impl AgrKey {
    /// Interpret the key as a compact Gamebryo rotation.
    pub fn rotation(&self) -> Quat {
        let x = self.values[0];
        let y = self.values[1];
        let z = self.values[2];
        let w = (1.0 - x * x - y * y - z * z).max(0.0).sqrt();
        Quat::from_xyzw(x, y, z, w).normalize()
    }
}

/// Keys of one track channel within a clip.
#[derive(Clone, Debug, PartialEq)]
pub struct AgrTrack {
    /// `header >> 3`.
    pub track: u8,
    /// `header & 7`. 0 = rotation; 1/2 are not yet rendered (units unknown).
    pub channel: u8,
    pub keys: Vec<AgrKey>,
}

/// One decoded clip (chunk). Chunk 0 doubles as the file header.
#[derive(Clone, Debug, PartialEq)]
pub struct AgrClip {
    pub index: usize,
    /// Chunk variant word (999..=1004); determines the record format.
    pub variant: u32,
    /// Record size in bytes for this variant (0 when unknown).
    pub record_size: usize,
    pub duration_s: f32,
    /// Records before the declared change records (variant-1002 per-track
    /// preamble; zero for the fixed-size object variants).
    pub preamble_records: usize,
    pub tracks: Vec<AgrTrack>,
    /// Parse notes: dropped out-of-order or duplicate keys, undecoded
    /// variants, trailer, etc.
    pub diagnostics: Vec<String>,
}

/// A parsed AGR file.
#[derive(Clone, Debug, PartialEq)]
pub struct AgrFile {
    pub variant: u32,
    pub clips: Vec<AgrClip>,
}

impl AgrFile {
    pub fn clip_count(&self) -> usize {
        self.clips.len()
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_f32(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// Find every chunk start: u32 magic followed by any known variant word.
/// Chunk variants can differ within one file (mixed 999/1002/1003 exist in
/// the object-animation corpus), so the file's first variant is not used as
/// a filter.
fn chunk_starts(bytes: &[u8]) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut offset = 0usize;
    while offset + CHUNK_HEADER_BYTES <= bytes.len() {
        if read_u32(bytes, offset) == AGR_MAGIC {
            let second = read_u32(bytes, offset + 4);
            if AGR_VARIANTS.contains(&second) {
                starts.push(offset);
            }
        }
        offset += 4;
    }
    starts
}

/// Parse an AGR byte slice.
pub fn parse_agr(bytes: &[u8]) -> Result<AgrFile, AgrError> {
    if bytes.len() < CHUNK_HEADER_BYTES {
        return Err(AgrError::TooShort { len: bytes.len() });
    }
    let magic = read_u32(bytes, 0);
    if magic != AGR_MAGIC {
        return Err(AgrError::BadMagic {
            offset: 0,
            word: magic,
            expected: AGR_MAGIC,
        });
    }
    let variant = read_u32(bytes, 4);
    if !AGR_VARIANTS.contains(&variant) {
        return Err(AgrError::BadVariant {
            offset: 0,
            variant,
            min: *AGR_VARIANTS.start(),
            max: *AGR_VARIANTS.end(),
        });
    }

    // Archive entries are stored in whole 2048-byte sectors; the tail of the
    // final chunk is zero padding, not records. Loose files carry no padding,
    // so trimming all-zero u32 words from the end is safe for both.
    let mut end = bytes.len();
    let mut padding_trimmed = 0usize;
    while end >= 4 && read_u32(bytes, end - 4) == 0 {
        end -= 4;
        padding_trimmed += 4;
    }
    let mut diagnostics = Vec::new();
    if padding_trimmed > 0 {
        diagnostics.push(format!("trimmed {padding_trimmed} bytes of tail padding"));
    }

    let starts = chunk_starts(&bytes[..end]);
    if starts.first() != Some(&0) {
        return Err(AgrError::BadMagic {
            offset: 0,
            word: magic,
            expected: AGR_MAGIC,
        });
    }

    let mut clips = Vec::with_capacity(starts.len());
    for (index, &start) in starts.iter().enumerate() {
        let chunk_end = starts.get(index + 1).copied().unwrap_or(end);
        clips.push(parse_clip(&bytes[..end], start, chunk_end, index)?);
    }
    if !diagnostics.is_empty()
        && let Some(first) = clips.first_mut()
    {
        let mut merged = diagnostics;
        merged.extend(std::mem::take(&mut first.diagnostics));
        first.diagnostics = merged;
    }
    Ok(AgrFile { variant, clips })
}

/// Record size for a chunk variant, when the layout is established.
///
/// Corpus-validated sizes: 999 -> 32 B, 1002 -> 8 B (+ per-track preamble),
/// 1003 -> 20 B, 1004 -> 12 B. Variants 1000/1001 are still unreduced.
fn variant_record_size(variant: u32) -> Option<usize> {
    match variant {
        999 => Some(32),
        1002 => Some(8),
        1003 => Some(20),
        1004 => Some(12),
        _ => None,
    }
}

fn parse_clip(bytes: &[u8], start: usize, end: usize, index: usize) -> Result<AgrClip, AgrError> {
    let variant = read_u32(bytes, start + 4);
    let count = read_u32(bytes, start + 8) as usize;
    let duration = read_f32(bytes, start + 16);
    if !duration.is_finite() || duration <= 0.0 || duration > 600.0 {
        return Err(AgrError::BadDuration {
            offset: start,
            duration,
        });
    }
    let Some(record_size) = variant_record_size(variant) else {
        return Ok(AgrClip {
            index,
            variant,
            record_size: 0,
            duration_s: duration,
            preamble_records: 0,
            tracks: Vec::new(),
            diagnostics: vec![format!(
                "variant {variant} chunk ({count} records): record layout not yet decoded"
            )],
        });
    };
    let mut data_bytes = end.saturating_sub(start + CHUNK_HEADER_BYTES);
    let mut diagnostics = Vec::new();
    if data_bytes % record_size == 4 {
        data_bytes -= 4;
        diagnostics.push("stripped 4-byte trailer".to_string());
    }
    if !data_bytes.is_multiple_of(record_size) {
        return Err(AgrError::MisalignedData {
            offset: start,
            data_bytes,
        });
    }
    let slots = data_bytes / record_size;
    if count > slots {
        return Err(AgrError::RecordOverflow {
            offset: start,
            count,
            slots,
        });
    }
    let preamble_records = slots - count;
    let body = &bytes[start + CHUNK_HEADER_BYTES..start + CHUNK_HEADER_BYTES + data_bytes];
    let tracks = match variant {
        1002 => decode_change_records(
            body,
            slots,
            count,
            preamble_records,
            duration,
            &mut diagnostics,
        ),
        999 => decode_object_float_records(body, count, duration, &mut diagnostics),
        1003 => decode_object_compact_records(body, count, duration, &mut diagnostics),
        1004 => decode_object_1004_records(body, count, duration, &mut diagnostics),
        _ => {
            // Record sizes are known but field semantics for the object
            // variants are still being reduced.
            return Ok(AgrClip {
                index,
                variant,
                record_size,
                duration_s: duration,
                preamble_records,
                tracks: Vec::new(),
                diagnostics: vec![format!(
                    "variant {variant} chunk ({count} records of {record_size} B): \
                     field layout not yet decoded"
                )],
            });
        }
    };
    Ok(AgrClip {
        index,
        variant,
        record_size,
        duration_s: duration,
        preamble_records,
        tracks,
        diagnostics,
    })
}

/// Groups and decodes the 8-byte change records of a variant-1002 chunk.
fn decode_change_records(
    body: &[u8],
    slots: usize,
    count: usize,
    preamble_records: usize,
    duration: f32,
    diagnostics: &mut Vec<String>,
) -> Vec<AgrTrack> {
    let _ = count;
    let mut grouped: Vec<AgrTrack> = Vec::new();
    let mut dropped_reversed = 0usize;
    let mut dropped_duplicate = 0usize;
    for slot in preamble_records..slots {
        let record = &body[slot * RECORD_BYTES..(slot + 1) * RECORD_BYTES];
        let time_lo = record[0] as i32;
        let header = record[1];
        let track = header >> 3;
        let channel = header & 7;
        let values = [
            i16::from_le_bytes([record[2], record[3]]) as f32 / COMPACT_QUAT_SCALE,
            i16::from_le_bytes([record[4], record[5]]) as f32 / COMPACT_QUAT_SCALE,
            i16::from_le_bytes([record[6], record[7]]) as f32 / COMPACT_QUAT_SCALE,
        ];
        let entry = match grouped
            .iter_mut()
            .find(|t| t.track == track && t.channel == channel)
        {
            Some(existing) => existing,
            None => {
                grouped.push(AgrTrack {
                    track,
                    channel,
                    keys: Vec::new(),
                });
                grouped.last_mut().expect("just pushed")
            }
        };
        let wraps = entry
            .keys
            .last()
            .map(|key| key.time_s * TIME_UNITS_PER_SECOND)
            .map(|prev_units| (prev_units / 256.0).floor() as i32)
            .unwrap_or(0);
        let prev_lo = entry
            .keys
            .last()
            .map(|key| (key.time_s * TIME_UNITS_PER_SECOND) as i32 - wraps * 256)
            .unwrap_or(0);
        let (unit_time, this_wrap) = if entry.keys.is_empty() {
            (time_lo, wraps)
        } else if time_lo < prev_lo {
            if prev_lo - time_lo > WRAP_THRESHOLD {
                (time_lo, wraps + 1)
            } else {
                dropped_reversed += 1;
                continue;
            }
        } else if time_lo == prev_lo {
            dropped_duplicate += 1;
            continue;
        } else {
            (time_lo, wraps)
        };
        let time_s = (this_wrap * 256 + unit_time) as f32 / TIME_UNITS_PER_SECOND;
        if time_s > duration + 0.25 {
            dropped_reversed += 1;
            continue;
        }
        entry.keys.push(AgrKey { time_s, values });
    }
    if dropped_reversed > 0 {
        diagnostics.push(format!(
            "dropped {dropped_reversed} out-of-order keys (time heuristic)"
        ));
    }
    if dropped_duplicate > 0 {
        diagnostics.push(format!("dropped {dropped_duplicate} duplicate-time keys"));
    }
    for track in &mut grouped {
        track.keys.sort_by(|a, b| a.time_s.total_cmp(&b.time_s));
    }
    grouped.retain(|track| !track.keys.is_empty());
    grouped.sort_by_key(|track| (track.track, track.channel));
    grouped
}

/// Pushes one key, creating the track on first use.
fn push_key(tracks: &mut Vec<AgrTrack>, track: u8, channel: u8, time_s: f32, values: [f32; 3]) {
    if let Some(existing) = tracks
        .iter_mut()
        .find(|t| t.track == track && t.channel == channel)
    {
        existing.keys.push(AgrKey { time_s, values });
    } else {
        tracks.push(AgrTrack {
            track,
            channel,
            keys: vec![AgrKey { time_s, values }],
        });
    }
}

/// Sorts each track's keys by time and drops same-time duplicates, then
/// removes empty tracks and sorts the track list canonically.
fn finish_tracks(mut tracks: Vec<AgrTrack>) -> Vec<AgrTrack> {
    for track in &mut tracks {
        track.keys.sort_by(|a, b| a.time_s.total_cmp(&b.time_s));
        track
            .keys
            .dedup_by(|a, b| (a.time_s - b.time_s).abs() < 1e-6);
    }
    tracks.retain(|track| !track.keys.is_empty());
    tracks.sort_by_key(|track| (track.track, track.channel));
    tracks
}

/// Decodes a variant-999 chunk: 32-byte float object records.
///
/// Layout (corpus-validated on SK8Board PICKUP and ANIBALL):
/// `{ u16 ordinal, u16 time_norm, f32 w, x, y, z, tx, ty, tz }`.
/// `time_norm / 65535 * duration` lands on exact 30 fps frames (verified:
/// ANIBALL STAND_DRIBBLE keys at frames 1..24 of a 0.8 s clip). The
/// quaternion is Gamebryo order `(w, x, y, z)`; records with ordinal zero are
/// the chunk's default preamble and are skipped. Translations are metres.
fn decode_object_float_records(
    body: &[u8],
    count: usize,
    duration: f32,
    diagnostics: &mut Vec<String>,
) -> Vec<AgrTrack> {
    let mut tracks = Vec::new();
    let mut defaults = 0usize;
    for i in 0..count {
        let record = &body[i * 32..(i + 1) * 32];
        let ordinal = u16::from_le_bytes([record[0], record[1]]);
        if ordinal == 0 {
            defaults += 1;
            continue;
        }
        let time_norm = u16::from_le_bytes([record[2], record[3]]) as f32 / 65535.0;
        let time_s = time_norm * duration;
        let x = read_f32(record, 8);
        let y = read_f32(record, 12);
        let z = read_f32(record, 16);
        let tx = read_f32(record, 20);
        let ty = read_f32(record, 24);
        let tz = read_f32(record, 28);
        push_key(&mut tracks, 0, 0, time_s, [x, y, z]);
        push_key(&mut tracks, 0, 1, time_s, [tx, ty, tz]);
    }
    if defaults > 0 {
        diagnostics.push(format!("skipped {defaults} default record(s)"));
    }
    finish_tracks(tracks)
}

/// Decodes a variant-1003 chunk: 20-byte compact object records.
///
/// Layout (corpus-validated on AniBroom, unit quaternions to 1e-4):
/// `{ u16 ordinal, u16 time_norm, i16 x, y, z, w, i16 tx, ty, tz, u16 pad }`.
/// Values are 1/32767-scale; the quaternion here is `(x, y, z, w)` (stored w;
/// the reader derives w from xyz like the other object variants so the
/// downstream representation stays uniform).
fn decode_object_compact_records(
    body: &[u8],
    count: usize,
    duration: f32,
    diagnostics: &mut Vec<String>,
) -> Vec<AgrTrack> {
    let mut tracks = Vec::new();
    let mut defaults = 0usize;
    for i in 0..count {
        let record = &body[i * 20..(i + 1) * 20];
        let ordinal = u16::from_le_bytes([record[0], record[1]]);
        if ordinal == 0 {
            defaults += 1;
            continue;
        }
        let time_norm = u16::from_le_bytes([record[2], record[3]]) as f32 / 65535.0;
        let time_s = time_norm * duration;
        let x = i16::from_le_bytes([record[4], record[5]]) as f32 / COMPACT_QUAT_SCALE;
        let y = i16::from_le_bytes([record[6], record[7]]) as f32 / COMPACT_QUAT_SCALE;
        let z = i16::from_le_bytes([record[8], record[9]]) as f32 / COMPACT_QUAT_SCALE;
        let tx = i16::from_le_bytes([record[12], record[13]]) as f32 / COMPACT_QUAT_SCALE;
        let ty = i16::from_le_bytes([record[14], record[15]]) as f32 / COMPACT_QUAT_SCALE;
        let tz = i16::from_le_bytes([record[16], record[17]]) as f32 / COMPACT_QUAT_SCALE;
        push_key(&mut tracks, 0, 0, time_s, [x, y, z]);
        push_key(&mut tracks, 0, 1, time_s, [tx, ty, tz]);
    }
    if defaults > 0 {
        diagnostics.push(format!("skipped {defaults} default record(s)"));
    }
    finish_tracks(tracks)
}

/// Decodes a variant-1004 chunk: 12-byte compact change records.
///
/// Layout (corpus-validated on SK8Board GIV/EXAMINE): `{ u8 frame,
/// u8 track<<3|channel, i16 x, y, z, u32 tail }`. The frame byte is a dense
/// 30 fps frame index (records advance 1,3,4,...); the tail is serialized
/// runtime data and is ignored. Values are compact-quat / translation
/// components at 1/32767 scale.
fn decode_object_1004_records(
    body: &[u8],
    count: usize,
    duration: f32,
    diagnostics: &mut Vec<String>,
) -> Vec<AgrTrack> {
    let mut tracks = Vec::new();
    let mut defaults = 0usize;
    for i in 0..count {
        let record = &body[i * 12..(i + 1) * 12];
        let frame = record[0];
        if frame == 0 {
            defaults += 1;
            continue;
        }
        let header = record[1];
        let track = header >> 3;
        let channel = header & 7;
        let time_s = (frame as f32 / 30.0).min(duration);
        let x = i16::from_le_bytes([record[2], record[3]]) as f32 / COMPACT_QUAT_SCALE;
        let y = i16::from_le_bytes([record[4], record[5]]) as f32 / COMPACT_QUAT_SCALE;
        let z = i16::from_le_bytes([record[6], record[7]]) as f32 / COMPACT_QUAT_SCALE;
        push_key(&mut tracks, track, channel, time_s, [x, y, z]);
    }
    if defaults > 0 {
        diagnostics.push(format!("skipped {defaults} default record(s)"));
    }
    finish_tracks(tracks)
}

/// Convert a parsed file into a runtime [`AnimationLibrary`].
///
/// Rotation channels (0) become compact-quat tracks; translation channels
/// (1) become position tracks at a trial scale of 1/32767 (meters-scale
/// model space — the same normalization as the rotation components).
/// Channel 2 (scale) is still withheld. Track targets are `track_{id}` so
/// the NIF adapter can map ids to nodes.
pub fn to_library(file: &AgrFile, name: impl Into<String>) -> AnimationLibrary {
    let clips = file
        .clips
        .iter()
        .map(|clip| {
            let mut tracks = Vec::new();
            for track in &clip.tracks {
                let times: Vec<f32> = track.keys.iter().map(|key| key.time_s).collect();
                match track.channel {
                    0 => {
                        let values: Vec<Quat> =
                            track.keys.iter().map(|key| key.rotation()).collect();
                        tracks.push(PropertyTrack {
                            target: format!("track_{:03}", track.track),
                            channel: TrackChannel::Rotation { times, values },
                            interpolation: Interpolation::Linear,
                        });
                    }
                    1 => {
                        let values: Vec<glam::Vec3> = track
                            .keys
                            .iter()
                            .map(|key| glam::Vec3::new(key.values[0], key.values[1], key.values[2]))
                            .collect();
                        tracks.push(PropertyTrack {
                            target: format!("track_{:03}", track.track),
                            channel: TrackChannel::Translation { times, values },
                            interpolation: Interpolation::Linear,
                        });
                    }
                    _ => {}
                }
            }
            AnimationClip {
                id: ClipId(clip.index as u32),
                name: format!("clip_{:02}", clip.index),
                duration: clip.duration_s,
                tracks,
                source_rate: Some(SourceRate {
                    numerator: 30,
                    denominator: 1,
                }),
                markers: Vec::new(),
                provenance: format!("Bully AGR variant {}", file.variant),
            }
        })
        .collect();
    AnimationLibrary {
        name: name.into(),
        clips,
        provenance: "Bully AGR (PC, experimental reader)".to_string(),
    }
}

/// How NIF scene nodes are named for AGR track binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NifMapping {
    /// Every scene-graph node counts (shapes included). Retained for
    /// comparison; it lets AGR tracks rotate geometry leaves independently,
    /// which splits rigid assemblies (verified visually on SK8Board.nif).
    AllNodes,
    /// Only bones count; shapes keep `shape_{n}` names and follow their
    /// owning node rigidly. Verified correct on the skateboard asset and
    /// matches the engine convention where animation targets are bones.
    #[default]
    BonesOnly,
}

/// Build a runtime [`ModelAsset`] from a parsed Bully NIF.
///
/// First-pass rig: the node hierarchy and bind-local geometry come from the
/// NIF; skin instances are not decoded yet, so shapes follow their owning
/// node rigidly (segmented-puppet animation). Nodes are named `track_{id}`
/// in scene-graph DFS order so AGR tracks bind by index — the mapping is a
/// trial assumption that the viewer makes visible immediately.
pub fn model_from_nif(
    nif: &NifFile,
    name: impl Into<String>,
    source_identity: impl Into<String>,
) -> Result<ModelAsset, String> {
    model_from_nif_with_mapping(nif, name, source_identity, NifMapping::default())
}

/// [`model_from_nif`] with an explicit node-naming strategy.
pub fn model_from_nif_with_mapping(
    nif: &NifFile,
    name: impl Into<String>,
    source_identity: impl Into<String>,
    mapping: NifMapping,
) -> Result<ModelAsset, String> {
    let mut nodes: Vec<SceneNode> = Vec::new();
    let mut meshes: Vec<MeshAsset> = Vec::new();
    let mut diagnostics: Vec<String> = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut track_counter = 0u32;

    for &root in &nif.footer.roots {
        visit_nif_block(
            nif,
            root,
            None,
            &mut nodes,
            &mut meshes,
            &mut diagnostics,
            &mut visited,
            mapping,
            &mut track_counter,
        );
    }
    if nodes.is_empty() {
        return Err("the NIF has no scene-graph nodes".to_string());
    }
    diagnostics.push(format!("{} nodes, {} meshes", nodes.len(), meshes.len()));

    let mut asset = ModelAsset::new(
        name.into(),
        source_identity.into(),
        nodes,
        meshes,
        BaseOrientation::Zup.to_yup_matrix(),
        BaseOrientation::Zup,
        Some(NodeId(0)),
    )
    .map_err(|error| format!("model admission: {error}"))?;
    asset.diagnostics.extend(diagnostics);
    Ok(asset)
}

#[allow(clippy::too_many_arguments)]
fn visit_nif_block(
    nif: &NifFile,
    block_index: i32,
    parent: Option<NodeId>,
    nodes: &mut Vec<SceneNode>,
    meshes: &mut Vec<MeshAsset>,
    diagnostics: &mut Vec<String>,
    visited: &mut std::collections::HashSet<i32>,
    mapping: NifMapping,
    track_counter: &mut u32,
) -> Option<NodeId> {
    if block_index < 0 || !visited.insert(block_index) {
        return None;
    }
    let block = nif.blocks.get(block_index as usize)?;
    let payload = nif.payloads.get(block_index as usize)?.as_ref()?;
    let id = NodeId(nodes.len() as u32);
    let (local, mesh, children): (NodeTransform, Option<usize>, Vec<i32>) = match payload {
        BlockPayload::NiNode(data) => (
            nif_local(&data.translation, &data.rotation.m, data.scale),
            None,
            data.children.clone(),
        ),
        BlockPayload::NiTriShape(data) => {
            let mesh_index = build_mesh_from_shape(
                nif,
                data.data_ref,
                data.name.as_deref().unwrap_or(&block.type_name),
                meshes,
                diagnostics,
            );
            (
                nif_local(&data.translation, &data.rotation.m, data.scale),
                mesh_index,
                Vec::new(),
            )
        }
        BlockPayload::NiTriStrips(data) => {
            let mesh_index = build_mesh_from_shape(
                nif,
                data.base.data_ref,
                data.base.name.as_deref().unwrap_or(&block.type_name),
                meshes,
                diagnostics,
            );
            (
                nif_local(
                    &data.base.translation,
                    &data.base.rotation.m,
                    data.base.scale,
                ),
                mesh_index,
                Vec::new(),
            )
        }
        _ => return None,
    };

    let original_name = match payload {
        BlockPayload::NiNode(data) => data.name.clone(),
        BlockPayload::NiTriShape(data) => data.name.clone(),
        BlockPayload::NiTriStrips(data) => data.base.name.clone(),
        _ => None,
    };
    let track_name = if mapping == NifMapping::BonesOnly && mesh.is_some() {
        format!("shape_{:03}", id.0)
    } else {
        let name = format!("track_{:03}", *track_counter);
        *track_counter += 1;
        name
    };
    if let Some(original) = &original_name
        && *original != track_name
    {
        // Keep the original identity visible for diagnostics; the binding
        // identity stays the index-based track name.
        if diagnostics.len() < 64 {
            diagnostics.push(format!("{track_name} = {original}"));
        }
    }

    nodes.push(SceneNode {
        id,
        parent,
        name: track_name,
        local,
        mesh,
    });
    for child in children {
        visit_nif_block(
            nif,
            child,
            Some(id),
            nodes,
            meshes,
            diagnostics,
            visited,
            mapping,
            track_counter,
        );
    }
    Some(id)
}

fn nif_local(
    translation: &crate::inspector::nif::Vector3,
    rotation: &[[f32; 3]; 3],
    scale: f32,
) -> NodeTransform {
    // `rotation` is stored row-major for column-vector products; glam wants
    // column arrays, so transpose while copying. Scale is uniform in NIF.
    let matrix = Mat3::from_cols_array(&[
        rotation[0][0],
        rotation[1][0],
        rotation[2][0],
        rotation[0][1],
        rotation[1][1],
        rotation[2][1],
        rotation[0][2],
        rotation[1][2],
        rotation[2][2],
    ]);
    NodeTransform {
        translation: Vec3::new(translation.x, translation.y, translation.z),
        rotation: Quat::from_mat3(&matrix).normalize(),
        scale: Vec3::splat(scale),
    }
}

fn build_mesh_from_shape(
    nif: &NifFile,
    data_ref: i32,
    name: &str,
    meshes: &mut Vec<MeshAsset>,
    diagnostics: &mut Vec<String>,
) -> Option<usize> {
    if data_ref < 0 {
        return None;
    }
    let data = match nif.payloads.get(data_ref as usize)?.as_ref()? {
        BlockPayload::NiTriShapeData(data) => data,
        BlockPayload::NiTriStripsData(_) => {
            diagnostics.push(format!(
                "mesh '{name}': triangle strips are not yet triangulated"
            ));
            return None;
        }
        _ => return None,
    };
    let mut vertices = Vec::with_capacity(data.vertices.len());
    for (index, position) in data.vertices.iter().enumerate() {
        let normal = data.normals.get(index).copied().unwrap_or_default();
        let uv = data
            .uvs
            .get(index)
            .map(|uv| [uv.u, uv.v])
            .unwrap_or([0.0, 0.0]);
        vertices.push(Vertex {
            position: [position.x, position.y, position.z],
            normal: [normal.x, normal.y, normal.z],
            uv,
        });
    }
    let indices: Vec<u32> = data
        .triangles
        .iter()
        .flat_map(|triangle| [triangle.v0 as u32, triangle.v1 as u32, triangle.v2 as u32])
        .collect();
    if vertices.is_empty() || indices.is_empty() {
        return None;
    }
    let index = meshes.len();
    meshes.push(MeshAsset {
        name: name.to_string(),
        texture_name: None,
        vertices,
        indices,
        diffuse: None,
        skin: None,
    });
    Some(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_u32(out: &mut Vec<u8>, value: u32) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    fn push_f32(out: &mut Vec<u8>, value: f32) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    fn push_record(out: &mut Vec<u8>, time: u8, header: u8, values: [i16; 3]) {
        out.push(time);
        out.push(header);
        for value in values {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }

    /// Synthetic 2-clip file with invented data (never game payloads).
    fn fixture() -> Vec<u8> {
        let mut out = Vec::new();
        // Clip 0: one preamble default (rotation track 0), two changes.
        push_u32(&mut out, AGR_MAGIC);
        push_u32(&mut out, 1002);
        push_u32(&mut out, 2); // change records
        push_u32(&mut out, 0);
        push_f32(&mut out, 1.0); // duration
        push_record(&mut out, 0, 0x00, [0, 0, -64]); // preamble default
        push_record(&mut out, 10, 0x00, [0, 0, -64]); // t=0.10s identity
        push_record(&mut out, 20, 0x00, [16384, 0, 0]); // t=0.20s 90° X-ish
        // Clip 1: preamble + two rotation changes (one wrap) + translation.
        push_u32(&mut out, AGR_MAGIC);
        push_u32(&mut out, 1002);
        push_u32(&mut out, 3);
        push_u32(&mut out, 0);
        push_f32(&mut out, 6.0);
        push_record(&mut out, 0, 0x00, [0, 0, -64]);
        push_record(&mut out, 250, 0x00, [0, 0, -64]); // t=2.50s
        push_record(&mut out, 5, 0x00, [0, 0, -64]); // wraps -> t=2.61s
        // Translation record; the last word stays non-zero so the tail
        // padding trim cannot mistake the file end for archive padding.
        push_record(&mut out, 7, 0x01, [100, 0, 1]);
        out
    }

    #[test]
    fn parses_synthetic_fixture() {
        let file = parse_agr(&fixture()).expect("fixture parses");
        assert_eq!(file.variant, 1002);
        assert_eq!(file.clip_count(), 2);
        let clip0 = &file.clips[0];
        assert_eq!(clip0.preamble_records, 1);
        assert!((clip0.duration_s - 1.0).abs() < 1e-6);
        let rotation = clip0
            .tracks
            .iter()
            .find(|t| t.track == 0 && t.channel == 0)
            .expect("rotation track");
        assert_eq!(rotation.keys.len(), 2);
        assert!((rotation.keys[0].time_s - 0.10).abs() < 1e-6);
        assert!((rotation.keys[1].time_s - 0.20).abs() < 1e-6);
        // (16384, 0, 0)/32767 -> x = 0.5, w = sqrt(1-0.25) ~ 0.866.
        let q = rotation.keys[1].rotation();
        assert!((q.x - 0.5).abs() < 1e-4);
        assert!((q.w - 0.8660).abs() < 1e-3);
    }

    #[test]
    fn wrap_reconstruction_keeps_keys_ascending() {
        let file = parse_agr(&fixture()).expect("fixture parses");
        let clip1 = &file.clips[1];
        let rotation = clip1
            .tracks
            .iter()
            .find(|t| t.track == 0 && t.channel == 0)
            .expect("rotation track");
        assert_eq!(rotation.keys.len(), 2);
        assert!((rotation.keys[0].time_s - 2.50).abs() < 1e-4);
        assert!((rotation.keys[1].time_s - 2.61).abs() < 1e-4);
        assert!(rotation.keys[0].time_s < rotation.keys[1].time_s);
    }

    #[test]
    fn translation_channels_are_emitted_as_positions() {
        let file = parse_agr(&fixture()).expect("fixture parses");
        let clip1 = &file.clips[1];
        assert!(clip1.tracks.iter().any(|t| t.channel == 1));
        let library = to_library(&file, "test");
        let clip = library.clips.iter().find(|c| c.name == "clip_01").unwrap();
        let translation = clip
            .tracks
            .iter()
            .find(|t| matches!(t.channel, TrackChannel::Translation { .. }))
            .expect("translation emitted");
        if let TrackChannel::Translation { values, .. } = &translation.channel {
            // [100, 0, 1] / 32767.
            assert!((values[0].x - 100.0 / 32767.0).abs() < 1e-6);
        }
    }

    #[test]
    fn library_clips_validate() {
        let file = parse_agr(&fixture()).expect("fixture parses");
        let library = to_library(&file, "test");
        for clip in &library.clips {
            clip.validate().expect("converted clips must validate");
        }
    }

    #[test]
    fn malformed_inputs_are_typed_errors() {
        assert!(matches!(
            parse_agr(&[0u8; 4]),
            Err(AgrError::TooShort { .. })
        ));
        let mut bad_magic = fixture();
        bad_magic[0] = 0xFF;
        assert!(matches!(
            parse_agr(&bad_magic),
            Err(AgrError::BadMagic { .. })
        ));
        let mut bad_variant = fixture();
        bad_variant[4..8].copy_from_slice(&77u32.to_le_bytes());
        assert!(matches!(
            parse_agr(&bad_variant),
            Err(AgrError::BadVariant { .. })
        ));
    }

    #[test]
    fn trailer_is_stripped() {
        let mut data = fixture();
        // Append a 4-byte trailer to the last chunk.
        data.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);
        let file = parse_agr(&data).expect("trailer tolerated");
        assert!(
            file.clips
                .last()
                .is_some_and(|c| c.diagnostics.iter().any(|d| d.contains("trailer")))
        );
    }

    #[test]
    fn nif_model_builds_when_available() {
        let Some(root) = crate::test_paths::bully_nif_tools() else {
            return;
        };
        // Prefer a character model; fall back to any parseable NIF.
        let mut candidates: Vec<std::path::PathBuf> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("nif"))
                    && path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .is_some_and(|stem| stem.eq_ignore_ascii_case("player"))
                {
                    candidates.push(path);
                }
            }
        }
        if candidates.is_empty() {
            return;
        }
        let bytes = std::fs::read(&candidates[0]).expect("read player NIF");
        let mut nif = NifFile::parse(&bytes).expect("parse player NIF");
        nif.resolve_string_indices();
        let model = model_from_nif(&nif, "PLAYER", "test:player").expect("model builds");
        assert!(model.nodes.len() > 1, "player NIF has a skeleton");
        assert!(!model.meshes.is_empty(), "player NIF has geometry");
        assert!(model.node_by_name("track_000").is_some());
        // Shapes must not carry AGR track identities: animating a geometry
        // leaf independently splits rigid assemblies (SK8Board regression).
        for node in &model.nodes {
            if node.mesh.is_some() {
                assert!(
                    !node.name.starts_with("track_"),
                    "shape node {} must not be a track target",
                    node.name
                );
            }
        }
    }

    /// Developer dump: set `IMGEDITOR_AGR_DUMP_AGR` and
    /// `IMGEDITOR_AGR_DUMP_NIF` to a sample pair, then run with
    /// `--nocapture` to print the model node tree, AGR tracks, and the
    /// name-based binding report. Used to tune the track-id mapping and
    /// rig alignment without the GUI.
    #[test]
    fn dump_agr_pair_when_requested() {
        let (Ok(agr_path), Ok(nif_path)) = (
            std::env::var("IMGEDITOR_AGR_DUMP_AGR"),
            std::env::var("IMGEDITOR_AGR_DUMP_NIF"),
        ) else {
            return;
        };
        let agr_bytes = std::fs::read(&agr_path).expect("read AGR");
        let nif_bytes = std::fs::read(&nif_path).expect("read NIF");
        let mut nif = NifFile::parse(&nif_bytes).expect("parse NIF");
        nif.resolve_string_indices();
        let model = model_from_nif(&nif, "dump", "dump").expect("model builds");
        println!(
            "== model: {} nodes, {} meshes ==",
            model.nodes.len(),
            model.meshes.len()
        );
        for node in &model.nodes {
            println!(
                "  node {:<8} parent={:<8} mesh={:?}",
                node.name,
                node.parent.map(|p| p.0).unwrap_or(u32::MAX),
                node.mesh
            );
        }
        for line in model.diagnostics.iter().take(48) {
            println!("  name: {line}");
        }
        let file = parse_agr(&agr_bytes).expect("parse AGR");
        // Rotation convention check: compare each bound node's NIF rest
        // quaternion against the clip's first rotation key. Near-identity
        // first keys mean the data is a delta over rest; keys matching rest
        // mean absolute local rotations.
        for clip in file.clips.iter().take(3) {
            println!(
                "  -- clip_{:02} convention check ({} tracks) --",
                clip.index,
                clip.tracks.len()
            );
            for track in clip.tracks.iter().filter(|t| t.channel == 0) {
                let target = format!("track_{:03}", track.track);
                let Some(node) = model.node_by_name(&target) else {
                    continue;
                };
                let Some(first) = track.keys.first() else {
                    continue;
                };
                let key = first.rotation();
                let rest = node.local.rotation;
                let dot = rest.dot(key).abs().clamp(0.0, 1.0);
                let delta_deg = 2.0 * dot.acos().to_degrees();
                println!(
                    "     {target}: rest={:.1}deg key={:.1}deg delta={:.1}deg t={:.3}",
                    rest.to_axis_angle().1.to_degrees(),
                    key.to_axis_angle().1.to_degrees(),
                    delta_deg,
                    first.time_s,
                );
            }
            for track in clip.tracks.iter().filter(|t| t.channel == 1) {
                let target = format!("track_{:03}", track.track);
                let Some(node) = model.node_by_name(&target) else {
                    continue;
                };
                let samples: Vec<String> = track
                    .keys
                    .iter()
                    .take(4)
                    .map(|key| {
                        format!(
                            "t={:.2} ({},{},{})",
                            key.time_s, key.values[0], key.values[1], key.values[2]
                        )
                    })
                    .collect();
                println!(
                    "     {target} ch1: rest_translation=({:.3},{:.3},{:.3}) {}",
                    node.local.translation.x,
                    node.local.translation.y,
                    node.local.translation.z,
                    samples.join(" | ")
                );
            }
        }
        println!(
            "== AGR variant {} clips {} ==",
            file.variant,
            file.clip_count()
        );
        for clip in file.clips.iter().take(24) {
            println!(
                "  clip {:02}: variant={} dur={:.3} tracks={} preamble={} diag={:?}",
                clip.index,
                clip.variant,
                clip.duration_s,
                clip.tracks.len(),
                clip.preamble_records,
                clip.diagnostics
            );
            for track in clip.tracks.iter().take(24) {
                let target = format!("track_{:03}", track.track);
                let bound = model.node_by_name(&target).is_some();
                println!(
                    "     track {:3} ch{} keys={:3} first_t={:.3} last_t={:.3} -> {} {}",
                    track.track,
                    track.channel,
                    track.keys.len(),
                    track.keys.first().map(|k| k.time_s).unwrap_or(0.0),
                    track.keys.last().map(|k| k.time_s).unwrap_or(0.0),
                    target,
                    if bound { "BOUND" } else { "unbound" }
                );
            }
        }
    }

    #[test]
    fn real_agr_parses_when_available() {
        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let anim = std::path::Path::new(&stream).join("Anim");
        // (name, expected clip count) — loose corpus, PC retail.
        let expected = [
            ("C_Player.agr", 436usize),
            ("Grap.agr", 59),
            ("MOT_CTRL.agr", 386),
            ("NPC_Cher.agr", 12),
        ];
        for (name, clips) in expected {
            let Ok(bytes) = std::fs::read(anim.join(name)) else {
                continue;
            };
            let file = parse_agr(&bytes).unwrap_or_else(|error| {
                panic!("{name} must parse: {error}");
            });
            assert_eq!(file.variant, 1002, "{name} variant");
            assert_eq!(file.clip_count(), clips, "{name} clip count");
            let library = to_library(&file, name);
            for clip in &library.clips {
                clip.validate()
                    .unwrap_or_else(|error| panic!("{name} clip must validate: {error}"));
            }
        }
    }

    /// Synthetic 999/1003/1004 chunks (invented data, never game payloads).
    fn object_fixture() -> Vec<u8> {
        let mut out = Vec::new();
        // 999 clip: 3 defaults + 2 keys; w-first quat, metres translation.
        push_u32(&mut out, AGR_MAGIC);
        push_u32(&mut out, 999);
        push_u32(&mut out, 5);
        push_u32(&mut out, 0);
        push_f32(&mut out, 0.5);
        for ordinal in [0u16, 0, 0] {
            out.extend_from_slice(&ordinal.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            for value in [1.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0] {
                push_f32(&mut out, value);
            }
        }
        for (ordinal, time_norm, w, x) in
            [(1u16, 32768u16, 0.7071f32, 0.7071f32), (2, 65535, 1.0, 0.0)]
        {
            out.extend_from_slice(&ordinal.to_le_bytes());
            out.extend_from_slice(&time_norm.to_le_bytes());
            for value in [w, x, 0.0, 0.0, 0.1, 0.2, 0.3] {
                push_f32(&mut out, value);
            }
        }
        // 1003 clip: 3 defaults + 1 key (compact i16 quat + translation).
        push_u32(&mut out, AGR_MAGIC);
        push_u32(&mut out, 1003);
        push_u32(&mut out, 4);
        push_u32(&mut out, 0);
        push_f32(&mut out, 1.0);
        for _ in 0..3 {
            for _ in 0..10 {
                out.extend_from_slice(&0u16.to_le_bytes());
            }
        }
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&65535u16.to_le_bytes());
        for value in [0i16, 16384, 0, 28361, 0, 0, 100] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&0u16.to_le_bytes());
        // 1004 clip: 1 default + 2 changes on tracks 0 and 1.
        push_u32(&mut out, AGR_MAGIC);
        push_u32(&mut out, 1004);
        push_u32(&mut out, 3);
        push_u32(&mut out, 0);
        push_f32(&mut out, 0.2);
        out.extend_from_slice(&[0, 0x00]);
        for value in [0i16, 0, -64] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        push_u32(&mut out, 0);
        out.extend_from_slice(&[3, 0x00]);
        for value in [0i16, 0, -64] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        push_u32(&mut out, 0);
        out.extend_from_slice(&[6, 0x09]); // track 1, channel 1
        for value in [100i16, 0, 1] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        push_u32(&mut out, 7);
        out
    }

    #[test]
    fn object_variants_decode_into_tracks() {
        let file = parse_agr(&object_fixture()).expect("fixture parses");
        assert_eq!(file.clip_count(), 3);

        // 999: defaults skipped, two rotation + two translation keys.
        let clip0 = &file.clips[0];
        assert_eq!(clip0.variant, 999);
        let rotation = clip0
            .tracks
            .iter()
            .find(|t| t.channel == 0)
            .expect("999 rotation track");
        assert_eq!(rotation.keys.len(), 2);
        assert!((rotation.keys[0].time_s - 0.25).abs() < 1e-3);
        assert!((rotation.keys[1].time_s - 0.5).abs() < 1e-3);
        let q = rotation.keys[0].rotation();
        assert!((q.x - 0.7071).abs() < 1e-3);
        assert!((q.y - 0.0).abs() < 1e-3);
        let translation = clip0
            .tracks
            .iter()
            .find(|t| t.channel == 1)
            .expect("999 translation track");
        assert!((translation.keys[0].values[1] - 0.2).abs() < 1e-5);

        // 1003: one key, i16 quat 16384/32767 -> x = 0.5.
        let clip1 = &file.clips[1];
        assert_eq!(clip1.variant, 1003);
        let rotation = clip1
            .tracks
            .iter()
            .find(|t| t.channel == 0)
            .expect("1003 rotation track");
        assert_eq!(rotation.keys.len(), 1);
        assert!((rotation.keys[0].time_s - 1.0).abs() < 1e-4);
        assert!((rotation.keys[0].values[1] - 0.5).abs() < 1e-3);

        // 1004: change stream keeps its track ids and channels.
        let clip2 = &file.clips[2];
        assert_eq!(clip2.variant, 1004);
        let rot0 = clip2
            .tracks
            .iter()
            .find(|t| t.track == 0 && t.channel == 0)
            .expect("1004 track 0 rotation");
        assert!((rot0.keys[0].time_s - 0.1).abs() < 1e-4);
        let trans1 = clip2
            .tracks
            .iter()
            .find(|t| t.track == 1 && t.channel == 1)
            .expect("1004 track 1 translation");
        assert!((trans1.keys[0].time_s - 0.2).abs() < 1e-4);
    }

    /// Extract one named entry from the retail World.img using its .dir
    /// sidecar (no full-archive read).
    fn world_entry(stream: &std::path::Path, name: &str) -> Option<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        let dir = std::fs::read(stream.join("World.dir")).ok()?;
        for record in dir.chunks_exact(32) {
            let end = record[8..].iter().position(|b| *b == 0).unwrap_or(24);
            let entry = String::from_utf8_lossy(&record[8..8 + end]);
            if !entry.eq_ignore_ascii_case(name) {
                continue;
            }
            let offset = u32::from_le_bytes(record[0..4].try_into().ok()?) as u64 * 2048;
            let size = u32::from_le_bytes(record[4..8].try_into().ok()?) as usize * 2048;
            let mut file = std::fs::File::open(stream.join("World.img")).ok()?;
            file.seek(SeekFrom::Start(offset)).ok()?;
            let mut bytes = vec![0u8; size];
            file.read_exact(&mut bytes).ok()?;
            return Some(bytes);
        }
        None
    }

    #[test]
    fn object_variant_corpus_decodes_when_available() {
        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let stream = std::path::Path::new(&stream);

        // SK8Board catalog: 17 clips; the OLLIE clip is 999, GIV/EXAMINE 1004.
        if let Some(bytes) = world_entry(stream, "SK8Board.agr") {
            let file = parse_agr(&bytes).expect("SK8Board.agr parses");
            assert_eq!(file.clip_count(), 17);
            for clip in &file.clips {
                assert!(
                    !clip.tracks.is_empty(),
                    "clip {:02} (variant {}) must decode tracks",
                    clip.index,
                    clip.variant
                );
                assert!(
                    !clip
                        .diagnostics
                        .iter()
                        .any(|d| d.contains("not yet decoded")),
                    "clip {:02} diagnostics: {:?}",
                    clip.index,
                    clip.diagnostics
                );
            }
            let pickup = file
                .clips
                .iter()
                .find(|c| c.variant == 999 && c.duration_s > 2.0)
                .expect("PICKUP clip");
            let rotation = pickup
                .tracks
                .iter()
                .find(|t| t.channel == 0)
                .expect("rotation track");
            assert!(rotation.keys.len() > 40, "PICKUP key count");
            assert!(
                (rotation.keys.last().unwrap().time_s - 2.3333).abs() < 0.01,
                "PICKUP spans its duration"
            );
            // Keys stay strictly ascending (the clip legitimately holds its
            // start pose for ~0.7 s, so gaps are expected) and quats stay
            // unit even after decoding.
            for pair in rotation.keys.windows(2) {
                assert!(pair[1].time_s > pair[0].time_s, "keys ascend");
                let q = pair[0].rotation();
                assert!((q.length() - 1.0).abs() < 1e-3, "unit quat");
            }
        }

        // AniBroom: three 1003 clips; the two sweeps have four keys each.
        if let Some(bytes) = world_entry(stream, "AniBroom.agr") {
            let file = parse_agr(&bytes).expect("AniBroom.agr parses");
            assert_eq!(file.clip_count(), 3);
            assert!(file.clips.iter().all(|c| c.variant == 1003));
            let sweep = &file.clips[1];
            let rotation = sweep
                .tracks
                .iter()
                .find(|t| t.channel == 0)
                .expect("rotation track");
            assert_eq!(rotation.keys.len(), 4);
            assert!((rotation.keys.last().unwrap().time_s - 0.667).abs() < 0.01);
            for key in &rotation.keys {
                let q = key.rotation();
                assert!((q.length() - 1.0).abs() < 1e-3, "unit quat");
            }
        }
    }
}
