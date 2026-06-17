use std::path::Path;

use compact_str::CompactString;
use memmap2::Mmap;

use crate::archive::{ArchiveInfo, EntryInfo};
use crate::parser::{SECTOR_SIZE, read_entry_header_standalone};

#[derive(Debug, Clone, Default)]
pub struct EntryInspection {
    pub file_name: CompactString,
    pub file_type: CompactString,
    pub size_bytes: u64,
    pub size_sectors: u32,
    pub offset_bytes: u64,
    pub source: CompactString,
    pub summary: Vec<(String, String)>,
    pub preview_hex: Option<String>,
    /// TXD metadata: list of (texture_name, format_name, width, height) strings.
    pub txd_textures: Vec<String>,
}

pub fn inspect_entry(archive: &ArchiveInfo, entry: &EntryInfo) -> EntryInspection {
    inspect_entry_standalone(
        entry,
        archive.path.as_deref(),
        archive.source_mmap.as_deref(),
        &archive.file_name,
    )
}

pub fn inspect_entry_standalone(
    entry: &EntryInfo,
    archive_path: Option<&Path>,
    mmap: Option<&Mmap>,
    archive_file_name: &str,
) -> EntryInspection {
    let mut inspection = EntryInspection {
        file_name: entry.file_name.clone(),
        file_type: entry.file_type.clone(),
        size_sectors: entry.sector,
        offset_bytes: u64::from(entry.offset) * SECTOR_SIZE,
        ..Default::default()
    };

    let header = read_entry_header_standalone(entry, archive_path, mmap, 8192).unwrap_or_default();
    let actual_size = actual_file_size(entry);
    inspection.size_bytes = actual_size;

    inspection.source = if entry.imported {
        entry
            .source_path
            .as_ref()
            .map(|p| CompactString::new(format!("Imported from {}", p.display())))
            .unwrap_or_else(|| CompactString::new("Imported"))
    } else {
        let archive_name = archive_path
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| archive_file_name.to_string());
        CompactString::new(format!(
            "Archive {} at sector {}",
            archive_name, entry.offset
        ))
    };

    let ext = Path::new(entry.file_name.as_str())
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "txd" => {
            inspect_renderware(&header, &mut inspection);
            inspect_txd(&header, &mut inspection);
        }
        "dff" => {
            inspect_renderware(&header, &mut inspection);
            inspect_dff(&header, &mut inspection);
        }
        "anm" | "ifp" => {
            inspect_renderware(&header, &mut inspection);
        }
        "col" => {
            inspect_collision(&header, &mut inspection);
            inspect_col_mesh(&header, &mut inspection);
        }
        "nif" => {
            inspect_nif(&header, &mut inspection);
        }
        "ipl" | "ide" | "dat" | "scm" | "txt" | "cfg" | "ini" => {
            inspect_text(&header, &mut inspection);
        }
        _ => {
            inspect_generic(&header, &mut inspection);
        }
    }

    inspection
}

fn actual_file_size(entry: &EntryInfo) -> u64 {
    if entry.imported {
        entry
            .source_path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .unwrap_or_else(|| u64::from(entry.sector) * SECTOR_SIZE)
    } else {
        u64::from(entry.sector) * SECTOR_SIZE
    }
}

fn inspect_renderware(header: &[u8], inspection: &mut EntryInspection) {
    if header.len() < 12 {
        inspection.summary.push(("Format".to_string(), "RenderWare (truncated)".to_string()));
        return;
    }

    let chunk_type = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let version = u32::from_le_bytes([header[8], header[9], header[10], header[11]]);

    let type_name = match chunk_type {
        0x10 => "Clump (model)",
        0x16 => "Texture Dictionary",
        0x1B => "Animation",
        0x0253F2F6 => "UV Animation",
        _ => "RenderWare stream",
    };

    inspection.summary.push(("Format".to_string(), type_name.to_string()));
    inspection.summary.push(("Version".to_string(), format!("0x{:08X}", version)));

    if inspection.file_name.as_str().to_ascii_lowercase().ends_with(".dff") && header.len() >= 28 {
        let clump_size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
        inspection.summary.push(("Clump size".to_string(), format!("{} bytes", clump_size)));
    }
}

fn inspect_collision(header: &[u8], inspection: &mut EntryInspection) {
    if header.starts_with(b"COLL") {
        inspection.summary.push(("Version".to_string(), "GTA III / VC (COLL)".to_string()));
    } else if header.starts_with(b"COL2") {
        inspection.summary.push(("Version".to_string(), "GTA SA (COL2)".to_string()));
    } else if header.starts_with(b"COL3") {
        inspection.summary.push(("Version".to_string(), "GTA SA (COL3)".to_string()));
    } else if header.starts_with(b"COL4") {
        inspection.summary.push(("Version".to_string(), "GTA IV (COL4)".to_string()));
    } else {
        inspection.summary.push(("Version".to_string(), "Unknown collision".to_string()));
    }
}

fn inspect_nif(header: &[u8], inspection: &mut EntryInspection) {
    if header.starts_with(b"Gamebryo File Format") {
        let prefix = String::from_utf8_lossy(&header[..header.len().min(64)]);
        let version_line = prefix
            .lines()
            .next()
            .unwrap_or("Gamebryo")
            .trim_end_matches('\0');
        inspection.summary.push(("Format".to_string(), version_line.to_string()));
    } else if header.len() >= 20 {
        let version = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let endian = header[12];
        let user_version = u32::from_le_bytes([header[13], header[14], header[15], header[16]]);
        inspection.summary.push(("Format".to_string(), "NetImmerse / Gamebryo".to_string()));
        inspection.summary.push(("Version".to_string(), format!("0x{:08X}", version)));
        inspection.summary.push(("Endian".to_string(), if endian == 1 { "Big" } else { "Little" }.to_string()));
        inspection.summary.push(("User version".to_string(), format!("0x{:08X}", user_version)));
    } else {
        inspection.summary.push(("Format".to_string(), "NIF (truncated)".to_string()));
    }
}

fn inspect_text(header: &[u8], inspection: &mut EntryInspection) {
    let text = String::from_utf8_lossy(header);
    let lines = text.lines().count();
    let non_empty = text.lines().filter(|l| !l.trim().is_empty()).count();
    inspection.summary.push(("Lines".to_string(), format!("{} ({} non-empty)", lines, non_empty)));

    if inspection.file_name.as_str().to_ascii_lowercase().ends_with(".scm") {
        inspection.summary.push(("Format".to_string(), "GTA script (main.scm)".to_string()));
    } else if inspection.file_name.as_str().to_ascii_lowercase().ends_with(".ipl") {
        inspection.summary.push(("Format".to_string(), "GTA item placement".to_string()));
    } else if inspection.file_name.as_str().to_ascii_lowercase().ends_with(".ide") {
        inspection.summary.push(("Format".to_string(), "GTA item definition".to_string()));
    }
}

fn inspect_dff(header: &[u8], inspection: &mut EntryInspection) {
    if header.len() < 12 {
        return;
    }
    // Fast header-only scan: try to find the clump struct for vertex/tri counts.
    let mut pos = 12usize;
    let section_size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
    let section_end = 12usize + section_size.min(header.len().saturating_sub(12));

    while pos + 12 <= section_end {
        let child_type = u32::from_le_bytes([header[pos], header[pos + 1], header[pos + 2], header[pos + 3]]);
        let child_size = u32::from_le_bytes([header[pos + 4], header[pos + 5], header[pos + 6], header[pos + 7]]) as usize;
        let child_end = (pos + 12 + child_size).min(section_end);

        if child_type == 0x01 && pos + 12 + 16 <= section_end {
            // Clump STRUCT: num_atomics, num_lights, num_cameras
            let data_offset = pos + 12;
            let num_atomics = u32::from_le_bytes([
                header[data_offset], header[data_offset + 1],
                header[data_offset + 2], header[data_offset + 3],
            ]);
            inspection.summary.push(("Atomics".to_string(), format!("{}", num_atomics)));
            break;
        }
        pos = child_end;
    }
}

fn inspect_txd(header: &[u8], inspection: &mut EntryInspection) {
    if header.len() < 12 {
        return;
    }
    // Try to parse the TXD to extract texture metadata.
    match crate::parser::txd::parse_txd(header) {
        Ok(txd) => {
            let count = txd.textures.len();
            inspection
                .summary
                .push(("Textures".to_string(), format!("{} texture(s)", count)));
            inspection.txd_textures = txd
                .textures
                .iter()
                .map(|t| {
                    format!(
                        "{} ({}: {}×{})",
                        t.diffuse_name,
                        t.format_name(),
                        t.width,
                        t.height
                    )
                })
                .collect();
        }
        Err(_) => {
            // Failed to parse header-only TXD — may need full data.
        }
    }
}

fn inspect_col_mesh(header: &[u8], inspection: &mut EntryInspection) {
    // Quick header-only parse to extract counts.
    match crate::parser::col::parse_col(header) {
        Ok(col) => {
            let total_verts: usize = col.entries.iter().map(|e| e.vertices.len()).sum();
            let total_faces: usize = col.entries.iter().map(|e| e.indices.len() / 3).sum();
            let total_spheres: u32 = col.entries.iter().map(|e| e.num_spheres).sum();
            let total_boxes: u32 = col.entries.iter().map(|e| e.num_boxes).sum();
            let has_shadow = col.entries.iter().any(|e| e.has_shadow);

            inspection
                .summary
                .push(("Entries".to_string(), format!("{}", col.entries.len())));
            inspection
                .summary
                .push(("Mesh vertices".to_string(), format!("{total_verts}")));
            inspection
                .summary
                .push(("Mesh faces".to_string(), format!("{total_faces}")));
            if total_spheres > 0 {
                inspection
                    .summary
                    .push(("Spheres".to_string(), format!("{total_spheres}")));
            }
            if total_boxes > 0 {
                inspection
                    .summary
                    .push(("Boxes".to_string(), format!("{total_boxes}")));
            }
            if has_shadow {
                inspection.summary.push(("Shadow mesh".to_string(), "Yes".to_string()));
            }
        }
        Err(_) => {}
    }
}

fn inspect_generic(header: &[u8], inspection: &mut EntryInspection) {
    if !header.is_empty() {
        let preview: Vec<String> = header.iter().take(32).map(|b| format!("{:02X}", b)).collect();
        inspection.preview_hex = Some(preview.join(" "));
    }
}

pub fn inspect_entry_cached(archive: &mut ArchiveInfo, index: usize) -> Option<EntryInspection> {
    if let Some(cached) = archive.inspection_cache.get(&index) {
        return Some(cached.clone());
    }

    let entry = archive.entries.get(index)?.clone();
    let inspection = inspect_entry(archive, &entry);
    archive.inspection_cache.insert(index, inspection.clone());
    Some(inspection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use crate::archive::EntryInfo;
    use crate::parser::ImgVersion;

    fn temp_file_with(content: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("entry.bin");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content).unwrap();
        (dir, path)
    }

    #[test]
    fn inspect_text_counts_lines() {
        let (_dir, path) = temp_file_with(b"line1\nline2\n\nline3\n");
        let mut archive = ArchiveInfo::new("test", true, ImgVersion::One);
        let mut entry = EntryInfo::new("data.ipl");
        entry.source_path = Some(path);
        entry.imported = true;
        entry.sector = 1;
        archive.entries.push(entry);

        let inspection = inspect_entry_cached(&mut archive, 0).unwrap();
        assert_eq!(inspection.file_name.as_str(), "data.ipl");
        assert!(inspection.summary.iter().any(|(k, v)| k == "Lines" && v == "4 (3 non-empty)"));
        assert!(inspection.summary.iter().any(|(k, _)| k == "Format"));
    }

    #[test]
    fn inspect_collision_detects_version() {
        let (_dir, path) = temp_file_with(b"COL2this is padding");
        let mut archive = ArchiveInfo::new("test", true, ImgVersion::One);
        let mut entry = EntryInfo::new("coll.col");
        entry.source_path = Some(path);
        entry.imported = true;
        entry.sector = 1;
        archive.entries.push(entry);

        let inspection = inspect_entry_cached(&mut archive, 0).unwrap();
        assert!(inspection
            .summary
            .iter()
            .any(|(k, v)| k == "Version" && v == "GTA SA (COL2)"));
    }

    #[test]
    fn inspect_generic_shows_hex_preview() {
        let (_dir, path) = temp_file_with(b"\x00\x01\x02\x03\x04\x05");
        let mut archive = ArchiveInfo::new("test", true, ImgVersion::One);
        let mut entry = EntryInfo::new("unknown.xyz");
        entry.source_path = Some(path);
        entry.imported = true;
        entry.sector = 1;
        archive.entries.push(entry);

        let inspection = inspect_entry_cached(&mut archive, 0).unwrap();
        assert!(inspection.preview_hex.as_ref().unwrap().starts_with("00 01 02 03"));
    }

    #[test]
    fn inspect_entry_uses_cache() {
        let (_dir, path) = temp_file_with(b"cache test");
        let mut archive = ArchiveInfo::new("test", true, ImgVersion::One);
        let mut entry = EntryInfo::new("data.txt");
        entry.source_path = Some(path);
        entry.imported = true;
        entry.sector = 1;
        archive.entries.push(entry);

        let first = inspect_entry_cached(&mut archive, 0).unwrap();
        let cached = archive.inspection_cache.get(&0).cloned().unwrap();
        assert_eq!(first.file_name, cached.file_name);
    }
}
