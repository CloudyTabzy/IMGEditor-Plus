//! Bully (PC) AGR reader — experimental, evidence-based.
//!
//! Reverse-engineered from the retail PC corpus by the local probe suite
//! (`bully-probe/`, outside the repository). The container and record
//! structure is corpus-validated (554 files, 3,261 clips, zero structural
//! errors); the time encoding is still under trial-and-error and is
//! implemented with explicit, documented heuristics.
//!
//! Validated layout:
//!
//! ```text
//! file    = chunk*
//! chunk   = { u32 magic=0x100, u32 variant ∈ 999..=1004,
//!             u32 record_count, u32 0, f32 duration_s }
//!           + preamble: P records   (P = data_bytes/8 - record_count)
//!           + changes : record_count records
//!           + optional 4-byte trailer when data_bytes % 8 == 4
//! record  = { u8 time_lo, u8 header, 3 × i16 }
//! header  = (track_id << 3) | channel   (channel 0/1/2 observed)
//! ```
//!
//! Rotation channels store Gamebryo compact quaternions: `(x, y, z)` at
//! scale 1/32767 with `w = sqrt(1 - |v|²)` reconstructed. Time is a u8
//! that wraps; within one track's records it is mostly ascending, with
//! occasional out-of-order records still under investigation. The parser
//! treats a decrease larger than half the range as a wrap and drops smaller
//! reversals as diagnostics, keeping key times strictly ascending for the
//! runtime's pose sampler.

use std::ops::RangeInclusive;

use glam::Quat;

use crate::inspector::animation::ClipId;
use crate::inspector::animation::clip::{
    AnimationClip, AnimationLibrary, Interpolation, PropertyTrack, SourceRate, TrackChannel,
};

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
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AgrKey {
    pub time_s: f32,
    pub values: [i16; 3],
}

impl AgrKey {
    /// Interpret the key as a compact Gamebryo rotation.
    pub fn rotation(&self) -> Quat {
        let x = self.values[0] as f32 / COMPACT_QUAT_SCALE;
        let y = self.values[1] as f32 / COMPACT_QUAT_SCALE;
        let z = self.values[2] as f32 / COMPACT_QUAT_SCALE;
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
    pub duration_s: f32,
    /// Records before the declared change records (per-track defaults and
    /// other preamble entries).
    pub preamble_records: usize,
    pub tracks: Vec<AgrTrack>,
    /// Parse notes: dropped out-of-order or duplicate keys, trailer, etc.
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

/// Find every chunk start: u32 magic followed by the variant word.
fn chunk_starts(bytes: &[u8], variant: u32) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut offset = 0usize;
    while offset + CHUNK_HEADER_BYTES <= bytes.len() {
        if read_u32(bytes, offset) == AGR_MAGIC && read_u32(bytes, offset + 4) == variant {
            starts.push(offset);
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

    let starts = chunk_starts(bytes, variant);
    if starts.first() != Some(&0) {
        return Err(AgrError::BadMagic {
            offset: 0,
            word: magic,
            expected: AGR_MAGIC,
        });
    }

    let mut clips = Vec::with_capacity(starts.len());
    for (index, &start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(bytes.len());
        clips.push(parse_clip(bytes, start, end, index)?);
    }
    Ok(AgrFile { variant, clips })
}

fn parse_clip(bytes: &[u8], start: usize, end: usize, index: usize) -> Result<AgrClip, AgrError> {
    let count = read_u32(bytes, start + 8) as usize;
    let duration = read_f32(bytes, start + 16);
    if !duration.is_finite() || duration <= 0.0 || duration > 600.0 {
        return Err(AgrError::BadDuration {
            offset: start,
            duration,
        });
    }
    let mut data_bytes = end.saturating_sub(start + CHUNK_HEADER_BYTES);
    let mut diagnostics = Vec::new();
    if data_bytes % RECORD_BYTES == 4 {
        data_bytes -= 4;
        diagnostics.push("stripped 4-byte trailer".to_string());
    }
    if !data_bytes.is_multiple_of(RECORD_BYTES) {
        return Err(AgrError::MisalignedData {
            offset: start,
            data_bytes,
        });
    }
    let slots = data_bytes / RECORD_BYTES;
    if count > slots {
        return Err(AgrError::RecordOverflow {
            offset: start,
            count,
            slots,
        });
    }
    let preamble_records = slots - count;
    let body = &bytes[start + CHUNK_HEADER_BYTES..start + CHUNK_HEADER_BYTES + data_bytes];

    // Group change records per (track, channel), keeping stream order.
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
            i16::from_le_bytes([record[2], record[3]]),
            i16::from_le_bytes([record[4], record[5]]),
            i16::from_le_bytes([record[6], record[7]]),
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

    Ok(AgrClip {
        index,
        duration_s: duration,
        preamble_records,
        tracks: grouped,
        diagnostics,
    })
}

/// Convert a parsed file into a runtime [`AnimationLibrary`].
///
/// Only rotation channels (0) become tracks today; channels 1/2 are read
/// but not emitted until their units are established. Track targets are
/// `track_{id}` so a future NIF adapter can map ids to bone names.
pub fn to_library(file: &AgrFile, name: impl Into<String>) -> AnimationLibrary {
    let clips = file
        .clips
        .iter()
        .map(|clip| {
            let mut tracks = Vec::new();
            for track in &clip.tracks {
                if track.channel != 0 {
                    continue;
                }
                let times: Vec<f32> = track.keys.iter().map(|key| key.time_s).collect();
                let values: Vec<Quat> = track.keys.iter().map(|key| key.rotation()).collect();
                tracks.push(PropertyTrack {
                    target: format!("track_{:03}", track.track),
                    channel: TrackChannel::Rotation { times, values },
                    interpolation: Interpolation::Linear,
                });
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
        push_record(&mut out, 7, 0x01, [100, 0, 0]); // translation, not emitted
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
    fn translation_channels_are_read_but_not_emitted() {
        let file = parse_agr(&fixture()).expect("fixture parses");
        let clip1 = &file.clips[1];
        assert!(clip1.tracks.iter().any(|t| t.channel == 1));
        let library = to_library(&file, "test");
        assert!(
            library
                .clips
                .iter()
                .flat_map(|c| &c.tracks)
                .all(|t| matches!(t.channel, TrackChannel::Rotation { .. }))
        );
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
}
