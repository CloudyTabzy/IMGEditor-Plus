use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::archive::{ArchiveInfo, EntryInfo};
use crate::parser::{
    ImgParser, MAX_ENTRY_NAME_BYTES, SECTOR_SIZE, decode_entry_name, import_entry,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct PcV1Parser;

impl PcV1Parser {
    fn dir_path(img_path: &Path) -> PathBuf {
        let mut path = img_path.to_path_buf();
        path.set_extension("dir");
        path
    }
}

impl ImgParser for PcV1Parser {
    fn open(&self, archive: &mut ArchiveInfo) -> Result<()> {
        let Some(path) = archive.path.as_ref() else {
            anyhow::bail!("new archives do not have a source path");
        };
        let dir_path = Self::dir_path(path);
        let dir_bytes = std::fs::read(&dir_path)
            .with_context(|| format!("failed to read IMG v1 directory: {}", dir_path.display()))?;

        if dir_bytes.len() % crate::parser::ENTRY_SIZE != 0 {
            anyhow::bail!("invalid IMG v1 directory size");
        }

        archive.entries.clear();
        for chunk in dir_bytes.chunks_exact(crate::parser::ENTRY_SIZE) {
            let offset = u32::from_le_bytes(chunk[0..4].try_into().expect("4 bytes"));
            let sector = u32::from_le_bytes(chunk[4..8].try_into().expect("4 bytes"));

            let mut raw = [0u8; MAX_ENTRY_NAME_BYTES];
            raw.copy_from_slice(&chunk[8..8 + MAX_ENTRY_NAME_BYTES]);

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

        let size = u64::from(entry.sector) * SECTOR_SIZE;
        let offset = u64::from(entry.offset) * SECTOR_SIZE;
        img.seek(std::io::SeekFrom::Start(offset))?;
        std::io::copy(&mut img.take(size), &mut std::io::BufWriter::new(output))?;
        Ok(())
    }

    fn import_entry(archive: &mut ArchiveInfo, path: &Path, replace: bool) -> Result<()> {
        import_entry(archive, path, replace)
    }

    fn save(&self, _archive: &mut ArchiveInfo, _output_path: &Path) -> Result<()> {
        Ok(())
    }

    fn version_text(&self) -> &'static str {
        "PC v1"
    }

    fn is_valid(&self, path: &Path) -> bool {
        path.exists() && Self::dir_path(path).exists()
    }
}
