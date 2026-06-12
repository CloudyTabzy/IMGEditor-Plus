use std::io::{Read, Seek};
use std::path::Path;

use anyhow::{Context, Result};

use crate::archive::{ArchiveInfo, EntryInfo};
use crate::parser::{ENTRY_SIZE, ImgParser, MAX_ENTRY_NAME_BYTES, decode_entry_name, import_entry};

#[derive(Debug, Default, Clone, Copy)]
pub struct PcV2Parser;

impl ImgParser for PcV2Parser {
    fn open(&self, archive: &mut ArchiveInfo) -> Result<()> {
        let Some(path) = archive.path.as_ref() else {
            anyhow::bail!("new archives do not have a source path");
        };
        let mut img = std::fs::File::open(path).context("failed to open IMG v2 archive")?;

        let mut header = [0_u8; 8];
        img.read_exact(&mut header)?;
        if &header[0..4] != b"VER2" {
            anyhow::bail!("invalid IMG v2 header");
        }

        let total = u32::from_le_bytes(header[4..8].try_into().expect("4 bytes")) as usize;
        archive.entries.clear();

        for _ in 0..total {
            let mut entry_record = [0_u8; ENTRY_SIZE];
            img.read_exact(&mut entry_record)?;
            let offset = u32::from_le_bytes(entry_record[0..4].try_into().expect("4 bytes"));
            let sector = u32::from_le_bytes(entry_record[4..8].try_into().expect("4 bytes"));

            let mut raw = [0u8; MAX_ENTRY_NAME_BYTES];
            raw.copy_from_slice(&entry_record[8..8 + MAX_ENTRY_NAME_BYTES]);

            let mut entry = EntryInfo::new(decode_entry_name(&raw));
            entry.file_name_raw = raw;
            entry.offset = offset;
            entry.sector = sector;
            archive.entries.push(entry);
        }

        archive.add_log("Opened archive".to_string());
        Ok(())
    }

    fn export_entry(
        &self,
        archive: &ArchiveInfo,
        entry: &EntryInfo,
        output_path: &Path,
    ) -> Result<()> {
        let Some(path) = archive.path.as_ref() else {
            anyhow::bail!("archive has no source path");
        };
        let mut img = std::fs::File::open(path).context("failed to open IMG archive")?;
        let output = std::fs::File::create(output_path).context("failed to create output file")?;

        let size = u64::from(entry.sector) * crate::parser::SECTOR_SIZE;
        let offset = u64::from(entry.offset) * crate::parser::SECTOR_SIZE;
        img.seek(std::io::SeekFrom::Start(offset))?;
        std::io::copy(&mut img.take(size), &mut std::io::BufWriter::new(output))?;
        Ok(())
    }

    fn import_entry(archive: &mut ArchiveInfo, path: &Path, replace: bool) -> Result<()> {
        import_entry(archive, path, replace)
    }

    fn save(
        &self,
        _archive: &mut ArchiveInfo,
        _output_path: &Path,
        _remove_existing: bool,
    ) -> Result<()> {
        Ok(())
    }

    fn version_text(&self) -> &'static str {
        "PC v2"
    }

    fn is_valid(&self, path: &Path) -> bool {
        let Ok(mut file) = std::fs::File::open(path) else {
            return false;
        };
        let mut header = [0_u8; 4];
        file.read_exact(&mut header).is_ok() && &header == b"VER2"
    }
}
