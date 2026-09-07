use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use memmap2::Mmap;
use std::sync::Arc;

use crate::archive::{ArchiveInfo, EntryInfo};
use crate::parser::{
    ImgParser, MAX_ENTRY_NAME_BYTES, SECTOR_SIZE, decode_entry_name, export_entry_to_file,
    import_entry,
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

        let img_file = std::fs::File::open(path).context("failed to open IMG v1 archive")?;
        archive.source_mmap = Some(Arc::new(unsafe { Mmap::map(&img_file)? }));

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
        let dir_path = Self::dir_path(output_path);

        let mut temp_img = output_path.as_os_str().to_owned();
        temp_img.push(".temp");
        let temp_img = PathBuf::from(temp_img);

        let mut temp_dir = dir_path.as_os_str().to_owned();
        temp_dir.push(".temp");
        let temp_dir = PathBuf::from(temp_dir);

        let result = self.save_internal(archive, output_path, &temp_img, &temp_dir, &source_path);

        if result.is_err() {
            let _ = std::fs::remove_file(&temp_img);
            let _ = std::fs::remove_file(&temp_dir);
        }

        result?;

        // Release the old mapping before replacing an in-place archive, then
        // map the file that was actually written.
        archive.source_mmap = None;

        let _ = std::fs::remove_file(output_path);
        std::fs::rename(&temp_dir, &dir_path).context("failed to write directory file")?;

        if remove_existing
            && let Some(ref src) = source_path
            && src != output_path
        {
            let _ = std::fs::remove_file(src);
        }

        std::fs::rename(&temp_img, output_path).context("failed to write archive file")?;

        archive.path = Some(output_path.to_path_buf());
        archive.file_name = output_path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_string());
        archive.version = crate::parser::ImgVersion::One;
        let img_file = std::fs::File::open(output_path)
            .context("failed to reopen packed IMG v1 archive")?;
        archive.source_mmap = Some(Arc::new(unsafe { Mmap::map(&img_file)? }));
        archive.add_log("Archive saved".to_string());
        Ok(())
    }

    fn version_text(&self) -> &'static str {
        "PC v1"
    }

    fn is_valid(&self, path: &Path) -> bool {
        path.exists() && Self::dir_path(path).exists()
    }
}

impl PcV1Parser {
    fn save_internal(
        &self,
        archive: &mut ArchiveInfo,
        _output_path: &Path,
        temp_img: &Path,
        temp_dir: &Path,
        _source_path: &Option<PathBuf>,
    ) -> Result<()> {
        const WRITE_BUF: usize = 1024 * 1024;

        let mut img_out = BufWriter::with_capacity(
            WRITE_BUF,
            std::fs::File::create(temp_img).context("failed to create temp img")?,
        );
        let mut dir_out = BufWriter::with_capacity(
            WRITE_BUF,
            std::fs::File::create(temp_dir).context("failed to create temp dir")?,
        );

        let total = archive.entries.len();
        let source_path = archive.path.clone();
        let source_mmap = archive.source_mmap.clone();
        archive.progress.start();

        // Layout pass: sizes come from existing metadata (or an imported
        // file's length); no entry data is read here.
        let mut layout = Vec::with_capacity(total);
        for entry in archive.entries.iter() {
            layout.push(crate::parser::entry_data_size(entry, source_mmap.as_deref())?);
        }

        // Write pass: both streams advance sequentially, and archive-backed
        // data comes straight from the source memory map (no per-entry Vec).
        let mut offset = 0u64;
        let mut source_file = None;
        for (index, (entry, &size)) in archive.entries.iter().zip(layout.iter()).enumerate() {
            if archive.progress.is_cancelled() {
                archive.progress.finish();
                anyhow::bail!("Rebuild cancelled");
            }

            dir_out.write_all(&((offset / SECTOR_SIZE) as u32).to_le_bytes())?;
            dir_out.write_all(&((size / SECTOR_SIZE) as u32).to_le_bytes())?;
            dir_out.write_all(&entry.file_name_raw)?;

            crate::parser::stream_entry_data(
                &mut img_out,
                entry,
                source_path.as_deref(),
                source_mmap.as_deref(),
                &mut source_file,
            )?;

            offset += size;
            if index % 64 == 0 || index + 1 == total {
                archive
                    .progress
                    .set_percentage((index + 1) as f32 / total as f32);
            }
        }

        dir_out.flush()?;
        img_out.flush()?;

        // Apply the new layout to the in-memory entries.
        let mut offset = 0u64;
        for (entry, &size) in archive.entries.iter_mut().zip(layout.iter()) {
            entry.offset = (offset / SECTOR_SIZE) as u32;
            entry.sector = (size / SECTOR_SIZE) as u32;
            offset += size;
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

    fn create_v1_archive(dir: &Path, name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
        let img_path = dir.join(format!("{}.img", name));
        let dir_path = dir.join(format!("{}.dir", name));

        let mut img = std::fs::File::create(&img_path).unwrap();
        let mut records = Vec::new();
        let mut offset = 0u32;

        for (entry_name, data) in entries {
            let rounded_len = sector_rounded_size(data.len() as u64);
            let mut padded = data.to_vec();
            padded.resize(rounded_len as usize, 0);
            img.write_all(&padded).unwrap();

            let sector_count = (rounded_len / SECTOR_SIZE) as u32;
            let raw = encode_entry_name(entry_name);
            records.extend_from_slice(&offset.to_le_bytes());
            records.extend_from_slice(&sector_count.to_le_bytes());
            records.extend_from_slice(&raw);

            offset += sector_count;
        }

        std::fs::write(&dir_path, records).unwrap();
        img_path
    }

    #[test]
    fn open_reads_entries() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_v1_archive(
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
        let img_path = create_v1_archive(dir.path(), "test", &[("entry.dff", data)]);

        let archive = ArchiveInfo::open(&img_path).unwrap();
        let output = dir.path().join("entry.dff");
        PcV1Parser
            .export_entry(&archive, &archive.entries[0], &output)
            .unwrap();

        let exported = std::fs::read(&output).unwrap();
        assert_eq!(&exported[..data.len()], data.as_slice());
        assert_eq!(exported.len() as u64, SECTOR_SIZE);
    }

    #[test]
    fn export_avoids_overwriting_existing() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_v1_archive(dir.path(), "test", &[("entry.dff", b"data")]);

        let archive = ArchiveInfo::open(&img_path).unwrap();
        let existing = dir.path().join("entry.dff");
        std::fs::write(&existing, b"existing").unwrap();

        PcV1Parser
            .export_entry(&archive, &archive.entries[0], &existing)
            .unwrap();

        let saved = dir.path().join("entry (2).dff");
        assert!(saved.exists());
    }

    #[test]
    fn import_entry_adds_file() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_v1_archive(dir.path(), "test", &[("entry.dff", b"data")]);
        let import_path = dir.path().join("new.txd");
        std::fs::write(&import_path, b"txd content").unwrap();

        let mut archive = ArchiveInfo::open(&img_path).unwrap();
        PcV1Parser::import_entry(&mut archive, &import_path, false).unwrap();

        assert_eq!(archive.entries.len(), 2);
        assert_eq!(archive.entries[1].file_name, "new.txd");
        assert!(archive.entries[1].imported);
    }

    #[test]
    fn save_rebuilds_archive() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_v1_archive(
            dir.path(),
            "test",
            &[("player.dff", b"dff"), ("texture.txd", b"txd")],
        );

        let mut archive = ArchiveInfo::open(&img_path).unwrap();
        let save_path = dir.path().join("saved.img");
        PcV1Parser.save(&mut archive, &save_path, false).unwrap();

        assert!(save_path.exists());
        assert!(dir.path().join("saved.dir").exists());

        let reopened = ArchiveInfo::open(&save_path).unwrap();
        assert_eq!(reopened.entries.len(), 2);
        assert_eq!(reopened.entries[0].file_name, "player.dff");
        assert_eq!(reopened.entries[1].file_name, "texture.txd");
    }

    #[test]
    fn save_in_place_preserves_data() {
        let dir = tempfile::tempdir().unwrap();
        let data = b"preserve me";
        let img_path = create_v1_archive(dir.path(), "test", &[("entry.dff", data)]);

        let mut archive = ArchiveInfo::open(&img_path).unwrap();
        PcV1Parser.save(&mut archive, &img_path, true).unwrap();

        let reopened = ArchiveInfo::open(&img_path).unwrap();
        assert_eq!(reopened.entries.len(), 1);

        let output = dir.path().join("re-export.dff");
        PcV1Parser
            .export_entry(&reopened, &reopened.entries[0], &output)
            .unwrap();
        let exported = std::fs::read(&output).unwrap();
        assert_eq!(&exported[..data.len()], data.as_slice());
    }

    #[test]
    fn import_and_save_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_v1_archive(dir.path(), "test", &[("entry.dff", b"original")]);
        let import_path = dir.path().join("new.txd");
        let import_data = b"imported txd";
        std::fs::write(&import_path, import_data).unwrap();

        let mut archive = ArchiveInfo::open(&img_path).unwrap();
        PcV1Parser::import_entry(&mut archive, &import_path, false).unwrap();

        let save_path = dir.path().join("combined.img");
        PcV1Parser.save(&mut archive, &save_path, false).unwrap();

        let reopened = ArchiveInfo::open(&save_path).unwrap();
        assert_eq!(reopened.entries.len(), 2);

        let output = dir.path().join("imported.txd");
        PcV1Parser
            .export_entry(&reopened, &reopened.entries[1], &output)
            .unwrap();
        let exported = std::fs::read(&output).unwrap();
        assert_eq!(&exported[..import_data.len()], import_data.as_slice());
    }
}
