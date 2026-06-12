use std::path::{Path, PathBuf};

use crate::archive::{ArchiveInfo, EntryInfo};

pub mod iparser;
pub mod pc_v1;
pub mod pc_v2;
pub mod unknown;

pub use iparser::ImgParser;
pub use pc_v1::PcV1Parser;
pub use pc_v2::PcV2Parser;
pub use unknown::UnknownParser;

pub const SECTOR_SIZE: u64 = 2048;
pub const ENTRY_SIZE: usize = 32;
pub const MAX_ENTRY_NAME_BYTES: usize = 24;
pub const MAX_ENTRY_NAME_LEN: usize = MAX_ENTRY_NAME_BYTES - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImgVersion {
    One,
    Two,
    Unknown,
}

pub fn detect_version(path: &Path) -> ImgVersion {
    if PcV1Parser.is_valid(path) {
        ImgVersion::One
    } else if PcV2Parser.is_valid(path) {
        ImgVersion::Two
    } else {
        ImgVersion::Unknown
    }
}

pub fn sector_rounded_size(byte_len: u64) -> u64 {
    if byte_len == 0 {
        SECTOR_SIZE
    } else {
        let remainder = byte_len % SECTOR_SIZE;
        if remainder == 0 {
            byte_len
        } else {
            byte_len + SECTOR_SIZE - remainder
        }
    }
}

pub fn decode_entry_name(raw: &[u8; MAX_ENTRY_NAME_BYTES]) -> String {
    let trimmed = raw.split(|&b| b == 0).next().unwrap_or(&[]);
    String::from_utf8_lossy(trimmed).into_owned()
}

pub fn encode_entry_name(name: &str) -> [u8; MAX_ENTRY_NAME_BYTES] {
    let mut raw = [0u8; MAX_ENTRY_NAME_BYTES];
    let mut len = 0;

    for c in name.chars() {
        let mut buf = [0u8; 4];
        let encoded = c.encode_utf8(&mut buf);
        if len + encoded.len() > MAX_ENTRY_NAME_LEN {
            break;
        }
        raw[len..len + encoded.len()].copy_from_slice(encoded.as_bytes());
        len += encoded.len();
    }

    raw
}

pub fn import_entry(archive: &mut ArchiveInfo, path: &Path, replace: bool) -> anyhow::Result<()> {
    if path.extension().is_none() {
        return Ok(());
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("import path is not valid UTF-8"))?;

    if file_name.chars().count() > MAX_ENTRY_NAME_LEN {
        archive.add_log(format!("Skipping {file_name}. Name too large."));
        return Ok(());
    }

    if replace {
        archive
            .entries
            .retain(|entry| !entry.file_name.eq_ignore_ascii_case(file_name));
    }

    let byte_len = std::fs::metadata(path)?.len();
    let mut entry = EntryInfo::new(file_name);
    entry.source_path = Some(path.to_path_buf());
    entry.imported = true;
    entry.sector = (sector_rounded_size(byte_len) / SECTOR_SIZE) as u32;
    archive.entries.push(entry);
    Ok(())
}

pub fn source_path_for_import(path: impl Into<PathBuf>) -> PathBuf {
    path.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn sector_rounded_size_examples() {
        assert_eq!(sector_rounded_size(0), SECTOR_SIZE);
        assert_eq!(sector_rounded_size(1), SECTOR_SIZE);
        assert_eq!(sector_rounded_size(SECTOR_SIZE), SECTOR_SIZE);
        assert_eq!(sector_rounded_size(SECTOR_SIZE + 1), SECTOR_SIZE * 2);
    }

    #[test]
    fn encode_decode_ascii_name() {
        let raw = encode_entry_name("player.dff");
        assert_eq!(&raw[..10], b"player.dff");
        assert_eq!(decode_entry_name(&raw), "player.dff");
    }

    #[test]
    fn encode_pads_with_zeros() {
        let raw = encode_entry_name("x");
        assert_eq!(raw[0], b'x');
        assert_eq!(raw[1], 0);
        assert_eq!(raw[MAX_ENTRY_NAME_BYTES - 1], 0);
    }

    #[test]
    fn encode_truncates_long_names() {
        let name = "a".repeat(30);
        let raw = encode_entry_name(&name);
        assert_eq!(raw[MAX_ENTRY_NAME_LEN], 0);
        assert_eq!(decode_entry_name(&raw), "a".repeat(MAX_ENTRY_NAME_LEN));
    }

    #[test]
    fn encode_unicode_respects_byte_budget() {
        let name = "é".repeat(12);
        let raw = encode_entry_name(&name);
        let decoded = decode_entry_name(&raw);
        assert_eq!(decoded.chars().count(), 11);
    }

    #[test]
    fn detect_img_v1_format() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = dir.path().join("test.img");
        let dir_path = dir.path().join("test.dir");
        std::fs::File::create(&img_path).unwrap();
        std::fs::File::create(&dir_path).unwrap();

        assert_eq!(detect_version(&img_path), ImgVersion::One);
    }

    #[test]
    fn detect_img_v2_format() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = dir.path().join("test.img");
        let mut file = std::fs::File::create(&img_path).unwrap();
        file.write_all(b"VER2").unwrap();
        file.write_all(&0_u32.to_le_bytes()).unwrap();

        assert_eq!(detect_version(&img_path), ImgVersion::Two);
    }

    #[test]
    fn detect_unknown_format() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = dir.path().join("test.bin");
        std::fs::File::create(&img_path).unwrap();

        assert_eq!(detect_version(&img_path), ImgVersion::Unknown);
    }
}
