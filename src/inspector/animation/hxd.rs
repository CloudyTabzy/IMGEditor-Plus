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
}

impl HxdSequence {
    /// Leaf name without the namespace (`IDLE_SK8BOARD`).
    pub fn leaf_name(&self) -> &str {
        self.name.rsplit('\\').next().unwrap_or(&self.name)
    }
}

/// One parsed hierarchy record.
#[derive(Clone, Debug, PartialEq)]
pub struct HxdRecord {
    /// Model name (`SK8Board`, `AniBroom`, `RMailbox`, ...).
    pub model: String,
    /// Joint names (`joint0`..., or semantic names for peds).
    pub joints: Vec<String>,
    /// Ordered animation sequences; the index matches the AGR chunk index.
    pub sequences: Vec<HxdSequence>,
}

impl HxdRecord {
    /// Sequence leaf names in order.
    pub fn sequence_names(&self) -> Vec<String> {
        self.sequences
            .iter()
            .map(|sequence| sequence.leaf_name().to_string())
            .collect()
    }
}

fn is_printable(byte: u8) -> bool {
    (0x20..=0x7e).contains(&byte)
}

/// Iterates NUL-terminated printable strings, returning `(start, text)`.
fn nul_strings(bytes: &[u8]) -> Vec<(usize, String)> {
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
                    if index - begin <= 63 {
                        out.push((
                            begin,
                            String::from_utf8_lossy(&bytes[begin..index]).into_owned(),
                        ));
                    }
                    start = None;
                } else if !is_printable(byte) || index - begin > 63 {
                    start = None;
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
        if !(bytes[start].is_ascii_uppercase() || bytes[start].is_ascii_digit()) {
            continue;
        }
        let Some(backslash) = text[start..].find('\\').map(|offset| start + offset) else {
            continue;
        };
        let namespace = &text[start..backslash];
        let name = &text[backslash + 1..];
        if namespace.len() >= 2
            && namespace
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            && !name.is_empty()
            && name.len() <= 40
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b' ')
        {
            return Some((start, text[start..].to_string()));
        }
    }
    None
}

fn read_f32(bytes: &[u8], offset: usize) -> Option<f32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(f32::from_le_bytes(slice.try_into().ok()?))
}

/// Parses one record body (loose `.HXD` file, or a body inside `hxds.dat`).
///
/// `fallback_model` is used when the record carries no readable name marker
/// (seen on the 451 KiB `MAINPED.HXD`); callers pass the file stem.
pub fn parse_record(body: &[u8], fallback_model: &str) -> HxdRecord {
    let strings = nul_strings(body);

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

    // Sequences: `{ f32 duration, f32 weight }` then `NS\NAME`.
    let mut sequences: Vec<HxdSequence> = Vec::new();
    for (begin, text) in &strings {
        let Some((offset, name)) = sequence_in_string(text) else {
            continue;
        };
        let name_start = begin + offset;
        let Some(duration) = read_f32(body, name_start.saturating_sub(8)) else {
            continue;
        };
        let Some(weight) = read_f32(body, name_start.saturating_sub(4)) else {
            continue;
        };
        if !duration.is_finite() || duration <= 0.0 || duration > 600.0 {
            continue;
        }
        if !weight.is_finite() || !(0.0..=1.0).contains(&weight) {
            continue;
        }
        sequences.push(HxdSequence {
            name,
            duration_s: duration,
            weight,
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
    let wanted = model.to_ascii_lowercase();
    let wanted_nif = format!("{wanted}.nif");
    entries.iter().position(|entry| {
        let file = entry.file_name.to_ascii_lowercase();
        file == wanted_nif || file == wanted
    })
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
