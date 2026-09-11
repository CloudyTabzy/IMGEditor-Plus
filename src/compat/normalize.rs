//! Mechanical header fixes for the pre-save normalize pass.
//!
//! Scope is deliberately narrow: only the DXT header inconsistencies we
//! have corpus evidence for, fixed by patching header fields in place -
//! **no pixel data is touched**. Fixing an entry stores patched bytes as
//! an [`crate::archive::EntryInfo`] override, which the save writers
//! already prefer over the source range.
//!
//! Not fixed here (reported only): DXT5/DXT2/DXT4 headers (no retail
//! evidence for the conventional nibble), palette questions and anything
//! requiring re-encoding - that is the Phase B converter's job.

use std::sync::Arc;

use crate::archive::ArchiveInfo;

/// One entry that can be repaired, with a human-readable description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderFix {
    pub entry_index: usize,
    pub file_name: String,
    pub detail: String,
}

/// Where a native texture's patched header fields live in the entry.
#[derive(Debug, Clone, Copy)]
struct NativeHeader {
    raster_pos: usize,
    depth_pos: usize,
    raster_format: u32,
    d3d_format: u32,
    depth: u8,
}

/// Plan every mechanically fixable entry in the archive. Reads entry
/// bytes; nothing is modified.
pub fn plan_fixes(archive: &ArchiveInfo) -> Vec<HeaderFix> {
    let mut fixes = Vec::new();
    for (index, entry) in archive.entries.iter().enumerate() {
        if !entry.file_name_lower.ends_with(".txd") {
            continue;
        }
        let Ok(bytes) = crate::parser::read_entry_data(archive, entry) else {
            continue;
        };
        for header in walk_native_headers(&bytes) {
            if let Some(detail) = fix_detail(&header) {
                fixes.push(HeaderFix {
                    entry_index: index,
                    file_name: entry.file_name.to_string(),
                    detail,
                });
                break; // one report line per entry is enough
            }
        }
    }
    fixes
}

/// Apply the fixes: every entry with a fixable header gets patched bytes
/// stored as an override. Returns the number of patched entries.
pub fn apply_fixes(archive: &mut ArchiveInfo) -> usize {
    let indices: Vec<usize> = archive
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.file_name_lower.ends_with(".txd"))
        .map(|(index, _)| index)
        .collect();
    let mut patched = 0;
    for index in indices {
        let Ok(mut bytes) = crate::parser::read_entry_data(archive, &archive.entries[index])
        else {
            continue;
        };
        let mut changed = false;
        for header in walk_native_headers(&bytes) {
            if let Some(nibble) = fixed_nibble(header.d3d_format) {
                let want = (header.raster_format & !0x0F00) | nibble;
                if header.raster_format != want {
                    bytes[header.raster_pos..header.raster_pos + 4]
                        .copy_from_slice(&want.to_le_bytes());
                    changed = true;
                }
                if header.depth != 16 {
                    bytes[header.depth_pos] = 16;
                    changed = true;
                }
            }
        }
        if changed {
            archive.entries[index].override_bytes = Some(Arc::new(bytes));
            patched += 1;
        }
    }
    patched
}

/// Describe what would change, or `None` when the header is fine.
fn fix_detail(header: &NativeHeader) -> Option<String> {
    let nibble = fixed_nibble(header.d3d_format)?;
    let want_format = (header.raster_format & !0x0F00) | nibble;
    let current_nibble = (header.raster_format >> 8) & 0xF;
    let want_nibble = (want_format >> 8) & 0xF;
    let mut parts = Vec::new();
    if header.raster_format != want_format {
        parts.push(format!(
            "raster nibble 0x{current_nibble:X} -> 0x{want_nibble:X}"
        ));
    }
    if header.depth != 16 {
        parts.push(format!("depth {} -> 16", header.depth));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

/// The conventional raster nibble (bits 8-11) for a DXT fourcc, where
/// corpus evidence exists: retail DXT1 uses 0x2xx (or 0x82xx with mips)
/// and DXT3 uses 0x3xx. DXT5/2/4 are reported but not guessed at.
fn fixed_nibble(d3d_format: u32) -> Option<u32> {
    match d3d_format {
        0x3154_5844 => Some(0x0200), // "DXT1"
        0x3354_5844 => Some(0x0300), // "DXT3"
        _ => None,
    }
}

// ---- TXD section walking --------------------------------------------

const RW_TEXTURE_DICTIONARY: u32 = 0x16;
const RW_TEXTURE_NATIVE: u32 = 0x15;
const RW_STRUCT: u32 = 1;

fn read_u32(bytes: &[u8], pos: usize) -> Option<u32> {
    let slice = bytes.get(pos..pos + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

/// Find every native texture's header field positions. Tolerant of
/// section sizes that exceed the buffer (truncated reads are not
/// expected here, but a malformed file must not panic).
fn walk_native_headers(bytes: &[u8]) -> Vec<NativeHeader> {
    let mut out = Vec::new();
    if read_u32(bytes, 0) != Some(RW_TEXTURE_DICTIONARY) {
        return out;
    }
    let top_size = read_u32(bytes, 4).unwrap_or(0) as usize;
    let top_end = (12 + top_size).min(bytes.len());
    let mut position = 12usize;
    while position + 12 <= top_end {
        let Some(kind) = read_u32(bytes, position) else {
            break;
        };
        let size = read_u32(bytes, position + 4).unwrap_or(0) as usize;
        let end = (position + 12 + size).min(top_end);
        if kind == RW_TEXTURE_NATIVE {
            let base = position + 12;
            if let Some(mut header) = native_header(&bytes[base..end]) {
                // Positions come back relative to the native body;
                // rebase them onto the entry bytes.
                header.raster_pos += base;
                header.depth_pos += base;
                out.push(header);
            }
        }
        if end <= position {
            break;
        }
        position = end;
    }
    out
}

/// Parse a TEXTURE_NATIVE's struct header: platform u32, flags 4, names
/// 64, raster format u32, D3D format u32, width/height u16 x2, depth u8.
fn native_header(body: &[u8]) -> Option<NativeHeader> {
    let mut position = 0usize;
    while position + 12 <= body.len() {
        let kind = read_u32(body, position)?;
        let size = read_u32(body, position + 4).unwrap_or(0) as usize;
        let end = (position + 12 + size).min(body.len());
        let struct_body = body.get(position + 12..end)?;
        if kind == RW_STRUCT {
            if struct_body.len() < 85 {
                return None;
            }
            return Some(NativeHeader {
                raster_pos: position + 12 + 72,
                depth_pos: position + 12 + 84,
                raster_format: read_u32(struct_body, 72)?,
                d3d_format: read_u32(struct_body, 76)?,
                depth: struct_body[84],
            });
        }
        if end <= position {
            break;
        }
        position = end;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::EntryInfo;
    use crate::parser::ImgVersion;

    fn u16le(value: u16) -> Vec<u8> {
        value.to_le_bytes().to_vec()
    }

    fn u32le(value: u32) -> Vec<u8> {
        value.to_le_bytes().to_vec()
    }

    fn section(kind: u32, body: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(u32le(kind));
        out.extend(u32le(body.len() as u32));
        out.extend(u32le(0x1803_FFFF));
        out.extend_from_slice(body);
        out
    }

    /// One 8x8 native texture with the given header fields.
    fn txd_with(raster_format: u32, d3d_format: u32, depth: u8) -> Vec<u8> {
        let mut native = Vec::new();
        native.extend(u32le(8)); // platform
        native.extend([6, 17, 0, 0]);
        native.extend([0u8; 64]); // names
        native.extend(u32le(raster_format));
        native.extend(u32le(d3d_format));
        native.extend(u16le(8));
        native.extend(u16le(8));
        native.extend([depth, 1, 4, 0]);
        native.extend(u32le(16));
        native.extend([0x8A; 16]);
        let mut dict = section(1, &[1, 0, 2, 0]);
        dict.extend(section(RW_TEXTURE_NATIVE, &section(RW_STRUCT, &native)));
        section(RW_TEXTURE_DICTIONARY, &dict)
    }

    fn archive_with(path: &std::path::Path, bytes: &[u8]) -> ArchiveInfo {
        std::fs::write(path, bytes).unwrap();
        let mut archive = ArchiveInfo::new("test", true, ImgVersion::One);
        let mut entry = EntryInfo::new("prop.txd");
        entry.imported = true;
        entry.source_path = Some(path.to_path_buf());
        archive.entries.push(entry);
        archive
    }

    #[test]
    fn dxt3_with_8888_header_is_planned_and_patched() {
        let dir = tempfile::tempdir().unwrap();
        // DXT3 fourcc on an 8888 nibble with 32-bit depth: the shipped
        // INU-writer bug shape.
        let path = dir.path().join("bad.txd");
        let mut archive = archive_with(&path, &txd_with(0x0500, 0x3354_5844, 32));

        let fixes = plan_fixes(&archive);
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].detail.contains("0x5 -> 0x3"), "{}", fixes[0].detail);
        assert!(fixes[0].detail.contains("depth 32 -> 16"));

        assert_eq!(apply_fixes(&mut archive), 1);
        let patched = archive.entries[0].override_bytes.as_ref().unwrap();
        let header = walk_native_headers(patched);
        assert_eq!(header[0].raster_format, 0x0300);
        assert_eq!(header[0].depth, 16);
        // Idempotent: nothing left to plan.
        assert!(plan_fixes(&archive).is_empty());
    }

    #[test]
    fn dxt1_stale_nibble_is_aligned_and_mip_flag_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stale.txd");
        // Retail SA shape: DXT1 fourcc, stale 1555 nibble, mip flag set.
        let archive = archive_with(&path, &txd_with(0x8100, 0x3154_5844, 16));

        let fixes = plan_fixes(&archive);
        assert_eq!(fixes.len(), 1);
        let mut archive = archive;
        assert_eq!(apply_fixes(&mut archive), 1);
        let patched = archive.entries[0].override_bytes.as_ref().unwrap();
        let header = walk_native_headers(patched);
        assert_eq!(header[0].raster_format, 0x8200, "nibble fixed, mip bit kept");
    }

    #[test]
    fn healthy_and_unevidenced_headers_are_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        // Clean DXT1: no plan.
        let good = archive_with(&dir.path().join("good.txd"), &txd_with(0x0200, 0x3154_5844, 16));
        assert!(plan_fixes(&good).is_empty());

        // DXT5 has no corpus evidence for a nibble: reported nowhere,
        // patched nowhere.
        let dxt5 = archive_with(&dir.path().join("dxt5.txd"), &txd_with(0x0500, 0x3554_5844, 32));
        assert!(plan_fixes(&dxt5).is_empty());
        let mut dxt5 = dxt5;
        assert_eq!(apply_fixes(&mut dxt5), 0);
        assert!(dxt5.entries[0].override_bytes.is_none());
    }
}
