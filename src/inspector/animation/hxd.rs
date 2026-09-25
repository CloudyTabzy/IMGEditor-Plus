//! Minimal Bully HXD (animation hierarchy) reader.
//!
//! The `Anim/` folder carries two disjoint catalogs that map models to their
//! animation sequences (evidence: `bully-probe/FINDINGS.md` §3):
//!
//! - `hxds.dat` — 130 concatenated records for level objects (doors, gates,
//!   switches, props). Layout: `u32 count`, then records
//!   `ANIM` + `u32 body_len` + body.
//! - `Anim/<MODEL>.HXD` — one loose record body per actor/vehicle prop
//!   (`SK8BOARD`, `BIKE`, `MAINPED`, `MOT_CTRL`, ...). No header.
//!
//! A record body is a compiler-dumped structure (serialized pointers, `0xCD`
//! debug fill). Two things are extracted reliably:
//!
//! - the model name: the identifier following the last `01 00 00 00` marker;
//! - the sequence list: NUL-terminated names of the form `NAMESPACE\NAME`
//!   prefixed by `{ f32 duration, f32 weight }`. A name may be fused with a
//!   stray `>`/`*>` byte from the preceding float, so the reader searches
//!   inside each string for the `NS\NAME` pattern.
//! - `MAINPED.HXD`'s external AGR table and the duplicated source/size
//!   descriptor carried by each sequence. This identifies the owning AGR
//!   without relying on namespace names (one AGR can use many namespaces).
//!
//! Everything else in a record (joints, pose data, serialized pointers) is
//! intentionally ignored.

use std::path::{Path, PathBuf};

/// One animation sequence entry in an HXD record.
#[derive(Clone, Debug, PartialEq)]
pub struct HxdSequence {
    /// Full namespaced name (`SKATEBOARD\IDLE_SK8BOARD`).
    pub name: String,
    /// Authored duration in seconds.
    pub duration_s: f32,
    /// Authored blend weight (0..=1).
    pub weight: f32,
    /// AGR chunk size recorded by the catalog, including its 4-byte runtime
    /// trailer. Present when the duplicated sequence descriptors validate.
    pub encoded_size: Option<u32>,
    /// Index into [`HxdRecord::resources`]. Present when the duplicated
    /// sequence descriptors validate.
    pub source_index: Option<u32>,
}

impl HxdSequence {
    /// Leaf name without the namespace (`IDLE_SK8BOARD`).
    pub fn leaf_name(&self) -> &str {
        self.name.rsplit('\\').next().unwrap_or(&self.name)
    }
}

/// One external AGR referenced by a loose hierarchy catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HxdResource {
    /// AGR stem (`C_Player`, `Grap`, `NPC_Cher`, ...).
    pub name: String,
    /// Total size recorded by the compiler catalog. This can differ slightly
    /// from the retail loose file and is not used for admission.
    pub encoded_size: u32,
    /// Associated model resource, usually an MXD name (`player.mxd`).
    pub model: String,
}

/// AGR-side data needed to pair parsed chunks with HXD sequence rows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HxdClipSignature {
    /// Parsed AGR chunk bytes, excluding archive-sector padding.
    pub source_size: usize,
    pub duration_s: f32,
}

/// One parsed hierarchy record.
#[derive(Clone, Debug, PartialEq)]
pub struct HxdRecord {
    /// Model name (`SK8Board`, `AniBroom`, `RMailbox`, ...).
    pub model: String,
    /// Joint names (`joint0`..., or semantic names for peds).
    pub joints: Vec<String>,
    /// Ordered animation sequences. Ordinary HXD records map directly onto
    /// AGR chunks; compound catalogs require source/size alignment.
    pub sequences: Vec<HxdSequence>,
    /// External AGR resources. Populated by `MAINPED.HXD`; ordinary loose HXD
    /// files and `hxds.dat` records do not carry this table.
    pub resources: Vec<HxdResource>,
}

impl HxdRecord {
    /// Sequence leaf names in order.
    pub fn sequence_names(&self) -> Vec<String> {
        self.sequences
            .iter()
            .map(|sequence| sequence.leaf_name().to_string())
            .collect()
    }

    /// Resolve ordered sequence names for an AGR source.
    ///
    /// Ordinary HXD records use the historical exact-count mapping. Compound
    /// catalogs such as `MAINPED.HXD` first select rows by their resource
    /// index, then align them to AGR chunks by the duplicated encoded size.
    /// The alignment tolerates stale catalog-only rows and partial coverage:
    /// clips without an ordered size match keep an empty name so the caller
    /// falls back to the positional `clip_NN` label (`Area_GirlsDorm` has
    /// six single-frame filler chunks beyond its nine named sequences).
    pub fn sequence_names_for_agr(&self, source: &str, clips: &[HxdClipSignature]) -> Vec<String> {
        let source_index = self
            .resources
            .iter()
            .position(|resource| resource.name.eq_ignore_ascii_case(source));
        let Some(source_index) = source_index else {
            return if self.sequences.len() == clips.len() {
                self.sequence_names()
            } else {
                Vec::new()
            };
        };

        let candidates: Vec<&HxdSequence> = self
            .sequences
            .iter()
            .filter(|sequence| sequence.source_index == Some(source_index as u32))
            .collect();
        if candidates.is_empty() || clips.is_empty() {
            return Vec::new();
        }

        // Minimum-cost ordered alignment. Encoded size is the admission key;
        // duration only breaks ties when repeated chunk sizes make more than
        // one mapping possible. Skipping a clip or a catalog row costs a
        // fixed penalty, so covered runs still name correctly when the
        // resource lists fewer rows than the AGR has chunks.
        const MAX_FINAL_PADDING: u32 = 64;
        const MAX_ALIGNMENT_CELLS: usize = 4_000_000;
        const SKIP_COST: f64 = 25.0;
        let rows = clips.len() + 1;
        let columns = candidates.len() + 1;
        let Some(cells) = rows
            .checked_mul(columns)
            .filter(|cells| *cells <= MAX_ALIGNMENT_CELLS)
        else {
            return Vec::new();
        };
        let mut costs = vec![f64::INFINITY; cells];
        let mut choices = vec![0_u8; cells];
        for column in 0..columns {
            costs[clips.len() * columns + column] = 0.0;
        }
        // The boundary column (every catalog row consumed) can still skip
        // the remaining clips at the fixed penalty, so a tail match after
        // the last row stays reachable.
        for row in 0..clips.len() {
            costs[row * columns + candidates.len()] = (clips.len() - row) as f64 * SKIP_COST;
        }
        for row in (0..clips.len()).rev() {
            for column in (0..candidates.len()).rev() {
                let cell = row * columns + column;
                // Skip this catalog row.
                let mut best = costs[cell + 1];
                let mut choice = 2_u8;
                // Skip this clip when no catalog row fits it.
                let skip_clip = costs[(row + 1) * columns + column] + SKIP_COST;
                if skip_clip < best {
                    best = skip_clip;
                    choice = 1;
                }
                let expected_size = clips[row]
                    .source_size
                    .checked_add(4)
                    .and_then(|size| u32::try_from(size).ok());
                let size_delta = candidates[column].encoded_size.zip(expected_size).and_then(
                    |(catalog_size, parsed_size)| {
                        let delta = catalog_size.checked_sub(parsed_size)?;
                        (delta == 0
                            || (row + 1 == clips.len()
                                && delta <= MAX_FINAL_PADDING
                                && delta.is_multiple_of(4)))
                        .then_some(delta)
                    },
                );
                if let Some(size_delta) = size_delta {
                    let remainder = costs[(row + 1) * columns + column + 1];
                    if remainder.is_finite() {
                        let duration_delta = f64::from(
                            (candidates[column].duration_s - clips[row].duration_s).abs(),
                        );
                        // Exact sizes always beat the bounded final-padding
                        // fallback (kept small so padding never loses to a
                        // skip); duration then disambiguates equal-size rows.
                        let padding_cost = if size_delta == 0 { 0.0 } else { 0.5 };
                        let matched = remainder + padding_cost + duration_delta;
                        if matched <= best {
                            best = matched;
                            choice = 0;
                        }
                    }
                }
                costs[cell] = best;
                choices[cell] = choice;
            }
        }
        if !costs[0].is_finite() {
            return Vec::new();
        }

        let mut names = Vec::with_capacity(clips.len());
        let (mut row, mut column) = (0usize, 0usize);
        while row < clips.len() {
            if column >= candidates.len() {
                names.push(String::new());
                row += 1;
                continue;
            }
            match choices[row * columns + column] {
                0 => {
                    names.push(candidates[column].leaf_name().to_string());
                    row += 1;
                    column += 1;
                }
                1 => {
                    names.push(String::new());
                    row += 1;
                }
                _ => column += 1,
            }
        }
        names
    }

    /// Model resource associated with one external AGR, if catalogued.
    pub fn model_for_source(&self, source: &str) -> Option<&str> {
        self.resources
            .iter()
            .find(|resource| resource.name.eq_ignore_ascii_case(source))
            .map(|resource| resource.model.as_str())
            .filter(|model| !model.is_empty())
    }
}

fn is_printable(byte: u8) -> bool {
    (0x20..=0x7e).contains(&byte)
}

/// Iterates NUL-terminated printable strings, returning `(start, text)`.
fn nul_strings(bytes: &[u8]) -> Vec<(usize, String)> {
    const MAX_STRING_BYTES: usize = 63;
    let mut out = Vec::new();
    let mut start = None;
    for (index, &byte) in bytes.iter().enumerate() {
        match start {
            None => {
                if is_printable(byte) {
                    start = Some(index);
                }
            }
            Some(begin) => {
                if byte == 0 {
                    out.push((
                        begin,
                        String::from_utf8_lossy(&bytes[begin..index]).into_owned(),
                    ));
                    start = None;
                } else if !is_printable(byte) {
                    start = None;
                } else if index - begin + 1 > MAX_STRING_BYTES {
                    // HXD compiler dumps can fuse printable pointer/fill
                    // bytes onto a real sequence name. Keep a sliding
                    // suffix instead of dropping the entire NUL-terminated
                    // run; this mirrors `[printable]{1,63}\0` recovery and
                    // preserves the actual `NAMESPACE\NAME` tail.
                    start = Some(index + 1 - MAX_STRING_BYTES);
                }
            }
        }
    }
    out
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        && text.len() <= 24
}

/// Finds `NS\NAME` inside a string, allowing stray fused prefix bytes.
fn sequence_in_string(text: &str) -> Option<(usize, String)> {
    let bytes = text.as_bytes();
    for start in 0..bytes.len() {
        if !bytes[start].is_ascii_alphanumeric() {
            continue;
        }
        let Some(backslash) = text[start..].find('\\').map(|offset| start + offset) else {
            continue;
        };
        let namespace = &text[start..backslash];
        let name_start = backslash + 1;
        let name_end = bytes[name_start..]
            .iter()
            .position(|byte| !(byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b' '))
            .map_or(bytes.len(), |offset| name_start + offset);
        let name = &text[name_start..name_end];
        if namespace.len() >= 2
            && namespace
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            && !name.is_empty()
        {
            return Some((start, text[start..name_end].to_string()));
        }
    }
    None
}

fn read_f32(bytes: &[u8], offset: usize) -> Option<f32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(f32::from_le_bytes(slice.try_into().ok()?))
}

fn read_u32_checked(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn fixed_string(bytes: &[u8]) -> Option<String> {
    let end = bytes.iter().position(|byte| *byte == 0)?;
    let text = bytes.get(..end)?;
    (!text.is_empty() && text.iter().all(|byte| is_printable(*byte)))
        .then(|| String::from_utf8_lossy(text).into_owned())
}

/// Parse the fixed external-resource table at the tail of `MAINPED.HXD`.
/// Its first row is `C_Player`; the preceding u32 is the row count and every
/// row is `{ char name[32], u32 size, char model[32], f32 weight }`.
fn parse_external_resources(body: &[u8]) -> Vec<HxdResource> {
    const ROW_BYTES: usize = 72;
    const FIELD_BYTES: usize = 32;

    for (start, marker) in body.windows(b"C_Player\0".len()).enumerate().skip(4) {
        if !marker.eq_ignore_ascii_case(b"C_Player\0") {
            continue;
        }
        let Some(count) = read_u32_checked(body, start - 4).map(|value| value as usize) else {
            continue;
        };
        if !(2..=4096).contains(&count) {
            continue;
        }
        let Some(table_bytes) = count.checked_mul(ROW_BYTES) else {
            continue;
        };
        let Some(table) = body.get(start..start.saturating_add(table_bytes)) else {
            continue;
        };

        let mut resources = Vec::with_capacity(count);
        for row in table.chunks_exact(ROW_BYTES) {
            let Some(name) = fixed_string(&row[..FIELD_BYTES]) else {
                resources.clear();
                break;
            };
            let encoded_size = u32::from_le_bytes(row[32..36].try_into().unwrap_or([0; 4]));
            let model = if row[36] == 0 {
                String::new()
            } else if let Some(model) = fixed_string(&row[36..68]) {
                model
            } else {
                resources.clear();
                break;
            };
            resources.push(HxdResource {
                name,
                encoded_size,
                model,
            });
        }
        if resources.len() == count {
            return resources;
        }
    }
    Vec::new()
}

/// Read the duplicated sequence descriptor following a 32-byte name field.
fn sequence_source(body: &[u8], name_start: usize) -> Option<(u32, u32)> {
    let first = body.get(name_start + 32..name_start + 48)?;
    let second = body.get(name_start + 64..name_start + 80)?;
    if first != second {
        return None;
    }
    let encoded_size = u32::from_le_bytes(first[8..12].try_into().ok()?);
    let source_index = u32::from_le_bytes(first[12..16].try_into().ok()?);
    (encoded_size >= 4).then_some((encoded_size, source_index))
}

/// Parses one record body (loose `.HXD` file, or a body inside `hxds.dat`).
///
/// `fallback_model` is used when the record carries no readable name marker
/// (seen on the 451 KiB `MAINPED.HXD`); callers pass the file stem.
pub fn parse_record(body: &[u8], fallback_model: &str) -> HxdRecord {
    let strings = nul_strings(body);
    let resources = parse_external_resources(body);

    // Model name: identifier after the last `01 00 00 00` marker.
    let mut model: Option<String> = None;
    for index in 0..body.len().saturating_sub(3) {
        if body[index..index + 4] == [1, 0, 0, 0] {
            let after = index + 4;
            if let Some((_begin, text)) = strings
                .iter()
                .find(|(begin, _)| *begin == after)
                .map(|(begin, text)| (*begin, text.clone()))
                && text != "default"
                && is_identifier(&text)
            {
                model = Some(text);
            }
        }
    }

    // Sequences: `{ f32 duration, f32 weight }` then `NS\NAME`. Fused
    // prefix bytes (printable tails of the weight float) and multi-word
    // namespaces can shift the found string away from the row's 32-byte
    // name field, so the duration/weight floats and the duplicated
    // descriptors must be pinned to the field start: try a small candidate
    // window and keep the offset whose floats are plausible and whose
    // descriptor copies agree. `DISHONERABLE\VAULT_BAR` and
    // `MINISNOW\MINISNOW_HITSHVL` only validate at their true field start,
    // which is why they were previously dropped as malformed.
    let mut sequences: Vec<HxdSequence> = Vec::new();
    for (begin, text) in &strings {
        let Some((offset, name)) = sequence_in_string(text) else {
            continue;
        };
        let found = begin + offset;
        let run_start = *begin;
        let mut accepted = false;
        // The 32-byte name field starts at the found string position for
        // clean rows; a fused float-tail byte swallowed into the namespace
        // shifts it one to four bytes later (`8MINISNOW\...`), a multi-word
        // namespace one to four bytes earlier (`N2B DISHONERABLE\...`).
        // Try the plain position first so every already-working row keeps
        // its exact values, then the nearest neighbours outward. Row-tail
        // blocks can look like a descriptor pair, so a candidate must also
        // carry a catalog-plausible duration (one 30 fps frame minimum),
        // a bounded size, and an in-range resource index.
        for delta in [0_i64, 1, 2, 3, 4, -1, -2, -3, -4] {
            let candidate = found as i64 + delta;
            if candidate < run_start as i64 {
                continue;
            }
            let candidate = candidate as usize;
            let Some(duration) = read_f32(body, candidate.saturating_sub(8)) else {
                continue;
            };
            let Some(weight) = read_f32(body, candidate.saturating_sub(4)) else {
                continue;
            };
            // Also rejects NaN and infinities, which fall outside any range.
            if !(0.01..=600.0).contains(&duration) {
                continue;
            }
            if !weight.is_finite() || !(0.0..=1.0).contains(&weight) {
                continue;
            }
            let Some((encoded_size, source_index)) = sequence_source(body, candidate) else {
                continue;
            };
            if !(4..=4 * 1024 * 1024).contains(&encoded_size) {
                continue;
            }
            if resources.is_empty() || source_index as usize >= resources.len() {
                continue;
            }
            sequences.push(HxdSequence {
                name: name.clone(),
                duration_s: duration,
                weight,
                encoded_size: Some(encoded_size),
                source_index: Some(source_index),
            });
            accepted = true;
            break;
        }
        if accepted {
            continue;
        }
        // Historical readback: ordinary records without a validated
        // descriptor pair stay useful, with the optional source fields
        // left empty.
        let Some(duration) = read_f32(body, found.saturating_sub(8)) else {
            continue;
        };
        let Some(weight) = read_f32(body, found.saturating_sub(4)) else {
            continue;
        };
        if !duration.is_finite() || duration <= 0.0 || duration > 600.0 {
            continue;
        }
        if !weight.is_finite() || !(0.0..=1.0).contains(&weight) {
            continue;
        }
        let source = sequence_source(body, found);
        sequences.push(HxdSequence {
            name,
            duration_s: duration,
            weight,
            encoded_size: source.map(|(size, _)| size),
            source_index: source.map(|(_, index)| index),
        });
    }

    // Joint list: identifier-ish strings that are not sequences, not the
    // model, not `default`, and either `jointN` or a known-ish part name.
    let mut joints: Vec<String> = Vec::new();
    for (_, text) in &strings {
        if text == "default" || Some(text.as_str()) == model.as_deref() {
            continue;
        }
        if sequence_in_string(text).is_some() {
            continue;
        }
        if text.contains(['|', '*', '\t']) {
            continue;
        }
        let is_joint = text
            .strip_prefix("joint")
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()));
        if is_joint || (joints.is_empty() && is_identifier(text) && text.len() >= 3) {
            // Keep only the leading cluster of joint-ish names; semantic
            // part names appear before the `default` marker in ped records.
            joints.push(text.clone());
        }
    }

    HxdRecord {
        model: model.unwrap_or_else(|| fallback_model.to_string()),
        joints,
        sequences,
        resources,
    }
}

/// Parses the concatenated `hxds.dat` catalog.
pub fn parse_hxds(bytes: &[u8]) -> Vec<HxdRecord> {
    let mut records = Vec::new();
    if bytes.len() < 4 {
        return records;
    }
    let count = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let mut offset = 4usize;
    while offset + 8 <= bytes.len() && records.len() < count {
        if bytes[offset..offset + 4] != *b"ANIM" {
            break;
        }
        let size =
            u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap_or([0; 4])) as usize;
        let body_start = offset + 8;
        let Some(body) = bytes.get(body_start..body_start + size) else {
            break;
        };
        records.push(parse_record(body, ""));
        offset = body_start + size;
    }
    records
}

/// Resolves the HXD record for an AGR stem.
///
/// Order: loose `Anim/<STEM>.HXD` (case-insensitive), then the `hxds.dat`
/// catalog by model name.
pub fn find_for_stem(anim_dir: &Path, stem: &str) -> Option<HxdRecord> {
    let wanted = stem.to_ascii_lowercase();
    if let Ok(entries) = std::fs::read_dir(anim_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.to_ascii_lowercase().ends_with(".hxd") {
                continue;
            }
            let file_stem = &name[..name.len() - 4];
            if file_stem.to_ascii_lowercase() != wanted {
                continue;
            }
            let bytes = std::fs::read(entry.path()).ok()?;
            return Some(parse_record(&bytes, file_stem));
        }
    }
    let catalog = std::fs::read(anim_dir.join("hxds.dat")).ok()?;
    parse_hxds(&catalog)
        .into_iter()
        .find(|record| record.model.to_ascii_lowercase() == wanted)
}

/// Resolve naming metadata for an AGR. Direct HXD/hxds records take
/// precedence; character and mission AGRs fall back to `MAINPED.HXD` only
/// when its resource table explicitly contains the requested stem.
pub fn find_for_agr(anim_dir: &Path, stem: &str) -> Option<HxdRecord> {
    if let Some(record) = find_for_stem(anim_dir, stem) {
        return Some(record);
    }
    let bytes = std::fs::read(anim_dir.join("MAINPED.HXD")).ok()?;
    let record = parse_record(&bytes, "MAINPED");
    record
        .resources
        .iter()
        .any(|resource| resource.name.eq_ignore_ascii_case(stem))
        .then_some(record)
}

/// Derives the game root from an archive path, if it looks like the retail
/// layout (`<root>/Stream/<archive>.img`).
pub fn anim_dir_for_archive(archive_path: Option<&Path>) -> Option<PathBuf> {
    let path = archive_path?;
    let parent = path.parent()?;
    let root = parent.parent().unwrap_or(parent);
    let anim = root.join("Anim");
    anim.is_dir().then_some(anim)
}

/// Case-insensitive NIF entry lookup by model name.
pub fn find_model_entry(entries: &[crate::archive::EntryInfo], model: &str) -> Option<usize> {
    let wanted = Path::new(model)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(model)
        .to_ascii_lowercase();
    let wanted_nif = format!("{wanted}.nif");
    entries.iter().position(|entry| {
        let file = entry.file_name.to_ascii_lowercase();
        file == wanted_nif || file == wanted
    })
}

/// Catalog-associated NIF candidates for the animation model picker: every
/// model referenced by the `Anim/` catalogs (loose `<MODEL>.HXD` stems,
/// `hxds.dat` records and `MAINPED.HXD` resources) that resolves to an
/// archive entry. Returned as `(entry index, file name)`, sorted by name —
/// this is the set of models that can legitimately carry animation clips.
pub fn candidate_models(
    anim_dir: Option<&Path>,
    entries: &[crate::archive::EntryInfo],
) -> Vec<(usize, String)> {
    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    if let Some(anim) = anim_dir {
        if let Ok(dir) = std::fs::read_dir(anim) {
            for entry in dir.flatten() {
                let file_name = entry.file_name();
                let file_name = file_name.to_string_lossy();
                if file_name.to_ascii_lowercase().ends_with(".hxd") {
                    names.insert(file_name[..file_name.len() - 4].to_ascii_lowercase());
                }
            }
        }
        if let Ok(bytes) = std::fs::read(anim.join("hxds.dat")) {
            for record in parse_hxds(&bytes) {
                names.insert(record.model.to_ascii_lowercase());
            }
        }
        if let Ok(bytes) = std::fs::read(anim.join("MAINPED.HXD")) {
            let record = parse_record(&bytes, "MAINPED");
            names.insert(record.model.to_ascii_lowercase());
            for resource in &record.resources {
                names.insert(resource.model.to_ascii_lowercase());
            }
        }
    }
    let mut seen: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut candidates: Vec<(usize, String)> = Vec::new();
    for name in &names {
        if name.is_empty() {
            continue;
        }
        if let Some(index) = find_model_entry(entries, name)
            && seen.insert(index)
        {
            candidates.push((index, entries[index].file_name.to_string()));
        }
    }
    candidates.sort_by(|left, right| {
        left.1
            .to_ascii_lowercase()
            .cmp(&right.1.to_ascii_lowercase())
    });
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_string_hxds_record(out: &mut Vec<u8>, model: &str, sequences: &[(&str, f32, f32)]) {
        let mut body = Vec::new();
        body.extend_from_slice(&[0u8; 8]); // serialized pointer area
        body.extend_from_slice(&1u32.to_le_bytes());
        body.extend_from_slice(b"default\0");
        body.extend_from_slice(&[0u8; 4]);
        for (name, duration, weight) in sequences {
            body.extend_from_slice(&1u32.to_le_bytes());
            body.extend_from_slice(&duration.to_le_bytes());
            body.extend_from_slice(&weight.to_le_bytes());
            body.extend_from_slice(name.as_bytes());
            body.push(0);
        }
        body.extend_from_slice(&1u32.to_le_bytes());
        body.extend_from_slice(model.as_bytes());
        body.push(0);
        let magic = b"ANIM";
        out.extend_from_slice(magic);
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&body);
    }

    #[test]
    fn parses_synthetic_record() {
        let mut bytes = Vec::new();
        push_string_hxds_record(
            &mut bytes,
            "RMailbox",
            &[("RMAILBOX\\IDLE", 0.5, 0.3), ("RMAILBOX\\OPEN", 0.25, 0.3)],
        );
        let mut file = Vec::new();
        file.extend_from_slice(&1u32.to_le_bytes());
        file.extend_from_slice(&bytes);
        let records = parse_hxds(&file);
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.model, "RMailbox");
        assert_eq!(record.sequences.len(), 2);
        assert_eq!(record.sequences[0].name, "RMAILBOX\\IDLE");
        assert_eq!(record.sequences[0].leaf_name(), "IDLE");
        assert!((record.sequences[1].duration_s - 0.25).abs() < 1e-6);
    }

    #[test]
    fn long_printable_prefix_keeps_the_sequence_suffix() {
        let mut body = vec![b'X'; 72];
        let sequence_start = body.len();
        body.extend_from_slice(b"C_PLAYER\\RUN\0");
        body[sequence_start - 8..sequence_start - 4].copy_from_slice(&0.75_f32.to_le_bytes());
        body[sequence_start - 4..sequence_start].copy_from_slice(&0.3_f32.to_le_bytes());

        let record = parse_record(&body, "MAINPED");
        assert_eq!(record.sequences.len(), 1);
        assert_eq!(record.sequences[0].name, "C_PLAYER\\RUN");
        assert!((record.sequences[0].duration_s - 0.75).abs() < 1e-6);
    }

    #[test]
    fn printable_suffix_after_sequence_name_is_ignored() {
        let mut body = vec![0u8; 8];
        body[..4].copy_from_slice(&0.5_f32.to_le_bytes());
        body[4..8].copy_from_slice(&0.3_f32.to_le_bytes());
        body.extend_from_slice(b"C_PLAYER\\RUN>junk\0");

        let record = parse_record(&body, "MAINPED");
        assert_eq!(record.sequences.len(), 1);
        assert_eq!(record.sequences[0].name, "C_PLAYER\\RUN");
    }

    #[test]
    fn compound_catalog_skips_stale_rows_by_chunk_size() {
        let sequence = |name: &str, encoded_size, source_index| HxdSequence {
            name: name.to_string(),
            duration_s: 1.0,
            weight: 0.3,
            encoded_size: Some(encoded_size),
            source_index: Some(source_index),
        };
        let record = HxdRecord {
            model: "MAINPED".into(),
            joints: Vec::new(),
            sequences: vec![
                sequence("C_PLAYER\\RUN", 104, 0),
                sequence("DISHONERABLE\\VAULT_BAR", 97, 0),
                sequence("C_PLAYER\\IDLE", 204, 0),
                sequence("NPC_GENERIC\\WARNING", 84, 1),
            ],
            resources: vec![
                HxdResource {
                    name: "C_Player".into(),
                    encoded_size: 0,
                    model: "player.mxd".into(),
                },
                HxdResource {
                    name: "NPC_Cher".into(),
                    encoded_size: 0,
                    model: "player.mxd".into(),
                },
            ],
        };
        let clips = [
            HxdClipSignature {
                source_size: 100,
                duration_s: 1.0,
            },
            HxdClipSignature {
                source_size: 200,
                duration_s: 1.0,
            },
        ];

        assert_eq!(
            record.sequence_names_for_agr("c_player", &clips),
            ["RUN", "IDLE"]
        );
        assert!(record.sequence_names_for_agr("missing", &clips).is_empty());
    }

    #[test]
    fn partial_coverage_names_only_matched_clips() {
        let sequence = |name: &str, encoded_size| HxdSequence {
            name: name.into(),
            duration_s: 1.0,
            weight: 0.3,
            encoded_size: Some(encoded_size),
            source_index: Some(0),
        };
        let record = HxdRecord {
            model: "PLAYER".into(),
            joints: Vec::new(),
            sequences: vec![
                sequence("C_PLAYER\\RUN", 104),
                sequence("C_PLAYER\\IDLE", 204),
            ],
            resources: vec![HxdResource {
                name: "C_Player".into(),
                encoded_size: 0,
                model: "player.mxd".into(),
            }],
        };
        let signature = |source_size, duration_s| HxdClipSignature {
            source_size,
            duration_s,
        };

        // Clip 1 has no size-compatible row: the covered run still names
        // around it and the unmatched clip keeps an empty (positional) name.
        let names = record.sequence_names_for_agr(
            "C_Player",
            &[
                signature(100, 1.0),
                signature(400, 1.0),
                signature(200, 1.0),
            ],
        );
        assert_eq!(names, ["RUN", "", "IDLE"]);

        // Catalog rows may also outnumber the clips.
        let names = record.sequence_names_for_agr("C_Player", &[signature(200, 1.0)]);
        assert_eq!(names, ["IDLE"]);

        // Unrelated sizes name nothing rather than guessing.
        let names = record.sequence_names_for_agr("C_Player", &[signature(999, 1.0)]);
        assert_eq!(names, [""]);
    }

    #[test]
    fn find_model_entry_is_case_insensitive() {
        let mut entry = crate::archive::EntryInfo::new("SK8Board.nif");
        let entries = vec![entry.clone()];
        assert_eq!(find_model_entry(&entries, "sk8board"), Some(0));
        entry.file_name = "Bike.nif".into();
        let entries = vec![entry];
        assert_eq!(find_model_entry(&entries, "BIKE"), Some(0));
        assert_eq!(find_model_entry(&entries, "MISSING"), None);
    }

    #[test]
    fn real_catalogs_parse_when_available() {
        let Ok(stream) = std::env::var("IMGEDITOR_BULLY_STREAM") else {
            return;
        };
        let anim = Path::new(&stream).parent().unwrap().join("Anim");
        if !anim.is_dir() {
            return;
        }
        if let Ok(bytes) = std::fs::read(anim.join("SK8BOARD.HXD")) {
            let record = parse_record(&bytes, "SK8BOARD");
            assert_eq!(record.model, "SK8Board");
            assert_eq!(record.sequences.len(), 17);
            assert_eq!(record.sequences[0].name, "SKATEBOARD\\IDLE_SK8BOARD");
            assert!((record.sequences[0].duration_s - 0.0333).abs() < 0.001);
            assert_eq!(record.sequences[16].name, "SKATEBOARD\\SK8_EXAMINE_O");
            assert!((record.sequences[16].duration_s - 3.8333).abs() < 0.01);
        }
        if let Ok(bytes) = std::fs::read(anim.join("hxds.dat")) {
            let records = parse_hxds(&bytes);
            assert!(records.len() >= 128, "records: {}", records.len());
            let broom = records
                .iter()
                .find(|record| record.model == "AniBroom")
                .expect("AniBroom record");
            assert_eq!(broom.sequences.len(), 3);
            assert_eq!(broom.sequences[0].name, "ANIBROOM\\IDLE");
            assert_eq!(broom.sequences[1].leaf_name(), "LEFT");
            assert_eq!(broom.sequences[2].leaf_name(), "RIGHT");
        }
        // Resolver checks: loose file first, catalog fallback second.
        let sk8 = find_for_stem(&anim, "SK8Board").expect("loose SK8Board.HXD");
        assert_eq!(sk8.model, "SK8Board");
        assert_eq!(sk8.sequences.len(), 17);
        let broom = find_for_stem(&anim, "AniBroom").expect("AniBroom via hxds.dat");
        assert_eq!(broom.model, "AniBroom");
        assert_eq!(broom.sequences.len(), 3);
        assert_eq!(find_for_stem(&anim, "1_07_Sk8Board"), None);

        if let (Ok(hxd_bytes), Ok(agr_bytes)) = (
            std::fs::read(anim.join("MAINPED.HXD")),
            std::fs::read(anim.join("C_Player.agr")),
        ) {
            let record = parse_record(&hxd_bytes, "MAINPED");
            // 3,358 rows through the historical reader plus the two fused-
            // prefix rows recovered by the descriptor-pinning candidate scan
            // (`MINISNOW\MINISNOW_HITSHVL`, `DISHONERABLE\VAULT_BAR`).
            assert_eq!(record.sequences.len(), 3_359);
            assert_eq!(record.resources.len(), 425);
            assert_eq!(record.resources[0].name, "C_Player");
            assert_eq!(record.model_for_source("c_player"), Some("player.mxd"));

            let agr = crate::inspector::animation::bully::parse_agr(&agr_bytes)
                .expect("C_Player.agr parses");
            assert_eq!(agr.clip_count(), 439);
            let signatures: Vec<_> = agr
                .clips
                .iter()
                .map(|clip| HxdClipSignature {
                    source_size: clip.source_size,
                    duration_s: clip.duration_s,
                })
                .collect();
            let names = record.sequence_names_for_agr("C_Player", &signatures);
            assert_eq!(names.len(), 439);
            assert_eq!(names.first().map(String::as_str), Some("RUN"));
            assert!(!names.iter().any(|name| name == "VAULT_BAR"));

            for (source, expected_count) in [("Grap", 59), ("NPC_Cher", 12)] {
                let Ok(agr_bytes) = std::fs::read(anim.join(format!("{source}.agr"))) else {
                    continue;
                };
                let agr = crate::inspector::animation::bully::parse_agr(&agr_bytes)
                    .unwrap_or_else(|error| panic!("{source}.agr: {error}"));
                assert_eq!(agr.clip_count(), expected_count);
                let signatures: Vec<_> = agr
                    .clips
                    .iter()
                    .map(|clip| HxdClipSignature {
                        source_size: clip.source_size,
                        duration_s: clip.duration_s,
                    })
                    .collect();
                assert_eq!(
                    record.sequence_names_for_agr(source, &signatures).len(),
                    expected_count
                );
            }
        }
        // Archive path -> Anim dir derivation with the retail layout.
        let stream = Path::new(&stream);
        let anim_from_archive =
            anim_dir_for_archive(Some(&stream.join("World.img"))).expect("derived Anim dir");
        assert_eq!(
            anim_from_archive.file_name().and_then(|name| name.to_str()),
            Some("Anim")
        );
    }
}
