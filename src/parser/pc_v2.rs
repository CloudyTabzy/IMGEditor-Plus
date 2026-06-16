use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use memmap2::Mmap;

use crate::archive::{ArchiveInfo, EntryInfo};
use crate::parser::{
    ENTRY_SIZE, ImgParser, MAX_ENTRY_NAME_BYTES, SECTOR_SIZE, decode_entry_name,
    export_entry_to_file, import_entry, read_entry_data_with_source,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct PcV2Parser;

impl PcV2Parser {
    fn dir_path(img_path: &Path) -> PathBuf {
        let mut path = img_path.to_path_buf();
        path.set_extension("dir");
        path
    }
}

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

        archive.source_mmap = Some(Arc::new(unsafe { Mmap::map(&img)? }));

        archive.add_log("Opened archive".to_string());
        Ok(())
    }

    fn export_entry(
        &self,
        archive: &ArchiveInfo,
        entry: &EntryInfo,
        output_path: &Path,
    ) -> Result<()> {
        export_entry_to_file(archive, entry, output_path)
    }

    fn import_entry(archive: &mut ArchiveInfo, path: &Path, replace: bool) -> Result<()> {
        import_entry(archive, path, replace)
    }

    fn save(
        &self,
        archive: &mut ArchiveInfo,
        output_path: &Path,
        remove_existing: bool,
    ) -> Result<()> {
        let source_path = archive.path.clone();

        let mut temp_path = output_path.as_os_str().to_owned();
        temp_path.push(".temp");
        let temp_path = PathBuf::from(temp_path);

        let result = self.save_internal(archive, &temp_path, &source_path);

        if result.is_err() {
            let _ = std::fs::remove_file(&temp_path);
        }

        result?;

        if remove_existing {
            if let Some(ref src) = source_path {
                if src != output_path {
                    let _ = std::fs::remove_file(src);
                }
            }
        }

        std::fs::rename(&temp_path, output_path).context("failed to write archive file")?;

        archive.path = Some(output_path.to_path_buf());
        archive.file_name = output_path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_string());
        archive.version = crate::parser::ImgVersion::Two;
        archive.add_log("Archive saved".to_string());
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

impl PcV2Parser {
    fn save_internal(
        &self,
        archive: &mut ArchiveInfo,
        temp_path: &Path,
        _source_path: &Option<PathBuf>,
    ) -> Result<()> {
        let mut out = std::fs::File::create(temp_path).context("failed to create temp img")?;

        out.write_all(b"VER2")?;
        let total = archive.entries.len() as u32;
        out.write_all(&total.to_le_bytes())?;

        let mut data_offset = 0x300000_u64;
        let source_path = archive.path.clone();
        let source_mmap = archive.source_mmap.clone();
        archive.progress.start();

        for (index, entry) in archive.entries.iter_mut().enumerate() {
            if archive.progress.is_cancelled() {
                archive.progress.finish();
                anyhow::bail!("Rebuild cancelled");
            }

            let mut data = read_entry_data_with_source(
                entry,
                source_path.as_deref(),
                source_mmap.as_deref(),
            )?;

            let size = data.len() as u64;
            entry.offset = (data_offset / SECTOR_SIZE) as u32;
            entry.sector = (size / SECTOR_SIZE) as u32;

            let dir_offset = 0x8_u64 + (index as u64) * ENTRY_SIZE as u64;
            out.seek(SeekFrom::Start(dir_offset))?;
            out.write_all(&entry.offset.to_le_bytes())?;
            out.write_all(&entry.sector.to_le_bytes())?;
            out.write_all(&entry.file_name_raw)?;

            out.seek(SeekFrom::Start(data_offset))?;
            out.write_all(&mut data)?;

            data_offset += size;
            archive
                .progress
                .set_percentage((index + 1) as f32 / total as f32);
        }

        archive.progress.set_percentage(1.0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::archive::ArchiveInfo;
    use crate::parser::{SECTOR_SIZE, encode_entry_name, sector_rounded_size};

    fn create_v2_archive(dir: &Path, name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
        let img_path = dir.join(format!("{}.img", name));
        let mut img = std::fs::File::create(&img_path).unwrap();

        img.write_all(b"VER2").unwrap();
        img.write_all(&(entries.len() as u32).to_le_bytes())
            .unwrap();

        let header_size = 8 + entries.len() * ENTRY_SIZE;
        let data_start = SECTOR_SIZE;
        let mut offset = (data_start / SECTOR_SIZE) as u32;

        for (entry_name, data) in entries {
            let rounded_len = sector_rounded_size(data.len() as u64);
            let sector_count = (rounded_len / SECTOR_SIZE) as u32;
            let raw = encode_entry_name(entry_name);

            img.write_all(&offset.to_le_bytes()).unwrap();
            img.write_all(&sector_count.to_le_bytes()).unwrap();
            img.write_all(&raw).unwrap();

            offset += sector_count;
        }

        img.write_all(&vec![0u8; (data_start - header_size as u64) as usize])
            .unwrap();

        for (_entry_name, data) in entries {
            let rounded_len = sector_rounded_size(data.len() as u64);
            let mut padded = data.to_vec();
            padded.resize(rounded_len as usize, 0);
            img.write_all(&padded).unwrap();
        }

        img_path
    }

    #[test]
    fn open_reads_entries() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_v2_archive(
            dir.path(),
            "test",
            &[("player.dff", b"dff data"), ("texture.txd", b"txd data")],
        );

        let archive = ArchiveInfo::open(&img_path).unwrap();
        assert_eq!(archive.entries.len(), 2);
        assert_eq!(archive.entries[0].file_name, "player.dff");
        assert_eq!(archive.entries[0].file_type, "Model");
        assert_eq!(archive.entries[1].file_name, "texture.txd");
        assert_eq!(archive.entries[1].file_type, "Texture");
    }

    #[test]
    fn export_entry_writes_sectors() {
        let dir = tempfile::tempdir().unwrap();
        let data = b"export me";
        let img_path = create_v2_archive(dir.path(), "test", &[("entry.dff", data)]);

        let archive = ArchiveInfo::open(&img_path).unwrap();
        let output = dir.path().join("entry.dff");
        PcV2Parser
            .export_entry(&archive, &archive.entries[0], &output)
            .unwrap();

        let exported = std::fs::read(&output).unwrap();
        assert_eq!(&exported[..data.len()], data.as_slice());
        assert_eq!(exported.len() as u64, SECTOR_SIZE);
    }

    #[test]
    fn import_entry_adds_file() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_v2_archive(dir.path(), "test", &[("entry.dff", b"data")]);
        let import_path = dir.path().join("new.txd");
        std::fs::write(&import_path, b"txd content").unwrap();

        let mut archive = ArchiveInfo::open(&img_path).unwrap();
        PcV2Parser::import_entry(&mut archive, &import_path, false).unwrap();

        assert_eq!(archive.entries.len(), 2);
        assert_eq!(archive.entries[1].file_name, "new.txd");
        assert!(archive.entries[1].imported);
    }

    #[test]
    fn save_rebuilds_archive() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_v2_archive(
            dir.path(),
            "test",
            &[("player.dff", b"dff"), ("texture.txd", b"txd")],
        );

        let mut archive = ArchiveInfo::open(&img_path).unwrap();
        let save_path = dir.path().join("saved.img");
        PcV2Parser.save(&mut archive, &save_path, false).unwrap();

        assert!(save_path.exists());

        let reopened = ArchiveInfo::open(&save_path).unwrap();
        assert_eq!(reopened.entries.len(), 2);
        assert_eq!(reopened.entries[0].file_name, "player.dff");
        assert_eq!(reopened.entries[1].file_name, "texture.txd");
    }

    #[test]
    fn import_and_save_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_v2_archive(dir.path(), "test", &[("entry.dff", b"original")]);
        let import_path = dir.path().join("new.txd");
        let import_data = b"imported txd";
        std::fs::write(&import_path, import_data).unwrap();

        let mut archive = ArchiveInfo::open(&img_path).unwrap();
        PcV2Parser::import_entry(&mut archive, &import_path, false).unwrap();

        let save_path = dir.path().join("combined.img");
        PcV2Parser.save(&mut archive, &save_path, false).unwrap();

        let reopened = ArchiveInfo::open(&save_path).unwrap();
        assert_eq!(reopened.entries.len(), 2);

        let output = dir.path().join("imported.txd");
        PcV2Parser
            .export_entry(&reopened, &reopened.entries[1], &output)
            .unwrap();
        let exported = std::fs::read(&output).unwrap();
        assert_eq!(&exported[..import_data.len()], import_data.as_slice());
    }
}
