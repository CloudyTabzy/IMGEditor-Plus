//! Bully (PC) AGR reader — experimental, evidence-based.
//!
//! Reverse-engineered from the retail PC corpus by the local probe suite
//! (`bully-probe/`, outside the repository). The container and all six
//! record variants are corpus-validated (554 files, 3,261 clips, zero
//! structural errors).
//!
//! Validated layout:
//!
//! ```text
//! file    = chunk*
//! chunk   = { u32 magic=0x100, u32 variant ∈ 999..=1004,
//!             u32 record_count, u32 0, f32 duration_s }
//!           + record_count records
//!           + variant-specific post-key data
//!           + optional 4-byte trailer/padding
//! ```
//!
//! Record layouts:
//!
//! - **1002** (character transform stream, 8 B): the first two little-endian
//!   words use the same packed predecessor/time/quaternion layout as the first
//!   two words of 1004. The low 11 bits are a predecessor record index, the
//!   next 9 bits are a normalized time code, and the remaining bits contain a
//!   signed-magnitude quaternion. Records form one linked rotation curve per
//!   animated transform. The declared records precede a runtime auxiliary
//!   section; the auxiliary records are not animation keys.
//! - **999** (object float records, 32 B):
//!   `{ u16 ordinal, u16 time_norm, f32 w, x, y, z, f32 tx, ty, tz }`.
//!   `time_norm / 65535 * duration` lands on exact 30 fps frames (verified
//!   against ANIBALL). Ordinal-zero records are chunk defaults.
//! - **1000** (linked float rotation records, 20 B):
//!   `{ u16 previous, u16 time_norm, f32 w, x, y, z }`, followed by sparse
//!   translation records `{ u32 record_index, f32 tx, ty, tz }`. The
//!   predecessor forest identifies curves; translation records use the
//!   referenced rotation key's time. Quaternion components are full-float and
//!   time uses `time_norm / 65535`.
//! - **1001** (linked compact rotation records, 12 B):
//!   `{ u16 previous, u16 time_norm, i16 x, y, z, w }`, followed by sparse
//!   translation records `{ u16 record_index, i16 tx, ty, tz }`. Quaternion
//!   components use `1 / 32767`; translation components use `1 / 1000`.
//!   These records share the linked-curve structure of 1002/1004.
//! - **1003** (object compact records, 20 B):
//!   `{ u16 ordinal, u16 time_norm, i16 x, y, z, w, i16 tx, ty, tz, u16 }`.
//!   Same time rule; values at 1/32767.
//! - **1004** (object transform stream, 12 B): three packed little-endian
//!   words. The first word contains an 11-bit predecessor index and a 9-bit
//!   normalized time code. The next two words contain signed-magnitude
//!   quaternion and translation components. Records form one linked curve per
//!   animated transform; the first root is the default pose and the final
//!   identity curve is a runtime sentinel.
//!
//! The runtime-facing key space is normalized: rotation keys are unit-quat
//! `(x, y, z)` components, translation keys are metres.

use std::ops::RangeInclusive;

use glam::{Mat3, Mat4, Quat, Vec3};

use crate::inspector::animation::ClipId;
use crate::inspector::animation::NodeId;
use crate::inspector::animation::clip::{
    AnimationClip, AnimationLibrary, Interpolation, PropertyTrack, SourceRate, TrackChannel,
};
use crate::inspector::animation::model::{
    MeshAsset, ModelAsset, NodeTransform, SceneNode, SkinBinding, VertexSkin,
};
use crate::inspector::nif::{BlockPayload, NifFile};
use crate::inspector::scene3d::camera::BaseOrientation;
use crate::inspector::scene3d::mesh::Vertex;

/// First u32 of every chunk.
pub const AGR_MAGIC: u32 = 0x0100;

/// Accepted chunk variant words (second u32).
pub const AGR_VARIANTS: RangeInclusive<u32> = 999..=1004;

const CHUNK_HEADER_BYTES: usize = 20;
const RECORD_BYTES: usize = 8;
const FLOAT_LINKED_RECORD_BYTES: usize = 20;
const COMPACT_LINKED_RECORD_BYTES: usize = 12;
const COMPACT_QUAT_SCALE: f32 = 32767.0;
const COMPACT_TRANSLATION_SCALE: f32 = 0.001;
const PACKED_QUAT_SCALE: f32 = 1.0 / 1023.0;
const PACKED_1004_TRANSLATION_SCALE: f32 = 0.01;
const FULL_TIME_SCALE: f32 = 1.0 / 65535.0;
const PACKED_TIME_SCALE: f32 = 1.0 / 511.0;
const PACKED_LINK_MASK: u32 = 0x7ff;

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
        "chunk at offset {offset} has {data_bytes} data bytes, not a multiple of its record size"
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
    /// Runtime-facing transform track id. For linked variants 1000, 1001,
    /// 1002 and 1004 this is the predecessor-linked curve root minus one; the
    /// named-rig mapping remains an adapter concern.
    pub track: u8,
    /// 0 = rotation; 1 = translation; 2 is not yet rendered (units unknown).
    pub channel: u8,
    pub keys: Vec<AgrKey>,
}

/// One decoded clip (chunk). Chunk 0 doubles as the file header.
#[derive(Clone, Debug, PartialEq)]
pub struct AgrClip {
    pub index: usize,
    /// Bytes occupied by this source chunk after archive-sector padding is
    /// removed. HXD catalogs store the corresponding value plus a 4-byte
    /// runtime trailer, which provides an exact naming key.
    pub source_size: usize,
    /// Chunk variant word (999..=1004); determines the record format.
    pub variant: u32,
    /// Record size in bytes for this variant (0 when unknown).
    pub record_size: usize,
    pub duration_s: f32,
    /// Post-key records recognized by the decoder: runtime auxiliary records
    /// for variant 1002, or sparse translation records for variants 1000/1001.
    pub auxiliary_records: usize,
    /// Raw 8-byte records after the declared stream of a 1002 chunk. Their
    /// runtime lookup semantics are not established, so they are preserved
    /// for diagnostics/future lossless tooling but never treated as keys.
    pub auxiliary_tail: Vec<[u8; 8]>,
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

    // Do not globally trim zero words here. A packed fixed-size record can
    // legitimately end in zero bytes (especially a 1004 identity key), and
    // trimming before variant-aware admission can silently remove that record.
    // Each fixed-size variant is sized from its declared count below. The
    // variable 1002 stream retains its legacy trailer/padding handling in its
    // own parsing branch.
    let end = bytes.len();
    let starts = chunk_starts(bytes);
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
        clips.push(parse_clip(bytes, start, chunk_end, index)?);
    }
    Ok(AgrFile { variant, clips })
}

/// Record size for a chunk variant, when the layout is established.
///
/// Corpus-validated sizes: 999 -> 32 B, 1000 -> 20 B, 1001 -> 12 B,
/// 1002 -> 8 B (+ runtime auxiliary records), 1003 -> 20 B, and 1004 ->
/// 12 B. Variants 1000/1001 carry additional sparse translation records after
/// their declared rotation stream.
fn variant_record_size(variant: u32) -> Option<usize> {
    match variant {
        999 => Some(32),
        1000 => Some(FLOAT_LINKED_RECORD_BYTES),
        1001 => Some(COMPACT_LINKED_RECORD_BYTES),
        1002 => Some(8),
        1003 => Some(20),
        1004 => Some(12),
        _ => None,
    }
}

fn variant_metadata_record_size(variant: u32) -> Option<usize> {
    match variant {
        1000 => Some(16),
        1001 => Some(8),
        _ => None,
    }
}

/// Find the sparse translation table at the end of a 1000/1001 chunk.
///
/// The table has no count in the file. Retail archive entries are sector
/// padded, and a valid final translation can itself end in zero bytes, so
/// trimming zero bytes is not sufficient. Every real table row has a
/// non-zero rotation-record index in the declared stream; use those indices
/// as the admission boundary and leave padding/trailers outside the logical
/// chunk.
fn fixed_metadata_end(
    bytes: &[u8],
    metadata_start: usize,
    end: usize,
    variant: u32,
    record_count: usize,
    diagnostics: &mut Vec<String>,
) -> (usize, usize) {
    let Some(metadata_size) = variant_metadata_record_size(variant) else {
        return (metadata_start, 0);
    };

    let mut metadata_end = metadata_start;
    let mut metadata_records = 0usize;
    let mut invalid_records = 0usize;
    let mut offset = metadata_start;
    let mut metadata_closed = false;
    while let Some(next) = offset.checked_add(metadata_size).filter(|next| *next <= end) {
        let record = &bytes[offset..next];
        let record_index = if variant == 1000 {
            read_u32(record, 0) as usize
        } else {
            u16::from_le_bytes([record[0], record[1]]) as usize
        };
        let all_zero = record.iter().all(|byte| *byte == 0);
        if !metadata_closed && record_index > 0 && record_index < record_count {
            metadata_records += 1;
            metadata_end = next;
        } else {
            metadata_closed = true;
            if !all_zero {
                // Zero rows are archive padding. Non-zero rows with an
                // invalid index are not promoted to metadata, which keeps
                // random trailer bytes from becoming translation keys. Once
                // either kind of terminator is seen, later rows remain part
                // of the ignored tail.
                invalid_records += 1;
            }
        }
        offset = next;
    }

    let partial_nonzero = bytes[offset..end].iter().any(|byte| *byte != 0);
    if invalid_records > 0 {
        diagnostics.push(format!(
            "ignored {invalid_records} malformed {variant} translation metadata record(s)"
        ));
    }
    if partial_nonzero {
        diagnostics.push(format!(
            "ignored {} non-aligned trailing byte(s) after {variant} translation metadata",
            end.saturating_sub(offset)
        ));
    }
    if metadata_records > 0
        && bytes[metadata_end..end]
            .iter()
            .any(|byte| *byte != 0)
        && !partial_nonzero
    {
        diagnostics.push(format!(
            "ignored {} trailing byte(s) after {variant} translation metadata",
            end.saturating_sub(metadata_end)
        ));
    }
    (metadata_end, metadata_records)
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
            source_size: end.saturating_sub(start),
            variant,
            record_size: 0,
            duration_s: duration,
            auxiliary_records: 0,
            auxiliary_tail: Vec::new(),
            tracks: Vec::new(),
            diagnostics: vec![format!(
                "variant {variant} chunk ({count} records): record layout not yet decoded"
            )],
        });
    };
    let mut diagnostics = Vec::new();
    let data_start = start + CHUNK_HEADER_BYTES;

    // Fixed transform variants have no implicit preamble: their declared count
    // is the exact number of records. This is important for archive entries,
    // whose sector padding can contain any number of zero words, including a
    // valid record-shaped suffix. The bounded slice also tolerates an
    // optional runtime trailer without treating it as a record.
    let (
        auxiliary_records,
        body_end,
        logical_source_size,
        metadata_start,
        metadata_end,
        auxiliary_tail,
    ) = if variant != 1002 {
        let available = end.saturating_sub(data_start);
        let required = count
            .checked_mul(record_size)
            .ok_or(AgrError::RecordOverflow {
                offset: start,
                count,
                slots: available / record_size,
            })?;
        if required > available {
            return Err(AgrError::RecordOverflow {
                offset: start,
                count,
                slots: available / record_size,
            });
        }
        let metadata_start = data_start + required;
        if let Some(_metadata_size) = variant_metadata_record_size(variant) {
            let (metadata_end, metadata_records) = fixed_metadata_end(
                bytes,
                metadata_start,
                end,
                variant,
                count,
                &mut diagnostics,
            );
            (
                metadata_records,
                metadata_start,
                metadata_end.saturating_sub(start),
                metadata_start,
                metadata_end,
                Vec::new(),
            )
        } else {
            let trailing = available - required;
            if trailing > 0
                && bytes[metadata_start..end]
                    .iter()
                    .any(|byte| *byte != 0)
            {
                diagnostics.push(format!(
                    "ignored {trailing} trailing byte(s) after the declared records"
                ));
            }
            (
                0,
                metadata_start,
                CHUNK_HEADER_BYTES + required,
                metadata_start,
                metadata_start,
                Vec::new(),
            )
        }
    } else {
        // Variant 1002 stores the declared animation records first, followed
        // by an auxiliary record section whose length is supplied by the
        // target/model runtime. The auxiliary records use the same 8-byte
        // storage size but are not part of the animation curve forest.
        // Only a sector-aligned archive entry is known to carry zero padding;
        // do not apply that heuristic to an intermediate chunk or a loose AGR
        // file. A four-byte non-record trailer is handled by the alignment
        // rule below for compatibility with existing HXD/runtime extracts.
        let available = end.saturating_sub(data_start);
        let required = count
            .checked_mul(record_size)
            .ok_or(AgrError::RecordOverflow {
                offset: start,
                count,
                slots: available / record_size,
            })?;
        if required > available {
            return Err(AgrError::RecordOverflow {
                offset: start,
                count,
                slots: available / record_size,
            });
        }
        let minimum_end = data_start + required;
        let mut data_end = end;
        if end == bytes.len() && bytes.len().is_multiple_of(2048) {
            let mut padding_trimmed = 0usize;
            while data_end >= 4 && data_end - 4 >= minimum_end && read_u32(bytes, data_end - 4) == 0
            {
                data_end -= 4;
                padding_trimmed += 4;
            }
            if padding_trimmed > 0 {
                diagnostics.push(format!(
                    "trimmed {padding_trimmed} bytes of archive tail padding"
                ));
            }
        }
        let mut data_bytes = data_end.saturating_sub(data_start);
        if data_bytes % record_size == 4 {
            data_bytes -= 4;
            data_end -= 4;
            diagnostics.push("stripped 4-byte trailer".to_string());
        }
        if !data_bytes.is_multiple_of(record_size) {
            return Err(AgrError::MisalignedData {
                offset: start,
                data_bytes,
            });
        }
        let slots = data_bytes / record_size;
        if required > data_bytes {
            return Err(AgrError::RecordOverflow {
                offset: start,
                count,
                slots,
            });
        }
        let auxiliary_records = slots - count;
        if auxiliary_records > 0 {
            diagnostics.push(format!(
                "ignored {auxiliary_records} auxiliary 1002 record(s) after the declared stream"
            ));
        }
        let auxiliary_tail = bytes[minimum_end..data_start + data_bytes]
            .chunks_exact(record_size)
            .map(|record| {
                let mut raw = [0_u8; 8];
                raw.copy_from_slice(record);
                raw
            })
            .collect();
        (
            auxiliary_records,
            data_start + data_bytes,
            data_end.saturating_sub(start),
            data_start + data_bytes,
            data_start + data_bytes,
            auxiliary_tail,
        )
    };
    let body = &bytes[data_start..body_end];
    let metadata = &bytes[metadata_start..metadata_end];
    let tracks = match variant {
        1002 => decode_object_1002_records(body, count, duration, &mut diagnostics),
        999 => decode_object_float_records(body, count, duration, &mut diagnostics),
        1000 => decode_variant_1000_records(
            body,
            count,
            duration,
            metadata,
            &mut diagnostics,
        ),
        1001 => decode_variant_1001_records(
            body,
            count,
            duration,
            metadata,
            &mut diagnostics,
        ),
        1003 => decode_object_compact_records(body, count, duration, &mut diagnostics),
        1004 => decode_object_1004_records(body, count, duration, &mut diagnostics),
        _ => {
            return Ok(AgrClip {
                index,
                source_size: logical_source_size,
                variant,
                record_size,
                duration_s: duration,
                auxiliary_records,
                auxiliary_tail,
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
        source_size: logical_source_size,
        variant,
        record_size,
        duration_s: duration,
        auxiliary_records,
        auxiliary_tail,
        tracks,
        diagnostics,
    })
}

/// One linked rotation key shared by the object and character AGR variants.
/// Packed variants 1002/1004 use bit fields; variants 1000/1001 populate the
/// same predecessor/time/rotation model from wider or compact scalar fields.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PackedRotationKey {
    previous: usize,
    time_code: u16,
    /// Quaternion order is `(x, y, z, w)`, with signed-magnitude components.
    rotation: [f32; 4],
}

/// Decode the two packed words used by the retail 1002 and 1004 evaluators.
/// The masks match `sub_6bb6e0`/`sub_6b1870` in the retail Bully executable.
fn decode_packed_rotation_key(record: &[u8]) -> PackedRotationKey {
    debug_assert!(record.len() >= RECORD_BYTES);
    let word0 = read_u32(record, 0);
    let word1 = read_u32(record, 4);

    let qx = signed_magnitude(
        (word0 >> 21) & 0x3ff,
        word0 & (1 << 20) != 0,
        PACKED_QUAT_SCALE,
    );
    let qy = signed_magnitude(word1 & 0x3ff, word0 & (1 << 31) != 0, PACKED_QUAT_SCALE);
    let qz = signed_magnitude(
        (word1 >> 11) & 0x3ff,
        word1 & (1 << 10) != 0,
        PACKED_QUAT_SCALE,
    );
    let qw = signed_magnitude(word1 >> 22, word1 & (1 << 21) != 0, PACKED_QUAT_SCALE);

    PackedRotationKey {
        previous: (word0 & PACKED_LINK_MASK) as usize,
        time_code: ((word0 >> 11) & 0x1ff) as u16,
        // AXIS_PERM probe: temporary y/z transposition test.
        rotation: [qx, qy, qz, qw],
    }
}

/// Build the bounded predecessor forest used by the packed AGR variants.
fn packed_curve_graph(
    keys: &[PackedRotationKey],
    variant: u32,
    diagnostics: &mut Vec<String>,
) -> (Vec<Option<usize>>, Vec<usize>) {
    let mut children = vec![None; keys.len()];
    let mut roots = Vec::new();
    let mut invalid_links = 0usize;
    let mut branch_links = 0usize;
    for (index, key) in keys.iter().enumerate() {
        if key.previous == 0 {
            roots.push(index);
            continue;
        }
        if key.previous >= index || key.previous >= keys.len() {
            invalid_links += 1;
            continue;
        }
        if children[key.previous].is_some() {
            branch_links += 1;
        } else {
            children[key.previous] = Some(index);
        }
    }
    if invalid_links > 0 {
        diagnostics.push(format!(
            "ignored {invalid_links} invalid {variant} predecessor link(s)"
        ));
    }
    if branch_links > 0 {
        diagnostics.push(format!(
            "ignored {branch_links} additional {variant} successor link(s) on branched curves"
        ));
    }
    (children, roots)
}

/// Follow one predecessor-linked curve. A valid retail stream is a forest of
/// chains: each non-root record points to an earlier key, and each key has at
/// most one successor. Malformed records are bounded and reported rather than
/// being allowed to loop or index outside the chunk.
fn packed_curve_indices(
    root: usize,
    record_count: usize,
    children: &[Option<usize>],
    variant: u32,
    diagnostics: &mut Vec<String>,
) -> Vec<usize> {
    let mut curve = Vec::new();
    let mut current = root;
    while curve.len() <= record_count {
        curve.push(current);
        let Some(next) = children[current] else {
            return curve;
        };
        if next >= record_count || curve.contains(&next) {
            diagnostics.push(format!(
                "{variant} curve rooted at {root} has a successor cycle or out-of-range link"
            ));
            return curve;
        }
        current = next;
    }
    diagnostics.push(format!(
        "{variant} curve rooted at {root} exceeded the record-count safety bound"
    ));
    curve
}

fn packed_rotation_is_identity(rotation: [f32; 4]) -> bool {
    const EPSILON: f32 = 0.002;
    rotation[0].abs() <= EPSILON
        && rotation[1].abs() <= EPSILON
        && rotation[2].abs() <= EPSILON
        && (rotation[3].abs() - 1.0).abs() <= EPSILON
}

/// Decode a variant-1002 linked rotation stream. Its declared records are at
/// the beginning of the body; any bytes after `count * 8` belong to the
/// runtime auxiliary section and are deliberately excluded by the caller.
fn decode_object_1002_records(
    body: &[u8],
    count: usize,
    duration: f32,
    diagnostics: &mut Vec<String>,
) -> Vec<AgrTrack> {
    let Some(required) = count.checked_mul(RECORD_BYTES) else {
        diagnostics.push("1002 record count overflows the byte size".to_string());
        return Vec::new();
    };
    if body.len() < required {
        diagnostics.push(format!(
            "1002 body has {} bytes for {count} declared records",
            body.len()
        ));
        return Vec::new();
    }

    let keys: Vec<PackedRotationKey> = (0..count)
        .map(|index| {
            decode_packed_rotation_key(&body[index * RECORD_BYTES..(index + 1) * RECORD_BYTES])
        })
        .collect();
    let (children, roots) = packed_curve_graph(&keys, 1002, diagnostics);
    if roots.is_empty() {
        diagnostics.push("1002 stream has no curve root".to_string());
        return Vec::new();
    }
    if roots[0] != 0 {
        diagnostics.push(format!(
            "1002 default root starts at record {}, expected record 0",
            roots[0]
        ));
    }
    let default_root = roots[0];
    let mut curve_roots: Vec<usize> = roots.iter().copied().skip(1).collect();
    let mut skipped_sentinel = false;
    if let Some(&candidate) = curve_roots.last() {
        let curve = packed_curve_indices(candidate, count, &children, 1002, diagnostics);
        if curve
            .last()
            .and_then(|index| keys.get(*index))
            .is_some_and(|key| key.time_code == 511)
            && curve.iter().all(|index| {
                keys.get(*index)
                    .is_some_and(|key| packed_rotation_is_identity(key.rotation))
            })
        {
            curve_roots.pop();
            skipped_sentinel = true;
        }
    }
    diagnostics.push(format!(
        "decoded {} linked 1002 rotation curve(s); skipped default root {}{}",
        curve_roots.len(),
        default_root,
        if skipped_sentinel {
            " and terminal identity sentinel"
        } else {
            ""
        }
    ));

    let mut tracks = Vec::new();
    for root in curve_roots {
        let Some(track_id) = root.checked_sub(1).and_then(|id| u8::try_from(id).ok()) else {
            diagnostics.push(format!(
                "1002 curve root {root} cannot map to the u8 track id space"
            ));
            continue;
        };
        let curve = packed_curve_indices(root, count, &children, 1002, diagnostics);
        let mut previous_time = None;
        for index in curve {
            let key = keys[index];
            if let Some(previous_time) = previous_time
                && key.time_code < previous_time
            {
                diagnostics.push(format!(
                    "1002 curve rooted at {root} contains a decreasing time code"
                ));
            }
            previous_time = Some(key.time_code);
            let time_s = (key.time_code as f32 * PACKED_TIME_SCALE * duration).min(duration);
            let hemisphere = if key.rotation[3] < 0.0 { -1.0 } else { 1.0 };
            push_key(
                &mut tracks,
                track_id,
                0,
                time_s,
                [
                    key.rotation[0] * hemisphere,
                    key.rotation[1] * hemisphere,
                    key.rotation[2] * hemisphere,
                ],
            );
        }
    }
    finish_tracks(tracks)
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

/// Decode a linked object stream whose rotation records use a full 16-bit
/// time code and whose translations live in a sparse post-key table. Variants
/// 1000 and 1001 differ only in the rotation/translation scalar encodings;
/// their predecessor forest and default/sentinel roots are identical.
fn decode_linked_transform_tracks(
    keys: &[PackedRotationKey],
    duration: f32,
    variant: u32,
    translations: &[(usize, [f32; 3])],
    diagnostics: &mut Vec<String>,
) -> Vec<AgrTrack> {
    let (children, roots) = packed_curve_graph(keys, variant, diagnostics);
    if roots.is_empty() {
        diagnostics.push(format!("{variant} stream has no curve root"));
        return Vec::new();
    }
    if roots[0] != 0 {
        diagnostics.push(format!(
            "{variant} default root starts at record {}, expected record 0",
            roots[0]
        ));
    }

    let default_root = roots[0];
    let mut curve_roots: Vec<usize> = roots.iter().copied().skip(1).collect();
    let mut skipped_sentinel = false;
    if let Some(&candidate) = curve_roots.last() {
        let curve = packed_curve_indices(candidate, keys.len(), &children, variant, diagnostics);
        if curve
            .last()
            .and_then(|index| keys.get(*index))
            .is_some_and(|key| key.time_code == u16::MAX)
            && curve.iter().all(|index| {
                keys.get(*index)
                    .is_some_and(|key| packed_rotation_is_identity(key.rotation))
            })
        {
            curve_roots.pop();
            skipped_sentinel = true;
        }
    }

    diagnostics.push(format!(
        "decoded {} linked {variant} transform curve(s); skipped default root {}{}",
        curve_roots.len(),
        default_root,
        if skipped_sentinel {
            " and terminal identity sentinel"
        } else {
            ""
        }
    ));

    let mut tracks = Vec::new();
    let mut track_for_record = vec![None; keys.len()];
    let mut ignored_rotations = 0usize;
    for root in curve_roots {
        let Some(track_id) = root.checked_sub(1).and_then(|id| u8::try_from(id).ok()) else {
            diagnostics.push(format!(
                "{variant} curve root {root} cannot map to the u8 track id space"
            ));
            continue;
        };
        let curve = packed_curve_indices(root, keys.len(), &children, variant, diagnostics);
        let mut previous_time = None;
        for index in curve {
            let key = keys[index];
            if let Some(previous_time) = previous_time
                && key.time_code < previous_time
            {
                diagnostics.push(format!(
                    "{variant} curve rooted at {root} contains a decreasing time code"
                ));
            }
            previous_time = Some(key.time_code);
            if !key.rotation.iter().all(|value| value.is_finite()) {
                ignored_rotations += 1;
                continue;
            }
            track_for_record[index] = Some(track_id);
            let time_s = (key.time_code as f32 * FULL_TIME_SCALE * duration).min(duration);
            let hemisphere = if key.rotation[3] < 0.0 { -1.0 } else { 1.0 };
            push_key(
                &mut tracks,
                track_id,
                0,
                time_s,
                [
                    key.rotation[0] * hemisphere,
                    key.rotation[1] * hemisphere,
                    key.rotation[2] * hemisphere,
                ],
            );
        }
    }
    if ignored_rotations > 0 {
        diagnostics.push(format!(
            "ignored {ignored_rotations} non-finite {variant} rotation record(s)"
        ));
    }

    let mut ignored_translations = 0usize;
    for &(record_index, values) in translations {
        let Some(key) = keys.get(record_index) else {
            ignored_translations += 1;
            continue;
        };
        let Some(track_id) = track_for_record[record_index] else {
            ignored_translations += 1;
            continue;
        };
        let time_s = (key.time_code as f32 * FULL_TIME_SCALE * duration).min(duration);
        push_key(&mut tracks, track_id, 1, time_s, values);
    }
    if ignored_translations > 0 {
        diagnostics.push(format!(
            "ignored {ignored_translations} {variant} translation metadata record(s) without an animated curve"
        ));
    }
    finish_tracks(tracks)
}

/// Decode variant 1000: full-float linked quaternion records followed by
/// full-float sparse translations. The quaternion is stored w-first while
/// [`AgrKey`] stores xyz and derives w.
fn decode_variant_1000_records(
    body: &[u8],
    count: usize,
    duration: f32,
    metadata: &[u8],
    diagnostics: &mut Vec<String>,
) -> Vec<AgrTrack> {
    let Some(required) = count.checked_mul(FLOAT_LINKED_RECORD_BYTES) else {
        diagnostics.push("1000 record count overflows the byte size".to_string());
        return Vec::new();
    };
    if body.len() < required {
        diagnostics.push(format!(
            "1000 body has {} bytes for {count} declared records",
            body.len()
        ));
        return Vec::new();
    }

    let keys: Vec<PackedRotationKey> = (0..count)
        .map(|index| {
            let record = &body[index * FLOAT_LINKED_RECORD_BYTES
                ..(index + 1) * FLOAT_LINKED_RECORD_BYTES];
            PackedRotationKey {
                previous: u16::from_le_bytes([record[0], record[1]]) as usize,
                time_code: u16::from_le_bytes([record[2], record[3]]),
                rotation: [
                    read_f32(record, 8),
                    read_f32(record, 12),
                    read_f32(record, 16),
                    read_f32(record, 4),
                ],
            }
        })
        .collect();

    let mut translations = Vec::with_capacity(metadata.len() / 16);
    for record in metadata.chunks_exact(16) {
        let record_index = read_u32(record, 0) as usize;
        let values = [read_f32(record, 4), read_f32(record, 8), read_f32(record, 12)];
        if values.iter().all(|value| value.is_finite()) {
            translations.push((record_index, values));
        } else {
            diagnostics.push("ignored non-finite 1000 translation metadata".to_string());
        }
    }
    if !metadata.len().is_multiple_of(16) {
        diagnostics.push(format!(
            "ignored {} partial 1000 translation metadata byte(s)",
            metadata.len() % 16
        ));
    }

    decode_linked_transform_tracks(
        &keys,
        duration,
        1000,
        &translations,
        diagnostics,
    )
}

/// Decode variant 1001: compact linked quaternion records followed by
/// compact sparse translations. Both scalar families use the retail constants
/// recovered from the executable: quaternion `1 / 32767`, translation
/// `1 / 1000`.
fn decode_variant_1001_records(
    body: &[u8],
    count: usize,
    duration: f32,
    metadata: &[u8],
    diagnostics: &mut Vec<String>,
) -> Vec<AgrTrack> {
    let Some(required) = count.checked_mul(COMPACT_LINKED_RECORD_BYTES) else {
        diagnostics.push("1001 record count overflows the byte size".to_string());
        return Vec::new();
    };
    if body.len() < required {
        diagnostics.push(format!(
            "1001 body has {} bytes for {count} declared records",
            body.len()
        ));
        return Vec::new();
    }

    let keys: Vec<PackedRotationKey> = (0..count)
        .map(|index| {
            let record = &body[index * COMPACT_LINKED_RECORD_BYTES
                ..(index + 1) * COMPACT_LINKED_RECORD_BYTES];
            let x = i16::from_le_bytes([record[4], record[5]]) as f32 / COMPACT_QUAT_SCALE;
            let y = i16::from_le_bytes([record[6], record[7]]) as f32 / COMPACT_QUAT_SCALE;
            let z = i16::from_le_bytes([record[8], record[9]]) as f32 / COMPACT_QUAT_SCALE;
            let w = i16::from_le_bytes([record[10], record[11]]) as f32 / COMPACT_QUAT_SCALE;
            PackedRotationKey {
                previous: u16::from_le_bytes([record[0], record[1]]) as usize,
                time_code: u16::from_le_bytes([record[2], record[3]]),
                rotation: [x, y, z, w],
            }
        })
        .collect();

    let mut translations = Vec::with_capacity(metadata.len() / 8);
    for record in metadata.chunks_exact(8) {
        let record_index = u16::from_le_bytes([record[0], record[1]]) as usize;
        let values = [
            i16::from_le_bytes([record[2], record[3]]) as f32 * COMPACT_TRANSLATION_SCALE,
            i16::from_le_bytes([record[4], record[5]]) as f32 * COMPACT_TRANSLATION_SCALE,
            i16::from_le_bytes([record[6], record[7]]) as f32 * COMPACT_TRANSLATION_SCALE,
        ];
        translations.push((record_index, values));
    }
    if !metadata.len().is_multiple_of(8) {
        diagnostics.push(format!(
            "ignored {} partial 1001 translation metadata byte(s)",
            metadata.len() % 8
        ));
    }

    decode_linked_transform_tracks(
        &keys,
        duration,
        1001,
        &translations,
        diagnostics,
    )
}

/// One decoded record from the retail variant-1004 evaluator.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PackedObjectKey1004 {
    rotation_key: PackedRotationKey,
    translation: [f32; 3],
}

fn signed_magnitude(value: u32, negative: bool, scale: f32) -> f32 {
    let magnitude = value as f32 * scale;
    if negative { -magnitude } else { magnitude }
}

/// Decode one 12-byte object key using the bit masks recovered from the
/// retail Bully executable (`sub_6bb6e0`, `sub_6b19d0`, `sub_6bc240`).
fn decode_packed_object_key_1004(record: &[u8]) -> PackedObjectKey1004 {
    debug_assert!(record.len() >= 12);
    let rotation_key = decode_packed_rotation_key(record);
    let word2 = read_u32(record, 8);

    let tx = signed_magnitude(
        word2 & 0x3ff,
        word2 & (1 << 10) != 0,
        PACKED_1004_TRANSLATION_SCALE,
    );
    let ty = signed_magnitude(
        (word2 >> 11) & 0x3ff,
        word2 & (1 << 21) != 0,
        PACKED_1004_TRANSLATION_SCALE,
    );
    let tz = signed_magnitude(
        (word2 >> 22) & 0x1ff,
        word2 & (1 << 31) != 0,
        PACKED_1004_TRANSLATION_SCALE,
    );

    PackedObjectKey1004 {
        rotation_key,
        translation: [tx, ty, tz],
    }
}

/// Decode a variant-1004 linked transform stream.
///
/// The retail evaluator treats the first word as `{ previous:11,
/// time_code:9, qx_sign:1, qx_magnitude:10, qy_sign:1 }`. The second word is
/// `{ qy_magnitude:10, qz_sign:1, qz_magnitude:10, qw_sign:1,
/// qw_magnitude:10 }`; the third is `{ tx: magnitude:10 + sign, ty:
/// magnitude:10 + sign, tz: magnitude:9 + sign }`. Time is
/// `time_code / 511 * duration`, quaternion magnitudes are divided by 1023,
/// and translations are divided by 100. The predecessor links are the track
/// structure; low-link zero means a curve root, not a default frame.
fn decode_object_1004_records(
    body: &[u8],
    count: usize,
    duration: f32,
    diagnostics: &mut Vec<String>,
) -> Vec<AgrTrack> {
    let Some(required) = count.checked_mul(12) else {
        diagnostics.push("1004 record count overflows the byte size".to_string());
        return Vec::new();
    };
    if body.len() < required {
        diagnostics.push(format!(
            "1004 body has {} bytes for {count} declared records",
            body.len()
        ));
        return Vec::new();
    }

    let keys: Vec<PackedObjectKey1004> = (0..count)
        .map(|index| decode_packed_object_key_1004(&body[index * 12..(index + 1) * 12]))
        .collect();
    let rotation_keys: Vec<PackedRotationKey> = keys.iter().map(|key| key.rotation_key).collect();
    let (children, roots) = packed_curve_graph(&rotation_keys, 1004, diagnostics);
    if roots.is_empty() {
        diagnostics.push("1004 stream has no curve root".to_string());
        return Vec::new();
    }
    if roots[0] != 0 {
        diagnostics.push(format!(
            "1004 default root starts at record {}, expected record 0",
            roots[0]
        ));
    }

    // Record zero is the stream-wide default pose. Retail chunks also end in
    // an identity curve that terminates the linked data. Detect the latter by
    // content, rather than blindly dropping the last root, so a shortened or
    // hand-authored stream with no sentinel still remains useful.
    let default_root = roots[0];
    let mut curve_roots: Vec<usize> = roots.iter().copied().skip(1).collect();
    let mut skipped_sentinel = false;
    if let Some(&candidate) = curve_roots.last() {
        let curve = packed_curve_indices(candidate, count, &children, 1004, diagnostics);
        if curve
            .last()
            .and_then(|index| keys.get(*index))
            .is_some_and(|key| key.rotation_key.time_code == 511)
            && curve.iter().all(|index| {
                keys.get(*index).is_some_and(|key| {
                    packed_rotation_is_identity(key.rotation_key.rotation)
                        && key
                            .translation
                            .iter()
                            .all(|component| component.abs() <= 0.002)
                })
            })
        {
            curve_roots.pop();
            skipped_sentinel = true;
        }
    }
    diagnostics.push(format!(
        "decoded {} linked 1004 transform curve(s); skipped default root {}{}",
        curve_roots.len(),
        default_root,
        if skipped_sentinel {
            " and terminal identity sentinel"
        } else {
            ""
        }
    ));

    let mut tracks = Vec::new();
    for root in curve_roots {
        let Some(track_id) = root.checked_sub(1).and_then(|id| u8::try_from(id).ok()) else {
            diagnostics.push(format!(
                "1004 curve root {root} cannot map to the u8 track id space"
            ));
            continue;
        };
        let curve = packed_curve_indices(root, count, &children, 1004, diagnostics);
        let mut previous_time = None;
        for index in curve {
            let key = keys[index];
            if let Some(previous_time) = previous_time
                && key.rotation_key.time_code < previous_time
            {
                diagnostics.push(format!(
                    "1004 curve rooted at {root} contains a decreasing time code"
                ));
            }
            previous_time = Some(key.rotation_key.time_code);
            let time_s =
                (key.rotation_key.time_code as f32 * PACKED_TIME_SCALE * duration).min(duration);

            // AgrKey derives the positive quaternion hemisphere. If the
            // packed runtime quaternion has negative w, negate all four
            // components before storing xyz so the represented orientation is
            // preserved by that normalized downstream form.
            let hemisphere = if key.rotation_key.rotation[3] < 0.0 {
                -1.0
            } else {
                1.0
            };
            push_key(
                &mut tracks,
                track_id,
                0,
                time_s,
                [
                    key.rotation_key.rotation[0] * hemisphere,
                    key.rotation_key.rotation[1] * hemisphere,
                    key.rotation_key.rotation[2] * hemisphere,
                ],
            );
            push_key(&mut tracks, track_id, 1, time_s, key.translation);
        }
    }
    finish_tracks(tracks)
}

/// Convert a parsed file into a runtime [`AnimationLibrary`].
///
/// Rotation channels (0) become compact-quat tracks; translation channels
/// (1) become position tracks. The source decoder applies the variant-specific
/// quantization before this conversion. Channel 2 (scale) is still withheld.
/// Track targets are `track_{id}` so the NIF adapter can map ids to nodes.
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

/// Renames clips from an HXD record's ordered sequence list.
///
/// The AGR chunk index maps onto the HXD sequence index (verified on
/// SK8Board: 17 chunks = 17 sequences, durations matching). Renaming only
/// applies when the counts agree; returns how many clips were renamed.
pub fn apply_hxd_names(
    library: &mut AnimationLibrary,
    record: &crate::inspector::animation::hxd::HxdRecord,
) -> usize {
    apply_clip_names(library, &record.sequence_names())
}

/// [`apply_hxd_names`] for a pre-extracted name list. Names are aligned by
/// index; empty entries leave the positional `clip_NN` fallback in place
/// (compound catalogs may cover only part of an AGR). Returns the number of
/// clips actually renamed.
pub fn apply_clip_names(library: &mut AnimationLibrary, names: &[String]) -> usize {
    if library.clips.len() != names.len() {
        return 0;
    }
    let mut renamed = 0;
    for (clip, name) in library.clips.iter_mut().zip(names) {
        if name.is_empty() {
            continue;
        }
        clip.name = name.clone();
        renamed += 1;
    }
    renamed
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
/// The node hierarchy and bind-local geometry come from the NIF. Mesh skin
/// instances are decoded through the Gamebryo chain
/// `NiSkinInstance -> NiSkinData -> NiSkinPartition`: partition vertices
/// reference a per-partition bone palette, the palette indexes the skin
/// instance's bone list, and each bone carries the mesh-local inverse bind
/// transform. Nodes are initially named `track_{id}` in scene-graph DFS order
/// to keep the imported hierarchy stable; AGR sessions resolve those names
/// through cross-clip bind-pose calibration because packed curve order can
/// include a source-root offset and attachment nodes.
pub fn model_from_nif(
    nif: &NifFile,
    name: impl Into<String>,
    source_identity: impl Into<String>,
) -> Result<ModelAsset, String> {
    model_from_nif_with_mapping(nif, name, source_identity, NifMapping::default())
}

/// One mesh's decoded-but-unresolved skin. Bone NIF blocks become [`NodeId`]s
/// only after the whole hierarchy is visited, because a skin instance may
/// reference nodes outside the mesh's own subtree.
struct PendingSkin {
    mesh_index: usize,
    skeleton_root: i32,
    bones: Vec<i32>,
    inverse_bind: Vec<Mat4>,
    weights: Vec<VertexSkin>,
}

/// Convert a NIF `NiTransform` into the model's node-space matrix using the
/// same row-major-to-column-major convention as [`nif_local`].
fn nif_transform_matrix(transform: &crate::inspector::nif::NiTransform) -> Mat4 {
    nif_local(
        &transform.translation,
        &transform.rotation.m,
        transform.scale,
    )
    .matrix()
}

fn partition_skin_weights(
    partition: &crate::inspector::nif::NiSkinPartitionPayload,
    vertex_count: usize,
    joint_count: usize,
) -> Result<Vec<VertexSkin>, String> {
    let mut decoded: Vec<Option<VertexSkin>> = vec![None; vertex_count];
    for (partition_index, part) in partition.partitions.iter().enumerate() {
        let rows = part.num_vertices as usize;
        let width = part.num_weights_per_vertex as usize;
        if part.bones.len() != part.num_bones as usize {
            return Err(format!(
                "partition {partition_index} declares {} palette bones but stores {}",
                part.num_bones,
                part.bones.len()
            ));
        }
        if width == 0 || part.weights.len() != rows || part.bone_indices.len() != rows {
            return Err(format!(
                "partition {partition_index} has incomplete weight/index tables"
            ));
        }
        if !part.vertex_map.is_empty() && part.vertex_map.len() != rows {
            return Err(format!(
                "partition {partition_index} has an incomplete vertex map"
            ));
        }

        for local in 0..rows {
            let weight_row = &part.weights[local];
            let index_row = &part.bone_indices[local];
            if weight_row.len() != width || index_row.len() != width {
                return Err(format!(
                    "partition {partition_index} vertex {local} has a truncated influence row"
                ));
            }
            let target = part
                .vertex_map
                .get(local)
                .map_or(local, |&mapped| mapped as usize);
            if target >= vertex_count {
                return Err(format!(
                    "partition {partition_index} maps vertex {local} to {target}, outside {vertex_count} vertices"
                ));
            }

            let mut influences = VertexSkin::new();
            for (&weight, &palette_index) in weight_row.iter().zip(index_row) {
                if !weight.is_finite() || weight < 0.0 {
                    return Err(format!(
                        "partition {partition_index} vertex {local} has an invalid weight"
                    ));
                }
                if weight == 0.0 {
                    continue;
                }
                let Some(&joint) = part.bones.get(palette_index as usize) else {
                    return Err(format!(
                        "partition {partition_index} vertex {local} references palette slot {palette_index} outside {} bones",
                        part.bones.len()
                    ));
                };
                if joint as usize >= joint_count {
                    return Err(format!(
                        "partition {partition_index} vertex {local} references skin joint {joint} outside {joint_count} joints"
                    ));
                }
                if let Some(existing) = influences
                    .iter_mut()
                    .find(|(slot, _)| *slot == joint as u32)
                {
                    existing.1 += weight;
                } else {
                    influences.push((joint as u32, weight));
                }
            }
            influences.sort_unstable_by_key(|(slot, _)| *slot);

            if let Some(previous) = &decoded[target] {
                let same = previous.len() == influences.len()
                    && previous
                        .iter()
                        .zip(&influences)
                        .all(|(left, right)| left.0 == right.0 && (left.1 - right.1).abs() <= 1e-5);
                if !same {
                    return Err(format!(
                        "partition {partition_index} disagrees with an earlier partition for vertex {target}"
                    ));
                }
            } else {
                decoded[target] = Some(influences);
            }
        }
    }
    let weights: Vec<VertexSkin> = decoded.into_iter().map(Option::unwrap_or_default).collect();
    if !weights.iter().any(|row| !row.is_empty()) {
        return Err("partition produced no positive influences".to_string());
    }
    Ok(weights)
}

fn direct_skin_weights(
    skin_data: &crate::inspector::nif::NiSkinDataPayload,
    vertex_count: usize,
) -> Result<Vec<VertexSkin>, String> {
    if !skin_data.has_vertex_weights {
        return Err("skin data does not contain vertex weights".to_string());
    }
    let mut weights = vec![VertexSkin::new(); vertex_count];
    for (slot, bone) in skin_data.bones.iter().enumerate() {
        for influence in &bone.vertex_weights {
            if !influence.weight.is_finite() || influence.weight < 0.0 {
                return Err(format!("skin joint {slot} has an invalid weight"));
            }
            if influence.weight == 0.0 {
                continue;
            }
            let target = influence.index as usize;
            if target >= vertex_count {
                return Err(format!(
                    "skin joint {slot} references vertex {target} outside {vertex_count} vertices"
                ));
            }
            if let Some(existing) = weights[target]
                .iter_mut()
                .find(|(joint, _)| *joint == slot as u32)
            {
                existing.1 += influence.weight;
            } else {
                weights[target].push((slot as u32, influence.weight));
            }
        }
    }
    if !weights.iter().any(|row| !row.is_empty()) {
        return Err("skin data vertex weights are empty".to_string());
    }
    Ok(weights)
}

/// Decode the skin chain for one shape into a pending, node-unresolved skin.
///
/// Weights come from `NiSkinPartition` (the retail storage): each partition
/// vertex lists `(palette index, weight)` pairs, its vertex map points at the
/// shape vertex, and the palette holds indices into the skin instance's bone
/// list. `NiSkinData`'s per-bone vertex weights are used as a fallback when
/// no partition is present.
fn build_pending_skin(
    nif: &NifFile,
    skin_instance_ref: i32,
    vertex_count: usize,
    mesh_index: usize,
    diagnostics: &mut Vec<String>,
) -> Option<PendingSkin> {
    if skin_instance_ref < 0 {
        return None;
    }
    let Some(BlockPayload::NiSkinInstance(instance)) = nif
        .payloads
        .get(skin_instance_ref as usize)
        .and_then(|p| p.as_ref())
    else {
        diagnostics.push("skin instance block is missing or unsupported".to_string());
        return None;
    };
    let skin_data = if instance.data_ref >= 0 {
        nif.payloads
            .get(instance.data_ref as usize)
            .and_then(|p| p.as_ref())
            .and_then(|payload| match payload {
                BlockPayload::NiSkinData(data) => Some(data),
                _ => None,
            })
    } else {
        None
    };
    let Some(skin_data) = skin_data else {
        diagnostics.push("skin data block is missing or unsupported".to_string());
        return None;
    };
    if instance.bones.len() != skin_data.bones.len() {
        diagnostics.push(format!(
            "skin skipped: {} bones but {} bind transforms",
            instance.bones.len(),
            skin_data.bones.len()
        ));
        return None;
    }
    let joint_count = instance.bones.len();
    if joint_count == 0 {
        diagnostics.push("skin skipped: no joints".to_string());
        return None;
    }
    if instance.skeleton_root_ref < 0
        || !matches!(
            nif.payloads
                .get(instance.skeleton_root_ref as usize)
                .and_then(|payload| payload.as_ref()),
            Some(BlockPayload::NiNode(_))
        )
    {
        diagnostics.push("skin skipped: skeleton root is missing or is not a NiNode".to_string());
        return None;
    }
    let mut unique_bones = std::collections::HashSet::with_capacity(joint_count);
    for &bone in &instance.bones {
        if bone < 0
            || !matches!(
                nif.payloads
                    .get(bone as usize)
                    .and_then(|payload| payload.as_ref()),
                Some(BlockPayload::NiNode(_))
            )
        {
            diagnostics.push(format!(
                "skin skipped: joint reference {bone} is missing or is not a NiNode"
            ));
            return None;
        }
        if !unique_bones.insert(bone) {
            diagnostics.push(format!("skin skipped: duplicate joint reference {bone}"));
            return None;
        }
    }
    let inverse_bind: Vec<Mat4> = skin_data
        .bones
        .iter()
        .map(|bone| nif_transform_matrix(&bone.skin_transform))
        .collect();
    if inverse_bind.iter().any(|matrix| {
        matrix
            .to_cols_array()
            .iter()
            .any(|value| !value.is_finite())
    }) {
        diagnostics.push("skin skipped: inverse-bind transform is non-finite".to_string());
        return None;
    }

    let partition = if instance.skin_partition_ref >= 0 {
        nif.payloads
            .get(instance.skin_partition_ref as usize)
            .and_then(|p| p.as_ref())
            .and_then(|payload| match payload {
                BlockPayload::NiSkinPartition(partition) => Some(partition),
                _ => None,
            })
    } else {
        None
    };
    let weights = match partition.filter(|partition| !partition.partitions.is_empty()) {
        Some(partition) => match partition_skin_weights(partition, vertex_count, joint_count) {
            Ok(weights) => weights,
            Err(partition_error) => {
                diagnostics.push(format!(
                    "skin partition rejected ({partition_error}); trying direct weights"
                ));
                match direct_skin_weights(skin_data, vertex_count) {
                    Ok(weights) => weights,
                    Err(direct_error) => {
                        diagnostics.push(format!("skin skipped: {direct_error}"));
                        return None;
                    }
                }
            }
        },
        None => {
            if instance.skin_partition_ref >= 0 {
                diagnostics.push(
                    "skin partition is missing, unsupported, or empty; trying direct weights"
                        .to_string(),
                );
            }
            match direct_skin_weights(skin_data, vertex_count) {
                Ok(weights) => weights,
                Err(error) => {
                    diagnostics.push(format!("skin skipped: {error}"));
                    return None;
                }
            }
        }
    };
    let influenced = weights.iter().filter(|row| !row.is_empty()).count();
    diagnostics.push(format!(
        "skin: {} joints, {} influenced vertices",
        joint_count, influenced
    ));
    Some(PendingSkin {
        mesh_index,
        skeleton_root: instance.skeleton_root_ref,
        bones: instance.bones.clone(),
        inverse_bind,
        weights,
    })
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
    let mut node_ids: std::collections::HashMap<i32, NodeId> = std::collections::HashMap::new();
    let mut pending_skins: Vec<PendingSkin> = Vec::new();
    let mut source_names: Vec<Option<String>> = Vec::new();
    // NIF-wide diffuse fallback for shapes without their own texturing
    // properties (resolved to pixels later by the caller, never here).
    let nif_diffuse = crate::inspector::viewer3d::find_diffuse_texture(nif);

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
            &mut node_ids,
            &mut pending_skins,
            &mut source_names,
            nif_diffuse.as_deref(),
        );
    }
    if nodes.is_empty() {
        return Err("the NIF has no scene-graph nodes".to_string());
    }

    // Resolve pending skins now that every visited block has a node id.
    let mut skinned_meshes = 0usize;
    for pending in pending_skins {
        let Some(&skeleton_root) = node_ids.get(&pending.skeleton_root) else {
            diagnostics.push(format!(
                "skin for '{}' skipped: skeleton root is outside the scene hierarchy",
                meshes
                    .get(pending.mesh_index)
                    .map(|mesh| mesh.name.as_str())
                    .unwrap_or("?")
            ));
            continue;
        };
        let mut joints = Vec::with_capacity(pending.bones.len());
        let mut unresolved = 0usize;
        for &bone in &pending.bones {
            match node_ids.get(&bone) {
                Some(&id) => joints.push(id),
                None => unresolved += 1,
            }
        }
        if unresolved > 0 || joints.len() != pending.inverse_bind.len() {
            diagnostics.push(format!(
                "skin for '{}' skipped: {unresolved} unresolved bone(s)",
                meshes
                    .get(pending.mesh_index)
                    .map(|mesh| mesh.name.as_str())
                    .unwrap_or("?")
            ));
            continue;
        }
        let outside_root = joints
            .iter()
            .filter(|&&joint| {
                let mut cursor = Some(joint);
                while let Some(node) = cursor {
                    if node == skeleton_root {
                        return false;
                    }
                    cursor = nodes.get(node.0 as usize).and_then(|node| node.parent);
                }
                true
            })
            .count();
        if outside_root > 0 {
            diagnostics.push(format!(
                "skin for '{}' skipped: {outside_root} joint(s) are outside the declared skeleton root",
                meshes
                    .get(pending.mesh_index)
                    .map(|mesh| mesh.name.as_str())
                    .unwrap_or("?")
            ));
            continue;
        }
        if let Some(mesh) = meshes.get_mut(pending.mesh_index) {
            mesh.skin = Some(SkinBinding {
                joints,
                inverse_bind: pending.inverse_bind,
                weights: pending.weights,
            });
            skinned_meshes += 1;
        }
    }
    if skinned_meshes > 0 {
        diagnostics.push(format!("{skinned_meshes} skinned mesh(es)"));
    }
    diagnostics.push(format!("{} nodes, {} meshes", nodes.len(), meshes.len()));

    let mut asset = ModelAsset::new(
        name.into(),
        source_identity.into(),
        nodes,
        meshes,
        BaseOrientation::Zup.to_yup_matrix(),
        BaseOrientation::Zup,
        None,
    )
    .map_err(|error| format!("model admission: {error}"))?;
    asset.set_source_names(source_names);

    // The NIF footer root is a scene container, not the actor's motion root.
    // Bully character exports identify the actual transform carrier as
    // `Dummy`; wrappers may appear before it and an `ARROW` attachment may
    // appear after the skeleton. Preserve a conservative root fallback for
    // non-character/older files without letting those helpers drive motion.
    asset.root_motion_node = asset
        .nodes
        .iter()
        .find(|node| {
            asset
                .source_name(node.id)
                .is_some_and(|name| name.eq_ignore_ascii_case("Dummy"))
        })
        .map(|node| node.id)
        .or_else(|| {
            asset
                .nodes
                .iter()
                .find(|node| {
                    node.parent.is_some()
                        && node.mesh.is_none()
                        && !asset
                            .source_name(node.id)
                            .is_some_and(|name| name.eq_ignore_ascii_case("Scene Root"))
                })
                .map(|node| node.id)
        })
        .or(Some(NodeId(0)));

    let hidden_meshes = asset
        .meshes
        .iter()
        .enumerate()
        .map(|(mesh_index, mesh)| {
            let Some(node_id) = asset.mesh_node(mesh_index) else {
                return false;
            };
            let source_name = asset.source_name(node_id).unwrap_or_default();
            let parent_is_arrow = asset
                .node(node_id)
                .and_then(|node| node.parent)
                .and_then(|parent| asset.source_name(parent))
                .is_some_and(|name| name.eq_ignore_ascii_case("ARROW"));
            let axis_helper = source_name.eq_ignore_ascii_case("Mesh")
                && mesh.vertices.len() == 6
                && mesh.indices.len() == 24;
            // `Editable Poly`/`Mesh` are 3ds Max default names: editor
            // helpers carry them, but so do many character bodies (the
            // wrapper-heavy ped NIFs). Only an unskinned mesh under one of
            // those names is treated as a stray editor object; a skinned
            // mesh is real geometry and must stay visible.
            let named_editor_object = mesh.skin.is_none()
                && (source_name.eq_ignore_ascii_case("Editable Poly")
                    || source_name.eq_ignore_ascii_case("Mesh"));
            parent_is_arrow || axis_helper || named_editor_object
        })
        .collect();
    asset.set_hidden_meshes(hidden_meshes);
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
    node_ids: &mut std::collections::HashMap<i32, NodeId>,
    pending_skins: &mut Vec<PendingSkin>,
    source_names: &mut Vec<Option<String>>,
    nif_diffuse: Option<&str>,
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
            let texture_name = crate::inspector::viewer3d::diffuse_texture_for_properties(
                nif,
                &data.properties,
            )
            .or_else(|| nif_diffuse.map(str::to_owned));
            let mesh_index = build_mesh_from_shape(
                nif,
                data.data_ref,
                data.skin_instance_ref,
                data.name.as_deref().unwrap_or(&block.type_name),
                texture_name,
                meshes,
                diagnostics,
                pending_skins,
            );
            (
                nif_local(&data.translation, &data.rotation.m, data.scale),
                mesh_index,
                Vec::new(),
            )
        }
        BlockPayload::NiTriStrips(data) => {
            let texture_name = crate::inspector::viewer3d::diffuse_texture_for_properties(
                nif,
                &data.base.properties,
            )
            .or_else(|| nif_diffuse.map(str::to_owned));
            let mesh_index = build_mesh_from_shape(
                nif,
                data.base.data_ref,
                data.base.skin_instance_ref,
                data.base.name.as_deref().unwrap_or(&block.type_name),
                texture_name,
                meshes,
                diagnostics,
                pending_skins,
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
    } else if original_name
        .as_deref()
        .is_some_and(|name| name.eq_ignore_ascii_case("Scene Root"))
    {
        // `Scene Root` is the NIF scene-graph root, not an animation target:
        // the exported animation skeleton starts one level below it. AGR
        // track indices must skip it or every track binds its parent bone.
        "Scene Root".to_string()
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
    source_names.push(original_name);
    node_ids.insert(block_index, id);
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
            node_ids,
            pending_skins,
            source_names,
            nif_diffuse,
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
    skin_instance_ref: i32,
    name: &str,
    texture_name: Option<String>,
    meshes: &mut Vec<MeshAsset>,
    diagnostics: &mut Vec<String>,
    pending_skins: &mut Vec<PendingSkin>,
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
        texture_name,
        vertices,
        indices,
        diffuse: None,
        skin: None,
    });
    if let Some(pending) = build_pending_skin(
        nif,
        skin_instance_ref,
        meshes[index].vertices.len(),
        index,
        diagnostics,
    ) {
        pending_skins.push(pending);
    }
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

    fn push_packed_1002_words(out: &mut Vec<u8>, word0: u32, word1: u32) {
        push_u32(out, word0);
        push_u32(out, word1);
    }

    fn push_linked_1000_record(
        out: &mut Vec<u8>,
        previous: u16,
        time_code: u16,
        w: f32,
        x: f32,
        y: f32,
        z: f32,
    ) {
        out.extend_from_slice(&previous.to_le_bytes());
        out.extend_from_slice(&time_code.to_le_bytes());
        for value in [w, x, y, z] {
            push_f32(out, value);
        }
    }

    fn push_linked_1001_record(
        out: &mut Vec<u8>,
        previous: u16,
        time_code: u16,
        x: i16,
        y: i16,
        z: i16,
        w: i16,
    ) {
        out.extend_from_slice(&previous.to_le_bytes());
        out.extend_from_slice(&time_code.to_le_bytes());
        for value in [x, y, z, w] {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }

    fn push_1000_translation(out: &mut Vec<u8>, record_index: u32, values: [f32; 3]) {
        push_u32(out, record_index);
        for value in values {
            push_f32(out, value);
        }
    }

    fn push_1001_translation(out: &mut Vec<u8>, record_index: u16, values: [i16; 3]) {
        out.extend_from_slice(&record_index.to_le_bytes());
        for value in values {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }

    /// Synthetic packed-1002 file with invented data (never game payloads).
    fn fixture() -> Vec<u8> {
        let mut out = Vec::new();
        // A 1002 body stores the declared stream first, followed by auxiliary
        // records. The latter deliberately look like valid roots to prove
        // they are not exposed as animation curves.
        push_u32(&mut out, AGR_MAGIC);
        push_u32(&mut out, 1002);
        push_u32(&mut out, 4); // declared animation records
        push_u32(&mut out, 0);
        push_f32(&mut out, 1.0); // duration
        let identity = 1023u32 << 22;
        let qx = 512u32 << 21;
        let qw = 886u32 << 22;
        push_packed_1002_words(&mut out, 0, identity); // default root
        push_packed_1002_words(&mut out, qx, qw); // curve root, t=0
        push_packed_1002_words(&mut out, qx | (255 << 11) | 1, qw); // t=255/511
        push_packed_1002_words(&mut out, 511 << 11, identity); // sentinel root
        push_packed_1002_words(&mut out, 0, 0); // auxiliary root-like record
        push_packed_1002_words(&mut out, 1 | (511 << 11), 0); // auxiliary child
        out
    }

    #[test]
    fn parses_synthetic_fixture() {
        let file = parse_agr(&fixture()).expect("fixture parses");
        assert_eq!(file.variant, 1002);
        assert_eq!(file.clip_count(), 1);
        let clip = &file.clips[0];
        assert_eq!(clip.auxiliary_records, 2);
        assert_eq!(clip.auxiliary_tail.len(), 2);
        assert_eq!(clip.auxiliary_tail[0], [0; 8]);
        assert_eq!(
            clip.auxiliary_tail[1],
            [1, 248, 15, 0, 0, 0, 0, 0],
            "1002 tail records must be preserved byte-for-byte"
        );
        assert!((clip.duration_s - 1.0).abs() < 1e-6);
        let rotation = clip
            .tracks
            .iter()
            .find(|t| t.track == 0 && t.channel == 0)
            .expect("rotation track");
        assert_eq!(rotation.keys.len(), 2);
        assert!((rotation.keys[0].time_s - 0.0).abs() < 1e-6);
        assert!((rotation.keys[1].time_s - 255.0 / 511.0).abs() < 1e-6);
        // Packed qx magnitude 512/1023 -> x ~= 0.5005 and a positive w.
        let q = rotation.keys[1].rotation();
        assert!((q.x - 512.0 / 1023.0).abs() < 1e-4);
        assert!((q.w - 0.8657).abs() < 1e-3);
        assert!(
            clip.diagnostics
                .iter()
                .any(|d| d.contains("auxiliary 1002"))
        );
    }

    #[test]
    fn packed_1002_stream_uses_predecessor_curves() {
        let file = parse_agr(&fixture()).expect("fixture parses");
        let clip = &file.clips[0];
        let rotation = clip
            .tracks
            .iter()
            .find(|t| t.track == 0 && t.channel == 0)
            .expect("rotation track");
        assert_eq!(rotation.keys.len(), 2);
        assert!((rotation.keys[0].time_s - 0.0).abs() < 1e-4);
        assert!((rotation.keys[1].time_s - 255.0 / 511.0).abs() < 1e-4);
        assert!(rotation.keys[0].time_s < rotation.keys[1].time_s);
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
    fn archive_padding_does_not_remove_declared_zero_1002_record() {
        let mut data = Vec::with_capacity(2048);
        push_u32(&mut data, AGR_MAGIC);
        push_u32(&mut data, 1002);
        push_u32(&mut data, 1);
        push_u32(&mut data, 0);
        push_f32(&mut data, 1.0);
        push_packed_1002_words(&mut data, 0, 0);
        data.resize(2048, 0);

        let file = parse_agr(&data).expect("zero-ended declared record remains valid");
        let clip = &file.clips[0];
        assert_eq!(clip.auxiliary_records, 0);
        assert_eq!(clip.source_size, CHUNK_HEADER_BYTES + RECORD_BYTES);
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
        // Optional HXD catalog naming: point IMGEDITOR_BULLY_ANIM at the
        // game's Anim folder to print the resolved clip names.
        if let Ok(anim) = std::env::var("IMGEDITOR_BULLY_ANIM") {
            let stem = std::path::Path::new(&agr_path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default();
            if let Some(record) =
                crate::inspector::animation::hxd::find_for_agr(std::path::Path::new(&anim), stem)
            {
                let signatures: Vec<_> = file
                    .clips
                    .iter()
                    .map(|clip| crate::inspector::animation::hxd::HxdClipSignature {
                        source_size: clip.source_size,
                        duration_s: clip.duration_s,
                    })
                    .collect();
                let names = record.sequence_names_for_agr(stem, &signatures);
                println!(
                    "  -- HXD names resolved: {} of {} clips --",
                    names.len(),
                    file.clips.len()
                );
                for (index, name) in names.iter().enumerate().take(30) {
                    println!("     clip {index:02} = {name}");
                }
            } else {
                println!("  -- no HXD record for stem '{stem}' --");
            }
        }
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
                "  clip {:02}: variant={} dur={:.3} tracks={} auxiliary={} diag={:?}",
                clip.index,
                clip.variant,
                clip.duration_s,
                clip.tracks.len(),
                clip.auxiliary_records,
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

    /// Developer corpus sweep: walk every `.agr` entry in the retail
    /// `World.img`, pair it exactly like the GUI does (HXD catalog first,
    /// the stem heuristic second), run the real calibration/binding path on
    /// the first clip, and report per-pair rig-shape facts. Surfaces the
    /// "Mandy-like" cases (placeholder behind wrapper branches), structural
    /// contract mismatches, and confident-rest-match violations — the
    /// semantic twist signal.
    ///
    /// Set `IMGEDITOR_BULLY_STREAM`, then run:
    /// `cargo test -j 2 --lib agr_corpus_audit_when_requested -- --ignored --nocapture`
    /// The full report is written to `target/agr-corpus-audit.txt`.
    #[test]
    #[ignore = "developer corpus sweep; run explicitly with --ignored"]
    fn agr_corpus_audit_when_requested() {
        use crate::archive::EntryInfo;
        use crate::inspector::animation::binding::{
            bind_clip_with_calibration, calibrate_bindings, target_rest_matches,
        };
        use crate::inspector::animation::model::ModelAsset;
        use crate::inspector::scene3d::camera::BaseOrientation;

        fn descends_from(model: &ModelAsset, mut id: crate::inspector::animation::NodeId, ancestor: crate::inspector::animation::NodeId) -> bool {
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
        let track_of = |model: &ModelAsset, id: crate::inspector::animation::NodeId| -> Option<u32> {
            model
                .node(id)?
                .name
                .strip_prefix("track_")
                .and_then(|number| number.parse::<u32>().ok())
        };
        let node_name = |model: &ModelAsset, id: Option<crate::inspector::animation::NodeId>| {
            id.and_then(|id| model.node(id))
                .map(|node| node.name.clone())
                .unwrap_or_else(|| "-".to_string())
        };

        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let stream = std::path::Path::new(&stream);
        let Ok(dir_bytes) = std::fs::read(stream.join("World.dir")) else {
            return;
        };
        let entries: Vec<EntryInfo> = dir_bytes
            .chunks_exact(32)
            .map(|record| {
                let end = record[8..].iter().position(|byte| *byte == 0).unwrap_or(24);
                EntryInfo::new(String::from_utf8_lossy(&record[8..8 + end]).into_owned())
            })
            .collect();
        let anim = stream.parent().expect("game root").join("Anim");

        let mut models: std::collections::HashMap<usize, ModelAsset> =
            std::collections::HashMap::new();
        let mut rows: Vec<String> = Vec::new();
        let mut character_offsets: std::collections::BTreeMap<u32, Vec<String>> =
            std::collections::BTreeMap::new();
        let (mut total, mut paired, mut failed) = (0usize, 0usize, 0usize);
        let mut character_rows: Vec<String> = Vec::new();
        let mut naming_gaps: Vec<String> = Vec::new();
        let (mut naming_no_record, mut naming_zero_named, mut naming_partial) =
            (0usize, 0usize, 0usize);

        for entry in &entries {
            let agr_name = entry.file_name.to_string();
            if !agr_name.to_ascii_lowercase().ends_with(".agr") {
                continue;
            }
            total += 1;
            let stem = agr_name
                .rsplit_once('.')
                .map(|(stem, _)| stem)
                .unwrap_or(&agr_name);
            let hxd = crate::inspector::animation::hxd::find_for_agr(&anim, stem);
            let model_entry = hxd
                .as_ref()
                .and_then(|record| {
                    record
                        .model_for_source(stem)
                        .and_then(|model| {
                            crate::inspector::animation::hxd::find_model_entry(&entries, model)
                        })
                        .or_else(|| {
                            crate::inspector::animation::hxd::find_model_entry(
                                &entries,
                                &record.model,
                            )
                        })
                })
                .or_else(|| crate::ui::app::find_agr_model_entry(&entries, &agr_name));
            let Some(model_index) = model_entry else {
                failed += 1;
                rows.push(format!("{agr_name}: no model paired"));
                continue;
            };
            let model_name = entries[model_index].file_name.to_string();
            let Some(agr_bytes) = world_entry(stream, &agr_name) else {
                failed += 1;
                rows.push(format!("{agr_name}: archive read failed"));
                continue;
            };
            let Ok(file) = parse_agr(&agr_bytes) else {
                failed += 1;
                rows.push(format!("{agr_name} | {model_name}: AGR parse failed"));
                continue;
            };
            let library = to_library(&file, &agr_name);
            let Some(clip) = library.clips.first() else {
                failed += 1;
                rows.push(format!("{agr_name} | {model_name}: no clips"));
                continue;
            };
            if !models.contains_key(&model_index) {
                let Some(nif_bytes) = world_entry(stream, &model_name) else {
                    failed += 1;
                    rows.push(format!("{agr_name} | {model_name}: NIF read failed"));
                    continue;
                };
                let Ok(mut nif) = NifFile::parse(&nif_bytes) else {
                    failed += 1;
                    rows.push(format!("{agr_name} | {model_name}: NIF parse failed"));
                    continue;
                };
                nif.resolve_string_indices();
                match model_from_nif(&nif, &model_name, format!("audit:{model_name}")) {
                    Ok(model) => {
                        models.insert(model_index, model);
                    }
                    Err(error) => {
                        failed += 1;
                        rows.push(format!(
                            "{agr_name} | {model_name}: model admission failed: {error}"
                        ));
                        continue;
                    }
                }
            }
            let Some(model) = models.get(&model_index) else {
                continue;
            };
            paired += 1;

            let calibration = calibrate_bindings(model, &library);
            let binding = bind_clip_with_calibration(model, clip, &calibration);

            // Naming coverage: how many clips the HXD catalog names, and
            // whether the unnamed ones are single-frame held-pose stubs
            // (one-frame duration, at most a two-key hold per curve) or real
            // motion that the catalog failed to cover.
            let signatures: Vec<_> = file
                .clips
                .iter()
                .map(|agr_clip| crate::inspector::animation::hxd::HxdClipSignature {
                    source_size: agr_clip.source_size,
                    duration_s: agr_clip.duration_s,
                })
                .collect();
            let names = hxd
                .as_ref()
                .map(|record| record.sequence_names_for_agr(stem, &signatures))
                .unwrap_or_default();
            let named = names.iter().filter(|name| !name.is_empty()).count();
            let mut unnamed_stubs = 0usize;
            let mut unnamed_micro = 0usize;
            let mut unnamed_motion = 0usize;
            for (index, agr_clip) in file.clips.iter().enumerate() {
                if names.get(index).is_some_and(|name| !name.is_empty()) {
                    continue;
                }
                let tracks_empty = agr_clip.tracks.is_empty();
                let max_keys = agr_clip
                    .tracks
                    .iter()
                    .map(|track| track.keys.len())
                    .max()
                    .unwrap_or(0);
                if !tracks_empty && max_keys <= 2 {
                    // A held pose: every curve is a single hold pair.
                    unnamed_stubs += 1;
                } else if agr_clip.duration_s <= 2.0 / 30.0 {
                    // A two-frame micro-clip with a token extra key.
                    unnamed_micro += 1;
                } else {
                    unnamed_motion += 1;
                }
            }
            if named < file.clips.len() {
                naming_gaps.push(format!(
                    "{agr_name} | {model_name} | hxd={} | named={named}/{} stub={unnamed_stubs} micro={unnamed_micro} motion={unnamed_motion}",
                    if hxd.is_some() { "yes" } else { "no" },
                    file.clips.len(),
                ));
                if hxd.is_none() {
                    naming_no_record += 1;
                } else if named == 0 {
                    naming_zero_named += 1;
                } else {
                    naming_partial += 1;
                }
            }

            let placeholder = model.root_motion_node;
            let placeholder_source = placeholder
                .and_then(|id| model.source_name(id))
                .unwrap_or("-");
            let wrappers_before = placeholder.map_or(0, |root| {
                model
                    .nodes
                    .iter()
                    .filter(|node| {
                        node.id.0 < root.0
                            && node.mesh.is_none()
                            && !node.name.eq_ignore_ascii_case("Scene Root")
                            && !descends_from(model, node.id, root)
                    })
                    .count()
            });
            let first_child = placeholder.and_then(|root| {
                model
                    .nodes
                    .iter()
                    .find(|node| {
                        node.mesh.is_none()
                            && node.id != root
                            && model.node(node.id).and_then(|child| child.parent) == Some(root)
                    })
                    .map(|node| node.id)
            });
            let curve0_index = clip
                .tracks
                .iter()
                .position(|track| track.target == "track_000");
            let curve0_node = curve0_index.and_then(|index| binding.node_for_track(index));
            let contract_match = curve0_node.is_some() && curve0_node == first_child;

            let rest_matches = target_rest_matches(model, &library);
            let mut confident = 0usize;
            let mut violations: Vec<String> = Vec::new();
            for (target, best, second) in &rest_matches {
                let Some((best_node, best_angle)) = best else {
                    continue;
                };
                let second_angle = second.map(|(_, angle)| angle).unwrap_or(f32::INFINITY);
                if *best_angle > 5.0 || second_angle - best_angle < 10.0 {
                    continue;
                }
                confident += 1;
                let bound = clip
                    .tracks
                    .iter()
                    .position(|track| &track.target == target)
                    .and_then(|index| binding.node_for_track(index));
                if bound != Some(*best_node) {
                    violations.push(format!(
                        "{target}->{}({:.1}deg) bound={}",
                        node_name(model, Some(*best_node)),
                        best_angle,
                        node_name(model, bound),
                    ));
                }
            }

            let skinned = model.has_skinning();
            let zup = model.source_orientation == BaseOrientation::Zup;
            let character = skinned && zup && placeholder_source.eq_ignore_ascii_case("Dummy");
            let offset = curve0_node.and_then(|id| track_of(model, id));
            let hidden = (0..model.meshes.len())
                .filter(|index| model.mesh_is_hidden(*index))
                .count();
            let row = format!(
                "{agr_name} | {model_name} | hxd={} v{} clips={} nodes={} meshes={} hidden={} skinned={} zup={} | placeholder={}({}) wraps={} | tracks={} bound={}/{} offset={} contract={} | names={named}/{} stub={unnamed_stubs} micro={unnamed_micro} motion={unnamed_motion} | conf={} viol={} {}",
                if hxd.is_some() { "yes" } else { "no" },
                file.variant,
                file.clip_count(),
                model.nodes.len(),
                model.meshes.len(),
                hidden,
                skinned,
                zup,
                node_name(model, placeholder),
                placeholder_source,
                wrappers_before,
                clip.tracks.len(),
                binding.bound_count(),
                binding.total_count(),
                offset.map_or("-".to_string(), |value| format!("+{value}")),
                if contract_match { "match" } else { "MISMATCH" },
                file.clips.len(),
                confident,
                violations.len(),
                violations.join(" "),
            );
            if character {
                character_rows.push(row.clone());
                if let Some(offset) = offset {
                    character_offsets.entry(offset).or_default().push(agr_name.clone());
                }
            }
            rows.push(row);
        }

        let mut report = String::new();
        report.push_str(&format!(
            "AGR corpus audit — {total} .agr entries, {paired} paired, {failed} unpaired/failed\n"
        ));
        report.push_str(&format!(
            "character-shaped pairs (skinned+Zup+Dummy placeholder): {}\n",
            character_rows.len()
        ));
        for (offset, names) in &character_offsets {
            report.push_str(&format!(
                "  offset +{offset}: {} pair(s): {}\n",
                names.len(),
                names.join(", ")
            ));
        }
        report.push_str(&format!(
            "\nnaming coverage gaps (named < clips): {} pairs\n  no HXD record: {naming_no_record}\n  record but zero named: {naming_zero_named}\n  partially named: {naming_partial}\n",
            naming_gaps.len()
        ));
        for row in &naming_gaps {
            report.push_str("  ");
            report.push_str(row);
            report.push('\n');
        }
        report.push_str("\n== character-shaped rows ==\n");
        for row in &character_rows {
            report.push_str(row);
            report.push('\n');
        }
        report.push_str("\n== all rows ==\n");
        for row in &rows {
            report.push_str(row);
            report.push('\n');
        }
        std::fs::write("target/agr-corpus-audit.txt", &report).expect("write audit report");
        println!("{report}");
    }

    /// The AGR model build must read each shape's diffuse texture name —
    /// its own `NiTexturingProperty`, else the NIF-wide fallback — so the
    /// loader can resolve pixels and animated scenes render textured.
    #[test]
    fn model_build_reads_diffuse_texture_names() {
        use crate::inspector::nif::{
            BlockMeta, BlockPayload, Endian, Footer, NiNodeData, NiSourceTextureData,
            NiTexturingPropertyData, NiTriShapeData, NiTriShapeDataPayload, TexDesc, Triangle,
            Vector3,
        };

        let blocks: Vec<BlockMeta> = (0..5)
            .map(|_| BlockMeta {
                type_index: 0,
                type_name: String::new(),
                size: 0,
                offset: 0,
            })
            .collect();
        let shape = |properties: Vec<i32>| {
            Some(BlockPayload::NiTriShape(NiTriShapeData {
                properties,
                data_ref: 3,
                skin_instance_ref: -1,
                rotation: crate::inspector::nif::Matrix33::identity(),
                scale: 1.0,
                ..Default::default()
            }))
        };
        let payloads = vec![
            Some(BlockPayload::NiNode(NiNodeData {
                children: vec![1],
                rotation: crate::inspector::nif::Matrix33::identity(),
                scale: 1.0,
                ..Default::default()
            })),
            shape(vec![2]),
            Some(BlockPayload::NiTexturingProperty(NiTexturingPropertyData {
                base: Some(TexDesc {
                    source_ref: 4,
                    ..Default::default()
                }),
                ..Default::default()
            })),
            Some(BlockPayload::NiTriShapeData(NiTriShapeDataPayload {
                vertices: vec![
                    Vector3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    Vector3 {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    Vector3 {
                        x: 0.0,
                        y: 1.0,
                        z: 0.0,
                    },
                ],
                triangles: vec![Triangle {
                    v0: 0,
                    v1: 1,
                    v2: 2,
                }],
                ..Default::default()
            })),
            Some(BlockPayload::NiSourceTexture(NiSourceTextureData {
                file_name: Some("models/chair_d.tga".to_string()),
                ..Default::default()
            })),
        ];
        let nif = NifFile {
            header_line: String::new(),
            version: crate::inspector::nif::BULLY_NIF_VERSION,
            endian: Endian::Little,
            user_version: 0,
            strings: Vec::new(),
            block_types: Vec::new(),
            blocks,
            payloads,
            footer: Footer { roots: vec![0] },
        };
        let model = model_from_nif(&nif, "textured", "textured").expect("model builds");
        assert_eq!(model.meshes.len(), 1);
        assert_eq!(
            model.meshes[0].texture_name.as_deref(),
            Some("models/chair_d.tga")
        );

        // NIF-wide fallback: the same layout without the shape's own
        // texturing property still picks up the orphan source texture.
        let mut fallback = nif;
        fallback.payloads[1] = shape(Vec::new());
        let model = model_from_nif(&fallback, "fallback", "fallback").expect("model builds");
        assert_eq!(
            model.meshes[0].texture_name.as_deref(),
            Some("models/chair_d.tga")
        );
    }

    /// The model picker's candidate list must resolve the retail catalogs to
    /// archive NIF entries (players, peds, props) instead of exposing raw
    /// model names.
    #[test]
    fn catalog_model_candidates_resolve_when_available() {
        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let stream = std::path::Path::new(&stream);
        let Ok(dir_bytes) = std::fs::read(stream.join("World.dir")) else {
            return;
        };
        let entries: Vec<crate::archive::EntryInfo> = dir_bytes
            .chunks_exact(32)
            .map(|record| {
                let end = record[8..].iter().position(|byte| *byte == 0).unwrap_or(24);
                crate::archive::EntryInfo::new(
                    String::from_utf8_lossy(&record[8..8 + end]).into_owned(),
                )
            })
            .collect();
        let anim = stream.parent().expect("game root").join("Anim");
        let candidates =
            crate::inspector::animation::hxd::candidate_models(Some(&anim), &entries);
        assert!(
            candidates.len() > 50,
            "expected a catalog-sized candidate list, got {}",
            candidates.len()
        );
        assert!(
            candidates
                .iter()
                .any(|(_, name)| name.eq_ignore_ascii_case("PLAYER.nif")),
            "the player model must be selectable, got: {:?}",
            candidates
                .iter()
                .map(|(_, name)| name.as_str())
                .take(20)
                .collect::<Vec<_>>()
        );
        for (index, name) in &candidates {
            assert!(
                entries.get(*index).is_some(),
                "candidate {name} must map to an archive entry"
            );
        }
    }

    /// AGR playback texture resolution: the model's diffuse texture names
    /// must resolve to pixels through the shared three-tier resolver (NFT
    /// catalog → archive texture → loose file) with the model *stem* as the
    /// catalog key, so the animated scene and the Texture tab can show them.
    #[test]
    fn agr_textures_resolve_for_models_when_available() {
        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let stream = std::path::Path::new(&stream);
        let img_path = stream.join("World.img");
        let Ok(dir_bytes) = std::fs::read(stream.join("World.dir")) else {
            return;
        };
        // Offsets/sectors are in 2048-byte units, exactly as the v1 parser
        // stores them; `ArchiveTextureIndex::read` needs them to fetch bytes.
        let entries: Vec<crate::archive::EntryInfo> = dir_bytes
            .chunks_exact(32)
            .map(|record| {
                let end = record[8..].iter().position(|byte| *byte == 0).unwrap_or(24);
                let mut entry = crate::archive::EntryInfo::new(
                    String::from_utf8_lossy(&record[8..8 + end]).into_owned(),
                );
                entry.offset = u32::from_le_bytes(record[0..4].try_into().unwrap());
                entry.sector = u32::from_le_bytes(record[4..8].try_into().unwrap());
                entry
            })
            .collect();
        let Some(nif_bytes) = world_entry(stream, "PLAYER.nif") else {
            return;
        };
        let mut nif = NifFile::parse(&nif_bytes).expect("PLAYER.nif parses");
        nif.resolve_string_indices();
        let model = model_from_nif(&nif, "PLAYER", "PLAYER").expect("player model builds");
        let names: Vec<String> = model
            .meshes
            .iter()
            .filter_map(|mesh| mesh.texture_name.clone())
            .collect();
        assert!(
            !names.is_empty(),
            "player meshes must carry diffuse texture names"
        );
        let game_root = stream.parent().expect("game root");
        let ide_map =
            std::sync::Arc::new(crate::inspector::texture::IdeMap::build(game_root));
        let resolver = crate::ui::app::nif_texture_resolver(
            crate::inspector::texture::ArchiveTextureIndex::from_entries(&entries, Some(&img_path)),
            Some(ide_map),
            "PLAYER",
        );
        let resolved: Vec<&str> = names
            .iter()
            .filter(|name| resolver(name).is_some())
            .map(String::as_str)
            .collect();
        eprintln!("resolved {}/{}: {resolved:?}", resolved.len(), names.len());
        assert!(
            resolved.len() == names.len(),
            "every player diffuse texture must resolve to pixels, missing: {:?}",
            names
                .iter()
                .filter(|name| resolver(name).is_none())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn real_agr_parses_when_available() {
        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let stream_path = std::path::Path::new(&stream);
        let anim = if stream_path.join("Anim").is_dir() {
            stream_path.join("Anim")
        } else {
            stream_path
                .parent()
                .map(|parent| parent.join("Anim"))
                .unwrap_or_else(|| stream_path.join("Anim"))
        };
        // (name, expected clip count, first variant) — loose corpus, PC retail.
        let expected = [
            ("C_Player.agr", 439usize, 1002u32),
            ("Grap.agr", 59, 1002),
            ("MOT_CTRL.agr", 418, 1004),
            ("NPC_Cher.agr", 12, 1002),
        ];
        for (name, clips, first_variant) in expected {
            let Ok(bytes) = std::fs::read(anim.join(name)) else {
                continue;
            };
            let file = parse_agr(&bytes).unwrap_or_else(|error| {
                panic!("{name} must parse: {error}");
            });
            assert_eq!(file.variant, first_variant, "{name} variant");
            assert_eq!(file.clip_count(), clips, "{name} clip count");
            if name == "C_Player.agr" {
                let first = file.clips.first().expect("C_Player first clip");
                assert_eq!(first.tracks.len(), 35, "C_Player packed curve count");
                assert!(first.auxiliary_records > 0);
                assert_eq!(
                    first.auxiliary_tail.len(),
                    first.auxiliary_records,
                    "1002 tail count must match preserved records"
                );
                assert!(first.tracks.iter().all(|track| track.channel == 0));
            }
            let library = to_library(&file, name);
            for clip in &library.clips {
                clip.validate()
                    .unwrap_or_else(|error| panic!("{name} clip must validate: {error}"));
            }
        }
    }

    #[test]
    fn linked_variants_decode_from_retail_player_when_available() {
        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let stream_path = std::path::Path::new(&stream);
        let anim = if stream_path.join("Anim").is_dir() {
            stream_path.join("Anim")
        } else {
            stream_path
                .parent()
                .map(|parent| parent.join("Anim"))
                .unwrap_or_else(|| stream_path.join("Anim"))
        };
        let Ok(bytes) = std::fs::read(anim.join("C_Player.agr")) else {
            return;
        };
        let file = parse_agr(&bytes).expect("C_Player.agr parses");
        let linked: Vec<&AgrClip> = file
            .clips
            .iter()
            .filter(|clip| matches!(clip.variant, 1000 | 1001))
            .collect();
        assert!(!linked.is_empty(), "C_Player contains linked object clips");
        assert!(linked.iter().any(|clip| clip.variant == 1000));
        assert!(linked.iter().any(|clip| clip.variant == 1001));

        // These three clips provide fixed raw-size sentinels for the two
        // descriptor families. The expected sizes include only the declared
        // rotation stream and admitted sparse rows, not any file padding.
        for (index, variant, metadata, source_size) in [
            (13usize, 1001u32, 7usize, 4_660usize),
            (377, 1000, 25, 9_260),
            (438, 1000, 12, 4_952),
        ] {
            let clip = file
                .clips
                .get(index)
                .unwrap_or_else(|| panic!("C_Player clip {index} exists"));
            assert_eq!(clip.variant, variant, "clip {index} variant");
            assert_eq!(clip.auxiliary_records, metadata, "clip {index} metadata");
            assert_eq!(clip.source_size, source_size, "clip {index} logical size");
        }

        for clip in linked {
            let expected_size = if clip.variant == 1000 { 20 } else { 12 };
            assert_eq!(clip.record_size, expected_size, "clip {}", clip.index);
            assert!(clip.auxiliary_records > 0, "clip {} metadata", clip.index);
            assert!(
                clip.tracks.iter().any(|track| track.channel == 0),
                "clip {} rotation tracks",
                clip.index
            );
            assert!(
                clip.tracks.iter().any(|track| track.channel == 1),
                "clip {} sparse translation tracks",
                clip.index
            );
            assert!(clip.diagnostics.iter().all(|diagnostic| {
                !diagnostic.contains("not yet decoded")
                    && !diagnostic.contains("field layout not yet decoded")
            }));
            for track in &clip.tracks {
                for key in &track.keys {
                    assert!(key.time_s.is_finite());
                    assert!(key.time_s >= 0.0 && key.time_s <= clip.duration_s);
                    assert!(key.values.iter().all(|value| value.is_finite()));
                }
            }
        }
    }

    fn push_packed_1004_words(out: &mut Vec<u8>, word0: u32, word1: u32, word2: u32) {
        push_u32(out, word0);
        push_u32(out, word1);
        push_u32(out, word2);
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
        // 1004 clip: default root, one linked transform curve, and a
        // two-record identity sentinel. The final word is deliberately zero:
        // it is valid record data, not archive padding.
        push_u32(&mut out, AGR_MAGIC);
        push_u32(&mut out, 1004);
        push_u32(&mut out, 5);
        push_u32(&mut out, 0);
        push_f32(&mut out, 0.2);
        let identity = 1023u32 << 22;
        push_packed_1004_words(&mut out, 0, identity, 0); // default root
        push_packed_1004_words(&mut out, 0, identity, 100); // curve root, t=0, x=1m
        push_packed_1004_words(&mut out, 1 | (511 << 11), identity, 200); // x=2m
        push_packed_1004_words(&mut out, 0, identity, 0); // sentinel root
        push_packed_1004_words(&mut out, 3 | (511 << 11), identity, 0); // sentinel end
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

        // 1004: predecessor links form one transform curve. The default root
        // and terminal identity sentinel are not exposed as animated tracks.
        let clip2 = &file.clips[2];
        assert_eq!(clip2.variant, 1004);
        let rot0 = clip2
            .tracks
            .iter()
            .find(|t| t.track == 0 && t.channel == 0)
            .expect("1004 track 0 rotation");
        assert_eq!(rot0.keys.len(), 2);
        assert!((rot0.keys[0].time_s - 0.0).abs() < 1e-4);
        assert!((rot0.keys[1].time_s - 0.2).abs() < 1e-4);
        let trans1 = clip2
            .tracks
            .iter()
            .find(|t| t.track == 0 && t.channel == 1)
            .expect("1004 track 0 translation");
        assert_eq!(trans1.keys.len(), 2);
        assert!((trans1.keys[0].values[0] - 1.0).abs() < 1e-4);
        assert!((trans1.keys[1].values[0] - 2.0).abs() < 1e-4);
        assert!(clip2.diagnostics.iter().any(|d| d.contains("sentinel")));
    }

    #[test]
    fn linked_1000_and_1001_decode_with_sparse_translations() {
        let mut data = Vec::new();

        // Variant 1000 stores full-float quaternions and a 16-byte sparse
        // translation table. The last translation deliberately ends in zero;
        // its non-zero record index must still keep the row in the logical
        // chunk instead of allowing a zero-trim heuristic to discard it.
        push_u32(&mut data, AGR_MAGIC);
        push_u32(&mut data, 1000);
        push_u32(&mut data, 4);
        push_u32(&mut data, 0);
        push_f32(&mut data, 1.0);
        push_linked_1000_record(&mut data, 0, 0, 1.0, 0.0, 0.0, 0.0); // default root
        push_linked_1000_record(&mut data, 0, 0, 1.0, 0.0, 0.0, 0.0); // curve root
        push_linked_1000_record(
            &mut data,
            1,
            u16::MAX,
            0.8660254,
            0.5,
            0.0,
            0.0,
        );
        push_linked_1000_record(&mut data, 0, u16::MAX, 1.0, 0.0, 0.0, 0.0); // sentinel
        push_1000_translation(&mut data, 1, [1.0, -0.5, 0.0]);
        push_1000_translation(&mut data, 2, [2.0, 0.0, 0.0]);
        push_1000_translation(&mut data, 0, [0.0, 0.0, 0.0]); // archive padding
        push_1000_translation(&mut data, 4, [9.0, 9.0, 9.0]); // invalid index, ignored
        push_1000_translation(&mut data, 1, [8.0, 8.0, 8.0]); // after padding, ignored

        // Add the second chunk after the first. This also proves that the
        // metadata scan is bounded by the next AGR header rather than
        // consuming later chunks as translation rows.
        push_u32(&mut data, AGR_MAGIC);
        push_u32(&mut data, 1001);
        push_u32(&mut data, 4);
        push_u32(&mut data, 0);
        push_f32(&mut data, 2.0);
        push_linked_1001_record(&mut data, 0, 0, 0, 0, 0, 32767); // default root
        push_linked_1001_record(&mut data, 0, 0, 0, 0, 0, 32767); // curve root
        push_linked_1001_record(&mut data, 1, u16::MAX, 16384, 0, 0, 28377);
        push_linked_1001_record(&mut data, 0, u16::MAX, 0, 0, 0, 32767); // sentinel
        push_1001_translation(&mut data, 1, [1000, -500, 0]);
        push_1001_translation(&mut data, 2, [2000, 0, 0]);
        push_1001_translation(&mut data, 0, [0, 0, 0]); // archive padding
        push_1001_translation(&mut data, 1, [8000, 8000, 8000]); // after padding, ignored

        let file = parse_agr(&data).expect("linked variants parse");
        assert_eq!(file.clip_count(), 2);

        let float_clip = &file.clips[0];
        assert_eq!(float_clip.variant, 1000);
        assert_eq!(float_clip.record_size, 20);
        assert_eq!(float_clip.auxiliary_records, 2);
        assert_eq!(float_clip.source_size, 20 + 4 * 20 + 2 * 16);
        let float_rotation = float_clip
            .tracks
            .iter()
            .find(|track| track.track == 0 && track.channel == 0)
            .expect("1000 rotation track");
        assert_eq!(float_rotation.keys.len(), 2);
        assert!((float_rotation.keys[1].time_s - 1.0).abs() < 1e-5);
        let float_q = float_rotation.keys[1].rotation();
        assert!((float_q.x - 0.5).abs() < 1e-4);
        assert!((float_q.w - 0.8660254).abs() < 1e-4);
        let float_translation = float_clip
            .tracks
            .iter()
            .find(|track| track.track == 0 && track.channel == 1)
            .expect("1000 translation track");
        assert_eq!(float_translation.keys.len(), 2);
        assert_eq!(float_translation.keys[1].values, [2.0, 0.0, 0.0]);
        assert!(float_clip
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("decoded 1 linked 1000")));
        assert!(float_clip
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("sentinel")));
        assert!(float_clip
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("malformed 1000")));

        let compact_clip = &file.clips[1];
        assert_eq!(compact_clip.variant, 1001);
        assert_eq!(compact_clip.record_size, 12);
        assert_eq!(compact_clip.auxiliary_records, 2);
        assert_eq!(compact_clip.source_size, 20 + 4 * 12 + 2 * 8);
        let compact_rotation = compact_clip
            .tracks
            .iter()
            .find(|track| track.track == 0 && track.channel == 0)
            .expect("1001 rotation track");
        assert_eq!(compact_rotation.keys.len(), 2);
        assert!((compact_rotation.keys[1].time_s - 2.0).abs() < 1e-5);
        let compact_q = compact_rotation.keys[1].rotation();
        assert!((compact_q.x - 0.5).abs() < 1e-3);
        assert!((compact_q.w - 0.8660254).abs() < 1e-3);
        let compact_translation = compact_clip
            .tracks
            .iter()
            .find(|track| track.track == 0 && track.channel == 1)
            .expect("1001 translation track");
        assert_eq!(compact_translation.keys.len(), 2);
        assert_eq!(compact_translation.keys[0].values, [1.0, -0.5, 0.0]);
        assert_eq!(compact_translation.keys[1].values, [2.0, 0.0, 0.0]);
        assert!(compact_clip
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("decoded 1 linked 1001")));
        assert!(compact_clip
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("sentinel")));
        assert!(compact_clip
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("malformed 1001")));
    }

    #[test]
    fn packed_1004_matches_retail_bitfields() {
        // SK8Board.agr GIV record 1, captured from the retail corpus.
        let record = [
            0x00, 0x00, 0x30, 0x09, 0xc0, 0xca, 0xb6, 0x1b, 0x0f, 0xe4, 0x82, 0x06,
        ];
        let key = decode_packed_object_key_1004(&record);
        assert_eq!(key.rotation_key.previous, 0);
        assert_eq!(key.rotation_key.time_code, 0);
        assert!((key.rotation_key.rotation[0] - (-0.0713587)).abs() < 1e-4);
        assert!((key.rotation_key.rotation[1] - 0.688172).abs() < 1e-4);
        assert!((key.rotation_key.rotation[2] - 0.71261).abs() < 1e-4);
        assert!((key.rotation_key.rotation[3] - (-0.107527)).abs() < 1e-4);
        assert!((key.translation[0] - (-0.15)).abs() < 1e-6);
        assert!((key.translation[1] - 0.92).abs() < 1e-6);
        assert!((key.translation[2] - 0.26).abs() < 1e-6);
    }

    #[test]
    fn packed_1002_reuses_retail_rotation_bitfields() {
        // The first two words of the captured SK8Board 1004 key are also the
        // complete 1002 key representation. This guards the shared decoder
        // against accidentally restoring the retired byte/channel layout.
        let record = [0x00, 0x00, 0x30, 0x09, 0xc0, 0xca, 0xb6, 0x1b];
        let key = decode_packed_rotation_key(&record);
        assert_eq!(key.previous, 0);
        assert_eq!(key.time_code, 0);
        assert!((key.rotation[0] - (-0.0713587)).abs() < 1e-4);
        assert!((key.rotation[1] - 0.688172).abs() < 1e-4);
        assert!((key.rotation[2] - 0.71261).abs() < 1e-4);
        assert!((key.rotation[3] - (-0.107527)).abs() < 1e-4);
    }

    #[test]
    fn malformed_1004_links_are_bounded_and_reported() {
        let mut data = Vec::new();
        push_u32(&mut data, AGR_MAGIC);
        push_u32(&mut data, 1004);
        push_u32(&mut data, 3);
        push_u32(&mut data, 0);
        push_f32(&mut data, 1.0);
        let identity = 1023u32 << 22;
        push_packed_1004_words(&mut data, 0, identity, 0);
        push_packed_1004_words(&mut data, 3, identity, 0); // forward/out-of-range link
        push_packed_1004_words(&mut data, 511 << 11, identity, 0); // terminal root

        let file = parse_agr(&data).expect("malformed links remain parseable");
        let clip = &file.clips[0];
        assert!(clip.tracks.is_empty());
        assert!(
            clip.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("invalid 1004 predecessor"))
        );
    }

    #[test]
    fn malformed_1002_links_are_bounded_and_reported() {
        let mut data = Vec::new();
        push_u32(&mut data, AGR_MAGIC);
        push_u32(&mut data, 1002);
        push_u32(&mut data, 3);
        push_u32(&mut data, 0);
        push_f32(&mut data, 1.0);
        let identity = 1023u32 << 22;
        push_packed_1002_words(&mut data, 0, identity);
        push_packed_1002_words(&mut data, 3, identity); // forward/out-of-range link
        push_packed_1002_words(&mut data, 511 << 11, identity); // terminal root

        let file = parse_agr(&data).expect("malformed links remain parseable");
        let clip = &file.clips[0];
        assert!(clip.tracks.is_empty());
        assert!(
            clip.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("invalid 1002 predecessor"))
        );
    }

    #[test]
    fn partition_weights_follow_both_palette_indirections() {
        let partition = crate::inspector::nif::NiSkinPartitionPayload {
            partitions: vec![crate::inspector::nif::SkinPartition {
                num_vertices: 2,
                num_bones: 3,
                num_weights_per_vertex: 2,
                bones: vec![4, 1, 3],
                vertex_map: vec![2, 0],
                weights: vec![vec![0.75, 0.25], vec![1.0, 0.0]],
                bone_indices: vec![vec![1, 2], vec![0, 1]],
                ..Default::default()
            }],
        };
        let weights = partition_skin_weights(&partition, 3, 5).expect("valid weights");
        assert_eq!(weights[0].as_slice(), &[(4, 1.0)]);
        assert!(weights[1].is_empty());
        assert_eq!(weights[2].as_slice(), &[(1, 0.75), (3, 0.25)]);
    }

    #[test]
    fn conflicting_partition_overlap_is_rejected() {
        let make = |weight| crate::inspector::nif::SkinPartition {
            num_vertices: 1,
            num_bones: 2,
            num_weights_per_vertex: 1,
            bones: vec![0, 1],
            vertex_map: vec![0],
            weights: vec![vec![weight]],
            bone_indices: vec![vec![u8::from(weight < 0.75)]],
            ..Default::default()
        };
        let partition = crate::inspector::nif::NiSkinPartitionPayload {
            partitions: vec![make(1.0), make(0.5)],
        };
        let error = partition_skin_weights(&partition, 1, 2).unwrap_err();
        assert!(error.contains("disagrees with an earlier partition"));
    }

    #[test]
    fn malformed_partition_tables_fail_closed() {
        let partition = crate::inspector::nif::NiSkinPartitionPayload {
            partitions: vec![crate::inspector::nif::SkinPartition {
                num_vertices: 1,
                num_bones: 1,
                num_weights_per_vertex: 1,
                bones: vec![0],
                vertex_map: vec![0],
                weights: vec![vec![1.0]],
                bone_indices: Vec::new(),
                ..Default::default()
            }],
        };
        assert!(partition_skin_weights(&partition, 1, 1).is_err());
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

            // End-to-end naming: the HXD catalog renames every clip in order.
            let anim = stream.parent().expect("game root").join("Anim");
            if let Some(record) = crate::inspector::animation::hxd::find_for_stem(&anim, "SK8Board")
            {
                assert_eq!(record.sequences.len(), file.clip_count());
                let mut library = to_library(&file, "SK8Board.agr");
                let named = apply_hxd_names(&mut library, &record);
                assert_eq!(named, 17);
                assert_eq!(library.clips[0].name, "IDLE_SK8BOARD");
                assert_eq!(library.clips[14].name, "SK8_GIV_O");
                assert_eq!(library.clips[15].name, "1_07_PICKUP");
                assert_eq!(library.clips[16].name, "SK8_EXAMINE_O");
            }
        }

        // Mission AGRs exercise the 1002 declared-stream/auxiliary-tail
        // boundary. The first three chunks are known retail samples with
        // non-zero auxiliary tails; only the declared records may contribute
        // curves.
        if let Some(bytes) = world_entry(stream, "1_07_Sk8Board.agr") {
            let file = parse_agr(&bytes).expect("1_07_Sk8Board.agr parses");
            assert_eq!(file.clip_count(), 3);
            assert!(file.clips.iter().all(|clip| clip.variant == 1002));
            assert_eq!(
                file.clips
                    .iter()
                    .map(|clip| clip.auxiliary_records)
                    .collect::<Vec<_>>(),
                vec![11, 18, 23]
            );
            assert!(file.clips.iter().all(|clip| {
                !clip.tracks.is_empty()
                    && clip.tracks.iter().all(|track| track.channel == 0)
                    && !clip
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.contains("invalid 1002 predecessor"))
            }));
        }

        // Exercise different root counts and curve lengths from the archive
        // corpus. This catches accidental dependence on the SK8Board shape
        // (one moving curve plus the default/sentinel roots).
        for name in [
            "AsyGate.agr",
            "Armor.agr",
            "Bike.agr",
            "1_02_MeetWithGary.agr",
        ] {
            let Some(bytes) = world_entry(stream, name) else {
                continue;
            };
            let file = parse_agr(&bytes).unwrap_or_else(|error| {
                panic!("{name} must parse: {error}");
            });
            let object_clips: Vec<&AgrClip> = file
                .clips
                .iter()
                .filter(|clip| clip.variant == 1004)
                .collect();
            assert!(!object_clips.is_empty(), "{name} has a 1004 clip");
            for clip in object_clips {
                assert!(!clip.tracks.is_empty(), "{name} clip has tracks");
                assert!(
                    !clip
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.contains("invalid 1004 predecessor")),
                    "{name} clip diagnostics: {:?}",
                    clip.diagnostics
                );
                for track in &clip.tracks {
                    for key in &track.keys {
                        assert!(key.time_s.is_finite());
                        assert!(key.time_s >= 0.0 && key.time_s <= clip.duration_s);
                    }
                }
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

    /// Same-stem AGR/NIF pairs exercise the object binding contract without
    /// relying on the player-specific calibration offset. The AGR root order
    /// is expected to match the NIF importer’s BonesOnly order for these
    /// standalone props, so this uses the exact runtime binding path first.
    #[test]
    fn object_1004_curve_roots_bind_across_matching_rigs_when_available() {
        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let stream = std::path::Path::new(&stream);
        let cases = [
            ("AsyGate.agr", "AsyGate.nif"),
            ("Armor.agr", "Armor.nif"),
            ("Bike.agr", "bike.nif"),
            ("SK8Board.agr", "SK8Board.nif"),
        ];
        let mut checked_assets = 0usize;
        let mut checked_clips = 0usize;

        for (agr_name, nif_name) in cases {
            let (Some(agr_bytes), Some(nif_bytes)) = (
                world_entry(stream, agr_name),
                world_entry(stream, nif_name),
            ) else {
                continue;
            };
            let file = parse_agr(&agr_bytes).unwrap_or_else(|error| {
                panic!("{agr_name} must parse: {error}");
            });
            let object_clips: Vec<&AgrClip> = file
                .clips
                .iter()
                .filter(|clip| clip.variant == 1004)
                .collect();
            if object_clips.is_empty() {
                continue;
            }

            let mut nif = NifFile::parse(&nif_bytes)
                .unwrap_or_else(|error| panic!("{nif_name} must parse: {error:?}"));
            nif.resolve_string_indices();
            let model = model_from_nif(&nif, nif_name, format!("test:{nif_name}"))
                .unwrap_or_else(|error| panic!("{nif_name} model admission: {error}"));
            let library = to_library(&file, agr_name);
            let mut asset_clips = 0usize;
            for clip in object_clips {
                let runtime_clip = library
                    .clips
                    .get(clip.index)
                    .expect("runtime clip follows source clip order");
                let binding =
                    crate::inspector::animation::binding::bind_clip(&model, runtime_clip);
                assert!(
                    binding.is_fully_bound(),
                    "{agr_name} clip {} did not bind to {nif_name}: {:?}",
                    clip.index,
                    binding.diagnostics
                );
                assert!(
                    clip.tracks.iter().any(|track| track.channel == 0),
                    "{agr_name} clip {} has no rotation channel",
                    clip.index
                );
                assert!(
                    clip.tracks.iter().any(|track| track.channel == 1),
                    "{agr_name} clip {} has no translation channel",
                    clip.index
                );
                checked_clips += 1;
                asset_clips += 1;
            }
            if asset_clips > 0 {
                checked_assets += 1;
            }
        }

        assert!(
            checked_assets > 0 && checked_clips > 0,
            "matching 1004 AGR/NIF corpus pairs were not available"
        );
    }

    /// Real-corpus regression for the wrapper-heavy Mandy rig. The AGR
    /// curves start at the NIF `Dummy` node rather than the first visible
    /// wrapper, and the NIF also contains two editor helper meshes. This
    /// verifies the complete admission path: right-side curves reach the
    /// skinned body, helpers do not affect the scene, and the grounded
    /// centered pose touches the viewer floor without moving along depth.
    #[test]
    fn mandy_puke_grounding_and_right_arm_when_available() {
        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let stream = std::path::Path::new(&stream);
        let (Some(agr_bytes), Some(nif_bytes)) = (
            world_entry(stream, "1_08_MandPuke.agr"),
            world_entry(stream, "JKGirl_Mandy.nif"),
        ) else {
            return;
        };

        let mut nif = NifFile::parse(&nif_bytes).expect("Mandy NIF parses");
        nif.resolve_string_indices();
        let model = model_from_nif(&nif, "Mandy", "Mandy").expect("Mandy model builds");
        let file = parse_agr(&agr_bytes).expect("Mandy AGR parses");
        let library = to_library(&file, "1_08_MandPuke.agr");
        let calibration = crate::inspector::animation::binding::calibrate_bindings(
            &model, &library,
        );

        let root = model.root_motion_node.expect("Mandy has a motion root");
        assert_eq!(model.source_name(root), Some("Dummy"));
        assert!(model.mesh_is_hidden(1), "NIF axis helper must be hidden");
        assert!(model.mesh_is_hidden(2), "NIF arrow helper must be hidden");

        let clip = library
            .clips
            .iter()
            .find(|clip| clip.name == "clip_02")
            .expect("Mandy loop clip");
        let binding = crate::inspector::animation::binding::bind_clip_with_calibration(
            &model,
            clip,
            &calibration,
        );
        assert_eq!(binding.bound_count(), clip.tracks.len());

        // The wrapper and body branches shift the normalized NIF numbering;
        // the AGR stream still lists the skeleton from `Root` (never the
        // `Dummy` placeholder), so every curve binds at the validated `+3`
        // offset. The rigid facial chain carries unique ~0-degree rest
        // matches at exactly this offset and pins the stream against the
        // placeholder-inclusive reading that twisted the skinned body.
        for (track_index, track) in clip.tracks.iter().enumerate() {
            let curve = track
                .target
                .strip_prefix("track_")
                .and_then(|value| value.parse::<u32>().ok())
                .expect("numeric AGR target");
            let expected = model
                .node_by_name(&format!("track_{:03}", curve + 3))
                .expect("Mandy calibrated node")
                .id;
            assert_eq!(
                binding
                    .node_for_track(track_index)
                    .expect("curve is bound"),
                expected,
                "AGR {} must bind three nodes below the Dummy placeholder",
                track.target
            );
        }
        for (curve, node) in [
            (0usize, "track_003"),
            (14, "track_017"),
            (15, "track_018"),
            (16, "track_019"),
            (17, "track_020"),
            (34, "track_037"),
        ] {
            let target = format!("track_{curve:03}");
            let track_index = clip
                .tracks
                .iter()
                .position(|track| track.target == target)
                .expect("curve entry exists");
            let expected = model.node_by_name(node).expect("Mandy anchor node").id;
            assert_eq!(
                binding.node_for_track(track_index),
                Some(expected),
                "Mandy curve {curve} must bind to {node}"
            );
        }

        let rest = crate::inspector::animation::pose::rest_scene(&model);
        let display_offset = -Vec3::from_array(rest.aabb.center());
        let mut ground_buffers = crate::inspector::animation::pose::PoseBuffers::new(&model);
        let ground = crate::inspector::animation::pose::clip_ground_offset_with_display_offset(
            &model,
            clip,
            &binding,
            128,
            display_offset,
            &mut ground_buffers,
        );
        let view_offset = display_offset + model.source_to_view.transform_vector3(ground);
        let mut buffers = crate::inspector::animation::pose::PoseBuffers::new(&model);
        let mut lowest = f32::INFINITY;
        for step in 0..128 {
            let time = clip.duration * step as f32 / 127.0;
            crate::inspector::animation::pose::sample_locals(
                clip,
                &binding,
                &model,
                time,
                &mut buffers.locals,
            );
            crate::inspector::animation::pose::evaluate_pose(
                &model,
                crate::inspector::animation::pose::RootMotionPolicy::Source,
                view_offset,
                &mut buffers,
            );
            let bounds = buffers.posed_bounds.expect("visible Mandy body bounds");
            lowest = lowest.min(bounds.min[1]);
            assert!(
                bounds.min[1] >= -1e-3,
                "grounded pose must not pass below the floor: {:?}",
                bounds
            );
        }
        assert!(
            lowest <= 1e-3,
            "grounded clip must reach the floor, lowest viewer Y was {lowest}"
        );

        let right_nodes: std::collections::HashSet<_> = (29..=36)
            .filter_map(|index| model.node_by_name(&format!("track_{index:03}")))
            .map(|node| node.id)
            .collect();
        let skin = model.meshes[0].skin.as_ref().expect("Mandy body skin");
        let right_slots: std::collections::HashSet<usize> = skin
            .joints
            .iter()
            .enumerate()
            .filter(|(_, node)| right_nodes.contains(node))
            .map(|(slot, _)| slot)
            .collect();
        assert!(!right_slots.is_empty(), "Mandy skin includes right arm joints");

        let right_arm = model.node_by_name("track_030").expect("right upper arm").id;
        let right_hand = model.node_by_name("track_032").expect("right hand").id;
        let mut sample_at = |time: f32| {
            crate::inspector::animation::pose::sample_locals(
                clip,
                &binding,
                &model,
                time,
                &mut buffers.locals,
            );
            crate::inspector::animation::pose::evaluate_pose(
                &model,
                crate::inspector::animation::pose::RootMotionPolicy::Source,
                view_offset,
                &mut buffers,
            );
            (
                buffers.node_positions_view[right_arm.0 as usize],
                buffers.node_positions_view[right_hand.0 as usize],
                buffers.out_vertices[0]
                    .iter()
                    .map(|vertex| Vec3::from_array(vertex.position))
                    .collect::<Vec<_>>(),
            )
        };
        let (upper_start, hand_start, vertices_start) = sample_at(0.0);
        let (upper_mid, hand_mid, vertices_mid) = sample_at(clip.duration * 0.5);
        assert!(
            upper_start.distance(upper_mid) > 1e-3
                || hand_start.distance(hand_mid) > 1e-3,
            "right arm nodes must respond to MANDY_PUKE_LOOP"
        );
        let mut affected = 0usize;
        let mut moved = 0usize;
        for (index, (start, mid)) in vertices_start.iter().zip(vertices_mid.iter()).enumerate() {
            if skin.weights[index]
                .iter()
                .any(|(slot, weight)| *weight > 1e-3 && right_slots.contains(&(*slot as usize)))
            {
                affected += 1;
                if start.distance(*mid) > 1e-4 {
                    moved += 1;
                }
            }
        }
        assert!(affected > 0, "right arm has weighted body vertices");
        assert!(
            moved > 0,
            "right arm weighted vertices must deform with its AGR curves"
        );

        let scene = crate::inspector::animation::pose::scene_from_pose(&model, &buffers);
        assert!(scene.meshes[1].indices.is_empty());
        assert!(scene.meshes[2].indices.is_empty());
        assert_eq!(scene.total_triangles(), model.meshes[0].indices.len() / 3);
    }

    /// Regression (2026-09-15): the helper-mesh suppression hid any mesh
    /// named `Editable Poly`, but the wrapper-heavy ped NIFs use that 3ds Max
    /// default name for their body (a skinned 32-joint mesh), so the whole
    /// `+2`/`+3` class rendered empty until the rule narrowed to unskinned
    /// editor objects. This pins a `+2` pedestrian: body visible, stream
    /// skipping the placeholder, and every curve bound at `+2`.
    #[test]
    fn wrapper_ped_body_stays_visible_when_available() {
        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let stream = std::path::Path::new(&stream);
        let (Some(agr_bytes), Some(nif_bytes)) = (
            world_entry(stream, "IDLE_DOUT_A.agr"),
            world_entry(stream, "DOH3a_Gurney.nif"),
        ) else {
            return;
        };

        let mut nif = NifFile::parse(&nif_bytes).expect("Gurney NIF parses");
        nif.resolve_string_indices();
        let model = model_from_nif(&nif, "Gurney", "Gurney").expect("Gurney model builds");
        let file = parse_agr(&agr_bytes).expect("IDLE_DOUT_A.agr parses");
        let library = to_library(&file, "IDLE_DOUT_A.agr");
        let calibration =
            crate::inspector::animation::binding::calibrate_bindings(&model, &library);
        let clip = library.clips.first().expect("idle clip");
        let binding = crate::inspector::animation::binding::bind_clip_with_calibration(
            &model, clip, &calibration,
        );

        let root = model.root_motion_node.expect("Gurney motion root");
        assert_eq!(model.source_name(root), Some("Dummy"));
        assert_eq!(
            model.node(root).map(|node| node.name.as_str()),
            Some("track_001")
        );
        let body = model.mesh_node(0).expect("Gurney body node");
        assert_eq!(model.source_name(body), Some("Editable Poly"));
        assert!(
            !model.mesh_is_hidden(0),
            "the skinned `Editable Poly` body must stay visible"
        );
        assert!(
            model.meshes[0].texture_name.is_some(),
            "the body mesh must carry its diffuse texture name, got {:?}",
            model.meshes[0].texture_name
        );
        assert_eq!(binding.bound_count(), clip.tracks.len());
        for (track_index, track) in clip.tracks.iter().enumerate() {
            let curve = track
                .target
                .strip_prefix("track_")
                .and_then(|value| value.parse::<u32>().ok())
                .expect("numeric AGR target");
            let expected = model
                .node_by_name(&format!("track_{:03}", curve + 2))
                .map(|node| node.id);
            assert_eq!(
                binding.node_for_track(track_index),
                expected,
                "Gurney curve {} must bind two nodes below the Dummy placeholder",
                track.target
            );
        }
    }

    /// Regression (2026-09-15): `MAINPED.HXD` rows whose 32-byte name field
    /// carries a fused float-tail byte (`8MINISNOW\...`) or a multi-word
    /// namespace (`N2B DISHONERABLE\...`) failed descriptor validation
    /// because the copies were read from the found string position instead
    /// of the field start; the rows were dropped and one clip per file
    /// stayed unnamed (`VAULT_BAR`, `MINISNOW_HITSHVL`). The candidate scan
    /// must recover them without disturbing any other row.
    #[test]
    fn compound_fused_prefix_rows_recover_when_available() {
        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let stream = std::path::Path::new(&stream);
        let anim = stream.parent().expect("game root").join("Anim");
        for (stem, clip_count, first_name) in [
            ("N2B Dishonerable", 5usize, "VAULT_BAR"),
            ("W_snowshwl", 8, "MINISNOW_HITSHVL"),
        ] {
            let Some(record) =
                crate::inspector::animation::hxd::find_for_agr(&anim, stem)
            else {
                continue;
            };
            let Some(agr_bytes) = world_entry(stream, &format!("{stem}.agr")) else {
                continue;
            };
            let file = parse_agr(&agr_bytes).expect("AGR parses");
            assert_eq!(file.clip_count(), clip_count, "{stem} clip count");
            let signatures: Vec<_> = file
                .clips
                .iter()
                .map(|clip| crate::inspector::animation::hxd::HxdClipSignature {
                    source_size: clip.source_size,
                    duration_s: clip.duration_s,
                })
                .collect();
            let names = record.sequence_names_for_agr(stem, &signatures);
            assert_eq!(names.len(), clip_count, "{stem} names align to clips");
            assert_eq!(names[0], first_name, "{stem} first clip name");
            assert!(
                names.iter().all(|name| !name.is_empty()),
                "{stem} must be fully named: {names:?}"
            );
        }
    }

    /// Compound-resource naming may cover only part of an AGR: the
    /// `Area_GirlsDorm` resource lists nine sequences while the AGR carries
    /// fifteen chunks (six single-frame filler clips). Covered clips take
    /// their catalog names by ordered size alignment; the rest keep the
    /// positional `clip_NN` fallback instead of losing every name.
    #[test]
    fn compound_resource_partial_naming_when_available() {
        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let stream = std::path::Path::new(&stream);
        let anim = stream.parent().expect("game root").join("Anim");
        let Some(record) =
            crate::inspector::animation::hxd::find_for_agr(&anim, "Area_GirlsDorm")
        else {
            return;
        };
        let Some(agr_bytes) = world_entry(stream, "Area_GirlsDorm.agr") else {
            return;
        };
        let file = parse_agr(&agr_bytes).expect("Area_GirlsDorm.agr parses");
        assert_eq!(file.clip_count(), 15);
        let signatures: Vec<_> = file
            .clips
            .iter()
            .map(|clip| crate::inspector::animation::hxd::HxdClipSignature {
                source_size: clip.source_size,
                duration_s: clip.duration_s,
            })
            .collect();
        let names = record.sequence_names_for_agr("Area_GirlsDorm", &signatures);
        assert_eq!(names.len(), 15);
        assert_eq!(names[0], "STUDY_OUT");
        assert_eq!(names[4], "STUDY_IN");
        assert_eq!(names[6], "FM_INTOBED_LEFT");
        assert_eq!(names[7], "FM_INTOBED_RIGHT");
        assert_eq!(names[8], "FM_OUTOFBED_LEFT");
        assert_eq!(names[9], "FM_OUTOFBED_RIGHT");
        assert!(
            names[5].is_empty() && names[10..].iter().all(String::is_empty),
            "single-frame filler clips stay unnamed: {names:?}"
        );

        let mut library = to_library(&file, "Area_GirlsDorm.agr");
        assert_eq!(
            crate::inspector::animation::bully::apply_clip_names(&mut library, &names),
            9
        );
        assert_eq!(library.clips[0].name, "STUDY_OUT");
        assert_eq!(library.clips[5].name, "clip_05");
        assert_eq!(library.clips[14].name, "clip_14");
    }

    /// Regression (2026-09-15): a generalized wrapper offset bound the
    /// player's first curve to the `Dummy` placeholder whenever the pose
    /// calibration came up short. The ordinary importer contract must keep
    /// the exported one-node offset for the player family on every library.
    #[test]
    fn player_family_binding_keeps_the_imported_order_when_available() {
        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let stream = std::path::Path::new(&stream);
        let anim = stream.parent().expect("game root").join("Anim");
        let (Ok(agr_bytes), Some(nif_bytes)) = (
            std::fs::read(anim.join("C_Player.agr")),
            world_entry(stream, "PLAYER.nif"),
        ) else {
            return;
        };

        let mut nif = NifFile::parse(&nif_bytes).expect("PLAYER.nif parses");
        nif.resolve_string_indices();
        let model = model_from_nif(&nif, "player", "player").expect("player model builds");
        let file = parse_agr(&agr_bytes).expect("C_Player.agr parses");
        let library = to_library(&file, "C_Player.agr");
        let calibration =
            crate::inspector::animation::binding::calibrate_bindings(&model, &library);
        let clip = library.clips.first().expect("first player clip");
        let binding = crate::inspector::animation::binding::bind_clip_with_calibration(
            &model, clip, &calibration,
        );

        let dummy = model
            .node_by_name("track_000")
            .expect("player placeholder")
            .id;
        assert_eq!(model.source_name(dummy), Some("Dummy"));
        assert_eq!(
            binding.bound_count(),
            clip.tracks.len(),
            "every C_Player curve must bind"
        );
        for (track_index, node) in [
            (0usize, "track_001"),
            (8, "track_009"),
            (9, "track_010"),
            (34, "track_035"),
        ] {
            let expected = model.node_by_name(node).map(|node| node.id);
            assert_ne!(expected, Some(dummy), "sanity: {node} is not the placeholder");
            assert_eq!(
                binding.node_for_track(track_index),
                expected,
                "C_Player curve {track_index} must bind to {node}, not the Dummy placeholder"
            );
        }
    }

    /// Regression (2026-09-15): `Hang_Workout` is the action-only library
    /// whose root/torso curves never reach the bind pose. The ordinary
    /// importer contract must recover those tracks (`curve i -> track_(i+1)`)
    /// instead of binding the first curve to the `Dummy` placeholder.
    #[test]
    fn action_only_player_clips_recover_root_tracks_when_available() {
        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let stream = std::path::Path::new(&stream);
        let (Some(agr_bytes), Some(nif_bytes)) = (
            world_entry(stream, "Hang_Workout.agr"),
            world_entry(stream, "PLAYER.nif"),
        ) else {
            return;
        };

        let mut nif = NifFile::parse(&nif_bytes).expect("PLAYER.nif parses");
        nif.resolve_string_indices();
        let model = model_from_nif(&nif, "player", "player").expect("player model builds");
        let file = parse_agr(&agr_bytes).expect("Hang_Workout.agr parses");
        let library = to_library(&file, "Hang_Workout.agr");
        let calibration =
            crate::inspector::animation::binding::calibrate_bindings(&model, &library);
        let clip = library.clips.first().expect("first workout clip");
        let binding = crate::inspector::animation::binding::bind_clip_with_calibration(
            &model, clip, &calibration,
        );

        let dummy = model
            .node_by_name("track_000")
            .expect("player placeholder")
            .id;
        assert_eq!(
            binding.bound_count(),
            clip.tracks.len(),
            "action-only root/torso curves must recover through the importer contract"
        );
        for (track_index, node) in [
            (0usize, "track_001"),
            (8, "track_009"),
            (9, "track_010"),
            (34, "track_035"),
        ] {
            let expected = model.node_by_name(node).map(|node| node.id);
            assert_ne!(expected, Some(dummy), "sanity: {node} is not the placeholder");
            assert_eq!(
                binding.node_for_track(track_index),
                expected,
                "Hang_Workout curve {track_index} must bind to {node}"
            );
        }
    }

    /// An ordinary character rig outside the player family must keep the
    /// exported one-node offset (`RAT_PED`): curve 0 binds to `Root`, never
    /// to the `Dummy` placeholder, and every curve binds.
    #[test]
    fn ordinary_character_rig_keeps_the_exported_order_when_available() {
        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let stream = std::path::Path::new(&stream);
        let (Some(agr_bytes), Some(nif_bytes)) = (
            world_entry(stream, "RAT_PED.agr"),
            world_entry(stream, "rat_ped.nif"),
        ) else {
            return;
        };

        let mut nif = NifFile::parse(&nif_bytes).expect("rat_ped.nif parses");
        nif.resolve_string_indices();
        let model = model_from_nif(&nif, "rat", "rat").expect("rat model builds");
        let file = parse_agr(&agr_bytes).expect("RAT_PED.agr parses");
        let library = to_library(&file, "RAT_PED.agr");
        let calibration =
            crate::inspector::animation::binding::calibrate_bindings(&model, &library);
        let clip = library.clips.first().expect("first rat clip");
        let binding = crate::inspector::animation::binding::bind_clip_with_calibration(
            &model, clip, &calibration,
        );

        let dummy = model.node_by_name("track_000").expect("rat placeholder").id;
        assert_eq!(model.source_name(dummy), Some("Dummy"));
        assert_eq!(
            binding.bound_count(),
            clip.tracks.len(),
            "every RAT_PED curve must bind"
        );
        for (track_index, track) in clip.tracks.iter().enumerate() {
            let curve = track
                .target
                .strip_prefix("track_")
                .and_then(|value| value.parse::<u32>().ok())
                .expect("numeric rat target");
            let expected = model
                .node_by_name(&format!("track_{:03}", curve + 1))
                .map(|node| node.id);
            assert_ne!(expected, Some(dummy), "curve {curve} must skip the placeholder");
            assert_eq!(
                binding.node_for_track(track_index),
                expected,
                "RAT_PED curve {} must bind one node below the placeholder",
                track.target
            );
        }
    }

    /// Regression: index-based track naming binds AGR curves to neighbor
    /// bones on the player rig (export order differs from the NIF's DFS
    /// order), which twists the skinned mesh while bone positions stay
    /// plausible. Calibrated binding matches each curve's first key against
    /// node rest rotations; action-only clips such as Hang_Workout then use
    /// the verified Bully offset when their root/torso never reaches rest.
    #[test]
    fn calibrated_binding_matches_bind_pose_when_available() {
        let (Ok(agr_path), Ok(nif_path)) = (
            std::env::var("IMGEDITOR_AGR_DUMP_AGR"),
            std::env::var("IMGEDITOR_AGR_DUMP_NIF"),
        ) else {
            return;
        };
        let agr_bytes = std::fs::read(&agr_path).expect("read AGR");
        let nif_bytes = std::fs::read(&nif_path).expect("read NIF");
        let mut nif = crate::inspector::nif::NifFile::parse(&nif_bytes).expect("parse NIF");
        nif.resolve_string_indices();
        let model = model_from_nif(&nif, "calibration", "calibration").expect("model builds");
        let file = parse_agr(&agr_bytes).expect("parse AGR");
        let library = to_library(&file, "calibration");
        let clip = library
            .clips
            .first()
            .expect("at least one clip");
        let calibration =
            crate::inspector::animation::binding::calibrate_bindings(&model, &library);
        let binding =
            crate::inspector::animation::binding::bind_clip_with_calibration(
                &model, clip, &calibration,
            );
        let nif_stem = std::path::Path::new(&nif_path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        let agr_stem = std::path::Path::new(&agr_path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        if nif_stem.eq_ignore_ascii_case("PLAYER")
            && (agr_stem.eq_ignore_ascii_case("C_Player")
                || agr_stem.eq_ignore_ascii_case("Hang_Workout"))
        {
            assert_eq!(
                binding.bound_count(),
                35,
                "player AGR must bind every declared character curve"
            );
            for (track, expected_node) in [
                (0, "track_001"), // Root
                (1, "track_002"), // pelvis
                (2, "track_003"), // left thigh
                (3, "track_004"), // left calf
                (4, "track_005"), // left foot
                (5, "track_006"), // right thigh
                (6, "track_007"), // right calf
                (7, "track_008"), // right foot
                (34, "track_035"), // TranslationNode/ARROW attachment
            ] {
                assert_eq!(
                    binding.node_for_track(track),
                    model.node_by_name(expected_node).map(|node| node.id),
                    "C_Player curve {track} must bind to {expected_node}"
                );
            }
        }
        assert!(
            binding.bound_count() >= 33,
            "calibrated binding places nearly all curves (bound {} of {})",
            binding.bound_count(),
            binding.total_count()
        );

        // Pose vs misbind discrimination: a single clip's first frame may
        // legitimately pose a bone 150 deg from rest, but a *misbound* bone
        // never matches any clip. Take each bound node's minimum deviation
        // across the first several clips' t=0 poses.
        let mut locals = model.default_locals();
        let mut best: Vec<f32> = vec![f32::MAX; model.nodes.len()];
        for clip in library.clips.iter().take(8) {
            crate::inspector::animation::pose::sample_locals(
                clip,
                &binding,
                &model,
                0.0,
                &mut locals,
            );
            for (node_index, node) in model.nodes.iter().enumerate() {
                if binding
                    .tracks
                    .iter()
                    .any(|track| track.node == Some(node.id))
                {
                    let sampled = locals[node_index].rotation;
                    let rest = node.local.rotation;
                    let dot = (sampled.x * rest.x
                        + sampled.y * rest.y
                        + sampled.z * rest.z
                        + sampled.w * rest.w)
                        .abs()
                        .clamp(-1.0, 1.0);
                    let deviation = dot.acos().to_degrees() * 2.0;
                    best[node_index] = best[node_index].min(deviation);
                }
            }
        }
        let mut minima: Vec<(String, f32)> = best
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, v)| *v < f32::MAX)
            .map(|(index, v)| (model.nodes[index].name.clone(), v))
            .collect();
        minima.sort_by(|a, b| a.1.total_cmp(&b.1));
        eprintln!(
            "per-node minimum deviations over 8 clips: {}",
            minima
                .iter()
                .map(|(name, v)| format!("{name}={v:.1}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let median_min = minima[minima.len() / 2].1;
        // The Root/Pelvis/Root01 chain rests within a few degrees of each
        // other, so their curves are only partially separable by rest
        // matching; the order prior binds them, but a couple may stay
        // opposed in every sampled pose.
        let stuck = minima.iter().filter(|(_, angle)| *angle > 60.0).count();
        assert!(
            median_min < 15.0,
            "median per-node minimum deviation {median_min:.1} deg (n={})",
            minima.len()
        );
        assert!(
            stuck <= 4,
            "{stuck} bound bones never approach their rest pose across 8 clips: {}",
            minima
                .iter()
                .filter(|(_, angle)| *angle > 60.0)
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    /// Regression: rotation-only character clips pivot at standing height,
    /// so prone/lying clips (push-ups, ground states) hang their contact
    /// points in the air. `clip_ground_offset` returns one constant per
    /// clip that plants the lowest sampled excursion on the floor: small
    /// for standing clips, substantial for clips that reach for the floor.
    #[test]
    fn clip_ground_offset_plants_contact_points() {
        let (Ok(agr_path), Ok(nif_path)) = (
            std::env::var("IMGEDITOR_AGR_DUMP_AGR"),
            std::env::var("IMGEDITOR_AGR_DUMP_NIF"),
        ) else {
            return;
        };
        let agr_bytes = std::fs::read(&agr_path).expect("read AGR");
        let nif_bytes = std::fs::read(&nif_path).expect("read NIF");
        let mut nif = crate::inspector::nif::NifFile::parse(&nif_bytes).expect("parse NIF");
        nif.resolve_string_indices();
        let model = model_from_nif(&nif, "ground", "ground").expect("model builds");
        let file = parse_agr(&agr_bytes).expect("parse AGR");
        let library = to_library(&file, "ground");
        let calibration =
            crate::inspector::animation::binding::calibrate_bindings(&model, &library);
        let up = model.ground_normal_model();
        let mut buffers =
            crate::inspector::animation::pose::PoseBuffers::new(&model);
        let mut offset_of = |index: usize| {
            let clip = library.clips.get(index).expect("clip exists");
            let binding =
                crate::inspector::animation::binding::bind_clip_with_calibration(
                    &model, clip, &calibration,
                );
            crate::inspector::animation::pose::clip_ground_offset(
                &model, clip, &binding, 12, &mut buffers,
            )
        };
        // RUN dips to the floor within its cycle (a real foot plant).
        let run = offset_of(0);
        println!("RUN ground offset {run:?}");
        assert!(
            run.dot(up) > 0.2,
            "RUN must reach the floor within its cycle (offset {run:?})"
        );
        // GROUND_ONBACK goes prone/lying: the offset must plant the body.
        let ground = offset_of(8);
        println!("GROUND_ONBACK ground offset {ground:?}");
        assert!(
            ground.dot(up) > 0.7,
            "GROUND_ONBACK must plant its contact points (offset {ground:?})"
        );
    }

    #[test]
    fn skinned_ped_model_when_available() {
        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let stream = std::path::Path::new(&stream);
        let Some(bytes) = world_entry(stream, "PLAYER.nif") else {
            return;
        };
        let mut nif = crate::inspector::nif::NifFile::parse(&bytes).expect("PLAYER.nif parses");
        nif.resolve_string_indices();
        let model = model_from_nif(&nif, "PLAYER.nif", "PLAYER.nif").expect("model builds");

        // Four meshes are skinned in the retail ped, with partition weight
        // counts matching the corpus-verified unique shape-vertex totals.
        let skinned: Vec<&MeshAsset> = model.meshes.iter().filter(|m| m.skin.is_some()).collect();
        assert_eq!(skinned.len(), 4, "skinned mesh count");
        let influenced: usize = skinned
            .iter()
            .map(|mesh| {
                let skin = mesh.skin.as_ref().unwrap();
                assert_eq!(
                    skin.weights.len(),
                    mesh.vertices.len(),
                    "weights per vertex"
                );
                assert_eq!(skin.inverse_bind.len(), skin.joints.len(), "bind per joint");
                skin.weights
                    .iter()
                    .filter(|weights| !weights.is_empty())
                    .count()
            })
            .sum();
        assert_eq!(influenced, 2587, "influenced vertices on the ped");

        // `Scene Root` is not an animation target; track numbering starts at
        // the first skeleton node so all C_Player rotation curves bind.
        assert!(model.node_by_name("Scene Root").is_some());
        assert!(
            model
                .node_by_name("track_000")
                .is_some_and(|node| node.name != "Scene Root"),
            "track_000 must not be the scene root"
        );
        assert!(model.node_by_name("track_034").is_some());

        // Loose character AGR + compound HXD naming: C_Player clips get real
        // names and the ped model covers every rotation curve.
        let anim = stream.parent().expect("game root").join("Anim");
        let Ok(agr_bytes) = std::fs::read(anim.join("C_Player.agr")) else {
            return;
        };
        let file = parse_agr(&agr_bytes).expect("C_Player.agr parses");
        let signatures: Vec<_> = file
            .clips
            .iter()
            .map(|clip| crate::inspector::animation::hxd::HxdClipSignature {
                source_size: clip.source_size,
                duration_s: clip.duration_s,
            })
            .collect();
        let record =
            crate::inspector::animation::hxd::find_for_agr(&anim, "C_Player").expect("MAINPED");
        let names = record.sequence_names_for_agr("C_Player", &signatures);
        assert_eq!(names.len(), 439, "all C_Player clips named");
        assert_eq!(names[0], "RUN");
        assert_eq!(names[13], "IDLE");

        // Every animated rotation curve of a dense clip binds to a bone node.
        let clip = &file.clips[0];
        let library = to_library(&file, "C_Player");
        let binding = crate::inspector::animation::binding::bind_clip(&model, &library.clips[0]);
        assert_eq!(
            binding.bound_count(),
            clip.tracks.len(),
            "all packed rotation curves bind"
        );
    }

    #[test]
    fn direct_weight_skin_model_builds_when_available() {
        let Some(root) = crate::test_paths::bully_nif_tools() else {
            return;
        };
        let path = root.join("CS_PLAY.nif");
        let Ok(bytes) = std::fs::read(path) else {
            return;
        };
        let mut nif = NifFile::parse(&bytes).expect("parse direct-weight NIF");
        nif.resolve_string_indices();
        let model = model_from_nif(&nif, "CS_PLAY", "test:direct-weights").expect("model builds");
        let skinned: Vec<_> = model
            .meshes
            .iter()
            .filter_map(|mesh| mesh.skin.as_ref())
            .collect();
        assert_eq!(skinned.len(), 1);
        assert!(
            skinned[0].weights.iter().any(|weights| !weights.is_empty()),
            "NiSkinData weights must remain a functional partition fallback"
        );
    }

    #[test]
    #[ignore = "walks the full extracted Bully NIF corpus"]
    fn all_extracted_skin_bindings_validate_when_requested() {
        let Some(root) = crate::test_paths::bully_nif_tools() else {
            return;
        };
        let mut pending = vec![root];
        let mut files = 0usize;
        let mut skins = 0usize;
        while let Some(directory) = pending.pop() {
            let entries = std::fs::read_dir(&directory)
                .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                    continue;
                }
                if !path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("nif"))
                {
                    continue;
                }
                let bytes = std::fs::read(&path)
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
                let header = NifFile::parse_header(&bytes)
                    .unwrap_or_else(|error| panic!("header {}: {error}", path.display()));
                if !header
                    .blocks
                    .iter()
                    .any(|block| block.type_name == "NiSkinInstance")
                {
                    continue;
                }
                let nif = NifFile::parse(&bytes)
                    .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
                let skinned_shapes: Vec<(i32, i32)> = nif
                    .payloads
                    .iter()
                    .flatten()
                    .filter_map(|payload| match payload {
                        BlockPayload::NiTriShape(shape) if shape.skin_instance_ref >= 0 => {
                            Some((shape.skin_instance_ref, shape.data_ref))
                        }
                        BlockPayload::NiTriStrips(shape) if shape.base.skin_instance_ref >= 0 => {
                            Some((shape.base.skin_instance_ref, shape.base.data_ref))
                        }
                        _ => None,
                    })
                    .collect();
                if skinned_shapes.is_empty() {
                    continue;
                }
                for (skin_instance_ref, data_ref) in skinned_shapes {
                    let vertex_count = match nif
                        .payloads
                        .get(data_ref as usize)
                        .and_then(|payload| payload.as_ref())
                    {
                        Some(BlockPayload::NiTriShapeData(data)) => data.vertices.len(),
                        Some(BlockPayload::NiTriStripsData(data)) => data.base.vertices.len(),
                        _ => panic!("skinned geometry data is missing in {}", path.display()),
                    };
                    let mut diagnostics = Vec::new();
                    assert!(
                        build_pending_skin(
                            &nif,
                            skin_instance_ref,
                            vertex_count,
                            0,
                            &mut diagnostics
                        )
                        .is_some(),
                        "skin in {} failed validation: {diagnostics:?}",
                        path.display()
                    );
                    skins += 1;
                }
                files += 1;
            }
        }
        assert_eq!(skins, 2_613);
        assert!(files > 1_000, "expected the extracted retail skin corpus");
    }
}
