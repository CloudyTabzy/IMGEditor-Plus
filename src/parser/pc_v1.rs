use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use memmap2::Mmap;
use std::sync::Arc;

use crate::archive::{ArchiveInfo, EntryInfo};
use crate::parser::{
    ImgParser, ImgVersion, MAX_ENTRY_NAME_BYTES, SECTOR_SIZE, decode_entry_name,
    export_entry_to_file, import_entry,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct PcV1Parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum V1ByteOrder {
    Little,
    Big,
}

impl V1ByteOrder {
    fn read_u32(self, bytes: &[u8]) -> u32 {
        let bytes: [u8; 4] = bytes.try_into().expect("four-byte IMG v1 field");
        match self {
            Self::Little => u32::from_le_bytes(bytes),
            Self::Big => u32::from_be_bytes(bytes),
        }
    }

    fn write_u32(self, value: u32) -> [u8; 4] {
        match self {
            Self::Little => value.to_le_bytes(),
            Self::Big => value.to_be_bytes(),
        }
    }
}

impl PcV1Parser {
    pub(crate) fn dir_path(img_path: &Path) -> PathBuf {
        let mut path = img_path.to_path_buf();
        path.set_extension("dir");
        path
    }

    pub(crate) fn open_with_endian(
        &self,
        archive: &mut ArchiveInfo,
        byte_order: V1ByteOrder,
    ) -> Result<()> {
        let Some(path) = archive.path.as_ref() else {
            anyhow::bail!("new archives do not have a source path");
        };
        let dir_path = Self::dir_path(path);
        let dir_bytes = std::fs::read(&dir_path)
            .with_context(|| format!("failed to read IMG v1 directory: {}", dir_path.display()))?;
        let img_len = std::fs::metadata(path)
            .with_context(|| format!("failed to stat IMG v1 archive: {}", path.display()))?
            .len();

        validate_v1_directory(&dir_bytes, img_len, byte_order)
            .map_err(|reason| anyhow::anyhow!("invalid IMG v1 directory: {reason}"))?;

        archive.entries.clear();
        for chunk in dir_bytes.chunks_exact(crate::parser::ENTRY_SIZE) {
            let offset = byte_order.read_u32(&chunk[0..4]);
            let sector = byte_order.read_u32(&chunk[4..8]);

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

    pub(crate) fn is_valid_with_endian(&self, path: &Path, byte_order: V1ByteOrder) -> bool {
        let Ok(img_len) = std::fs::metadata(path).map(|metadata| metadata.len()) else {
            return false;
        };
        let dir_path = Self::dir_path(path);
        let Ok(dir_bytes) = std::fs::read(dir_path) else {
            return false;
        };

        validate_v1_directory(&dir_bytes, img_len, byte_order).is_ok()
    }
}

impl ImgParser for PcV1Parser {
    fn open(&self, archive: &mut ArchiveInfo) -> Result<()> {
        self.open_with_endian(archive, V1ByteOrder::Little)
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
        self.save_with_endian(
            archive,
            output_path,
            remove_existing,
            V1ByteOrder::Little,
            ImgVersion::One,
        )
    }

    fn version_text(&self) -> &'static str {
        "PC v1"
    }

    fn is_valid(&self, path: &Path) -> bool {
        self.is_valid_with_endian(path, V1ByteOrder::Little)
    }
}

impl PcV1Parser {
    pub(crate) fn save_with_endian(
        &self,
        archive: &mut ArchiveInfo,
        output_path: &Path,
        remove_existing: bool,
        byte_order: V1ByteOrder,
        version: ImgVersion,
    ) -> Result<()> {
        let source_path = archive.path.clone();
        let dir_path = Self::dir_path(output_path);

        let mut temp_img = output_path.as_os_str().to_owned();
        temp_img.push(".temp");
        let temp_img = PathBuf::from(temp_img);

        let mut temp_dir = dir_path.as_os_str().to_owned();
        temp_dir.push(".temp");
        let temp_dir = PathBuf::from(temp_dir);

        let result = self.save_internal(
            archive,
            output_path,
            &temp_img,
            &temp_dir,
            &source_path,
            byte_order,
        );

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
        archive.version = version;
        let img_file =
            std::fs::File::open(output_path).context("failed to reopen packed IMG v1 archive")?;
        archive.source_mmap = Some(Arc::new(unsafe { Mmap::map(&img_file)? }));
        archive.add_log("Archive saved".to_string());
        Ok(())
    }

    fn save_internal(
        &self,
        archive: &mut ArchiveInfo,
        _output_path: &Path,
        temp_img: &Path,
        temp_dir: &Path,
        _source_path: &Option<PathBuf>,
        byte_order: V1ByteOrder,
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
            layout.push(crate::parser::entry_data_size(
                entry,
                source_mmap.as_deref(),
            )?);
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

            dir_out.write_all(&byte_order.write_u32((offset / SECTOR_SIZE) as u32))?;
            dir_out.write_all(&byte_order.write_u32((size / SECTOR_SIZE) as u32))?;
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

fn validate_v1_directory(
    dir_bytes: &[u8],
    img_len: u64,
    byte_order: V1ByteOrder,
) -> std::result::Result<(), String> {
    if dir_bytes.len() % crate::parser::ENTRY_SIZE != 0 {
        return Err("directory size is not a multiple of 32 bytes".to_string());
    }

    let mut ranges = Vec::with_capacity(dir_bytes.len() / crate::parser::ENTRY_SIZE);
    let mut named_entries = 0usize;

    for (index, chunk) in dir_bytes
        .chunks_exact(crate::parser::ENTRY_SIZE)
        .enumerate()
    {
        let offset = byte_order.read_u32(&chunk[0..4]);
        let sector = byte_order.read_u32(&chunk[4..8]);
        let raw_name = &chunk[8..8 + MAX_ENTRY_NAME_BYTES];
        let name_len = raw_name
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(raw_name.len());
        let name = &raw_name[..name_len];

        if !name.is_empty() {
            named_entries += 1;
            if name
                .iter()
                .any(|&byte| !(0x20..=0x7E).contains(&byte))
            {
                return Err(format!("entry {index} has a non-printable filename"));
            }
        }

        let Some(end_sector) = offset.checked_add(sector) else {
            return Err(format!("entry {index} sector range overflows"));
        };
        let Some(end_byte) = u64::from(end_sector).checked_mul(SECTOR_SIZE) else {
            return Err(format!("entry {index} byte range overflows"));
        };
        if end_byte > img_len {
            return Err(format!(
                "entry {index} ends at byte {end_byte}, beyond IMG size {img_len}"
            ));
        }

        if sector > 0 {
            ranges.push((offset, end_sector, index));
        }
    }

    if !dir_bytes.is_empty() && named_entries == 0 {
        return Err("directory contains no named entries".to_string());
    }

    ranges.sort_unstable_by_key(|&(start, _, _)| start);
    for pair in ranges.windows(2) {
        let (_, previous_end, previous_index) = pair[0];
        let (next_start, _, next_index) = pair[1];
        if next_start < previous_end {
            return Err(format!(
                "entries {previous_index} and {next_index} overlap"
            ));
        }
    }

    Ok(())
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
