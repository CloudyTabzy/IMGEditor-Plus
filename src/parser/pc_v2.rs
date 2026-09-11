use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use memmap2::Mmap;

use crate::archive::{ArchiveInfo, EntryInfo, ImgV2Size};
use crate::parser::{
    ENTRY_SIZE, ImgParser, MAX_ENTRY_NAME_BYTES, SECTOR_SIZE, decode_entry_name,
    export_entry_to_file, import_entry,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct PcV2Parser;

const V2_HEADER_SIZE: u64 = 8;

#[derive(Debug, Clone, Copy)]
struct V2LayoutEntry {
    offset: u32,
    data_size: u64,
    size_words: ImgV2Size,
}

/// Returns the first sector-aligned data byte after an IMG v2 directory.
pub(crate) fn data_start_for_entry_count(entry_count: usize) -> Result<u64> {
    let entry_count = u64::try_from(entry_count)
        .map_err(|_| anyhow::anyhow!("IMG v2 has too many directory entries"))?;
    data_start_for_count(entry_count)
}

pub(crate) fn directory_end_for_entry_count(entry_count: usize) -> Result<u64> {
    let entry_count = u64::try_from(entry_count)
        .map_err(|_| anyhow::anyhow!("IMG v2 has too many directory entries"))?;
    directory_end_for_count(entry_count)
}

fn directory_end_for_count(entry_count: u64) -> Result<u64> {
    let directory_bytes = entry_count
        .checked_mul(ENTRY_SIZE as u64)
        .ok_or_else(|| anyhow::anyhow!("IMG v2 directory table is too large"))?;
    V2_HEADER_SIZE
        .checked_add(directory_bytes)
        .ok_or_else(|| anyhow::anyhow!("IMG v2 directory table is too large"))
}

fn data_start_for_count(entry_count: u64) -> Result<u64> {
    let directory_end = directory_end_for_count(entry_count)?;
    let padded_end = directory_end
        .checked_add(SECTOR_SIZE - 1)
        .ok_or_else(|| anyhow::anyhow!("IMG v2 directory table is too large"))?;
    Ok((padded_end / SECTOR_SIZE) * SECTOR_SIZE)
}

fn validate_v2_name(raw: &[u8; MAX_ENTRY_NAME_BYTES], index: usize) -> Result<bool> {
    let name_len = raw.iter().position(|&byte| byte == 0).unwrap_or(raw.len());
    let name = &raw[..name_len];

    if name.is_empty() {
        return Ok(false);
    }
    if name.iter().any(|&byte| !(0x20..=0x7E).contains(&byte)) {
        anyhow::bail!("entry {index} has a non-printable filename");
    }

    Ok(true)
}

fn checked_entry_range(offset: u32, sector_count: u32, index: usize) -> Result<(u64, u64)> {
    let start = u64::from(offset)
        .checked_mul(SECTOR_SIZE)
        .ok_or_else(|| anyhow::anyhow!("entry {index} byte offset overflows"))?;
    let size = u64::from(sector_count)
        .checked_mul(SECTOR_SIZE)
        .ok_or_else(|| anyhow::anyhow!("entry {index} byte size overflows"))?;
    let end = start
        .checked_add(size)
        .ok_or_else(|| anyhow::anyhow!("entry {index} byte range overflows"))?;
    Ok((start, end))
}

fn v2_size_words_for_write(entry: &EntryInfo, data_size: u64) -> Result<ImgV2Size> {
    if !data_size.is_multiple_of(SECTOR_SIZE) {
        anyhow::bail!(
            "entry {} is not aligned to the IMG sector size",
            entry.file_name
        );
    }

    let sector_count = u16::try_from(data_size / SECTOR_SIZE).map_err(|_| {
        anyhow::anyhow!(
            "entry {} is too large for IMG v2 (maximum is {} sectors)",
            entry.file_name,
            u16::MAX
        )
    })?;

    match entry.v2_size {
        Some(size_words) if size_words.effective_sector_count() == u32::from(sector_count) => {
            Ok(size_words)
        }
        _ => Ok(ImgV2Size::canonical(sector_count)),
    }
}

impl ImgParser for PcV2Parser {
    fn open(&self, archive: &mut ArchiveInfo) -> Result<()> {
        let Some(path) = archive.path.as_ref() else {
            anyhow::bail!("new archives do not have a source path");
        };
        let mut img = std::fs::File::open(path).context("failed to open IMG v2 archive")?;

        let img_len = img
            .metadata()
            .context("failed to inspect IMG v2 archive")?
            .len();
        let entries = Self::read_validated_entries(&mut img, img_len)?;
        let source_mmap = Arc::new(unsafe { Mmap::map(&img)? });

        archive.entries = entries;
        archive.source_mmap = Some(source_mmap);

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

        let result = self.save_internal(archive, &temp_path);
        archive.progress.finish();

        if result.is_err() {
            let _ = std::fs::remove_file(&temp_path);
        }

        result?;

        // Release the old mapping before replacing an in-place archive, then
        // map the file that was actually written.
        archive.source_mmap = None;

        if remove_existing
            && let Some(ref src) = source_path
            && src != output_path
        {
            let _ = std::fs::remove_file(src);
        }

        std::fs::rename(&temp_path, output_path).context("failed to write archive file")?;

        archive.path = Some(output_path.to_path_buf());
        archive.file_name = output_path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_string());
        archive.version = crate::parser::ImgVersion::Two;
        let img_file =
            std::fs::File::open(output_path).context("failed to reopen packed IMG v2 archive")?;
        archive.source_mmap = Some(Arc::new(unsafe { Mmap::map(&img_file)? }));
        archive.add_log("Archive saved".to_string());
        Ok(())
    }

    fn version_text(&self) -> &'static str {
        "PC v2"
    }

    fn is_valid(&self, path: &Path) -> bool {
        let Ok(mut img) = std::fs::File::open(path) else {
            return false;
        };
        let Ok(img_len) = img.metadata().map(|metadata| metadata.len()) else {
            return false;
        };
        Self::read_validated_entries(&mut img, img_len).is_ok()
    }
}

impl PcV2Parser {
    fn read_validated_entries(img: &mut std::fs::File, img_len: u64) -> Result<Vec<EntryInfo>> {
        img.seek(SeekFrom::Start(0))?;

        let mut header = [0_u8; V2_HEADER_SIZE as usize];
        img.read_exact(&mut header)
            .context("failed to read IMG v2 header")?;
        if &header[0..4] != b"VER2" {
            anyhow::bail!("invalid IMG v2 header");
        }

        let entry_count = u32::from_le_bytes(header[4..8].try_into().expect("4 bytes"));
        let directory_end = directory_end_for_count(u64::from(entry_count))?;
        if directory_end > img_len {
            anyhow::bail!(
                "IMG v2 directory table ends at byte {directory_end}, beyond IMG size {img_len}"
            );
        }
        let data_start = data_start_for_count(u64::from(entry_count))?;
        let entry_count = usize::try_from(entry_count)
            .map_err(|_| anyhow::anyhow!("IMG v2 has too many directory entries"))?;

        let mut entries = Vec::new();
        entries
            .try_reserve(entry_count)
            .map_err(|_| anyhow::anyhow!("IMG v2 directory has too many entries to load safely"))?;
        let mut named_entries = 0usize;

        for index in 0..entry_count {
            let mut entry_record = [0_u8; ENTRY_SIZE];
            img.read_exact(&mut entry_record)
                .with_context(|| format!("failed to read IMG v2 entry {index}"))?;
            let offset = u32::from_le_bytes(entry_record[0..4].try_into().expect("4 bytes"));
            let size_words = ImgV2Size {
                streaming_size: u16::from_le_bytes(entry_record[4..6].try_into().expect("2 bytes")),
                archive_size: u16::from_le_bytes(entry_record[6..8].try_into().expect("2 bytes")),
            };

            let mut raw = [0u8; MAX_ENTRY_NAME_BYTES];
            raw.copy_from_slice(&entry_record[8..8 + MAX_ENTRY_NAME_BYTES]);
            if validate_v2_name(&raw, index)? {
                named_entries += 1;
            }

            let effective_sector_count = size_words.effective_sector_count();
            let (start, end) = checked_entry_range(offset, effective_sector_count, index)?;
            if end > img_len {
                anyhow::bail!("entry {index} ends at byte {end}, beyond IMG size {img_len}");
            }
            if effective_sector_count > 0 && start < data_start {
                anyhow::bail!("entry {index} starts at byte {start}, inside the IMG v2 directory");
            }

            let mut entry = EntryInfo::new(decode_entry_name(&raw));
            entry.file_name_raw = raw;
            entry.offset = offset;
            entry.sector = effective_sector_count;
            entry.v2_size = Some(size_words);
            entries.push(entry);
        }

        if entry_count > 0 && named_entries == 0 {
            anyhow::bail!("IMG v2 directory contains no named entries");
        }

        Ok(entries)
    }

    fn save_internal(&self, archive: &mut ArchiveInfo, temp_path: &Path) -> Result<()> {
        const WRITE_BUF: usize = 1024 * 1024;
        let total = archive.entries.len();
        let total_u32 = u32::try_from(total)
            .map_err(|_| anyhow::anyhow!("IMG v2 has too many entries to save"))?;
        let data_start = if total == 0 {
            V2_HEADER_SIZE
        } else {
            data_start_for_entry_count(total)?
        };
        let source_path = archive.path.clone();
        let source_mmap = archive.source_mmap.clone();
        archive.progress.start();

        // Layout pass: metadata only, no entry data reads.
        let mut layout = Vec::new();
        layout
            .try_reserve(total)
            .map_err(|_| anyhow::anyhow!("IMG v2 has too many entries to save safely"))?;
        let mut next_data_offset = data_start;
        let mut named_entries = 0usize;
        for (index, entry) in archive.entries.iter().enumerate() {
            if validate_v2_name(&entry.file_name_raw, index)? {
                named_entries += 1;
            }
            let data_size = crate::parser::entry_data_size(entry, source_mmap.as_deref())?;
            let size_words = v2_size_words_for_write(entry, data_size)?;
            let offset = if data_size == 0 {
                0
            } else {
                u32::try_from(next_data_offset / SECTOR_SIZE).map_err(|_| {
                    anyhow::anyhow!("IMG v2 output offset is too large for {}", entry.file_name)
                })?
            };
            next_data_offset = next_data_offset
                .checked_add(data_size)
                .ok_or_else(|| anyhow::anyhow!("IMG v2 output is too large"))?;
            layout.push(V2LayoutEntry {
                offset,
                data_size,
                size_words,
            });
        }
        if total > 0 && named_entries == 0 {
            anyhow::bail!("IMG v2 directory contains no named entries");
        }

        let mut out = BufWriter::with_capacity(
            WRITE_BUF,
            std::fs::File::create(temp_path).context("failed to create temp img")?,
        );

        // Directory pass: header + all records in one sequential stream.
        out.write_all(b"VER2")?;
        out.write_all(&total_u32.to_le_bytes())?;
        for (entry, layout_entry) in archive.entries.iter().zip(layout.iter()) {
            out.write_all(&layout_entry.offset.to_le_bytes())?;
            out.write_all(&layout_entry.size_words.streaming_size.to_le_bytes())?;
            out.write_all(&layout_entry.size_words.archive_size.to_le_bytes())?;
            out.write_all(&entry.file_name_raw)?;
        }

        // Data pass: BufWriter::seek flushes first, so this seeks once to the
        // data region and then streams every entry sequentially.
        if layout.iter().any(|entry| entry.data_size > 0) {
            out.seek(SeekFrom::Start(data_start))?;
        }
        let mut source_file = None;
        for (index, entry) in archive.entries.iter().enumerate() {
            if archive.progress.is_cancelled() {
                archive.progress.finish();
                anyhow::bail!("Rebuild cancelled");
            }
            crate::parser::stream_entry_data(
                &mut out,
                entry,
                source_path.as_deref(),
                source_mmap.as_deref(),
                &mut source_file,
            )?;
            if index % 64 == 0 || index + 1 == total {
                archive
                    .progress
                    .set_percentage((index + 1) as f32 / total as f32);
            }
        }
        out.flush()?;

        // Apply the new layout to the in-memory entries.
        for (entry, layout_entry) in archive.entries.iter_mut().zip(layout.iter()) {
            entry.offset = layout_entry.offset;
            entry.sector = layout_entry.size_words.effective_sector_count();
            entry.v2_size = Some(layout_entry.size_words);
        }

        archive.progress.set_percentage(1.0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Seek, SeekFrom, Write};

    use super::*;
    use crate::archive::{ArchiveInfo, ImgV2Size};
    use crate::parser::{SECTOR_SIZE, encode_entry_name, read_entry_data, sector_rounded_size};

    fn write_v2_record(
        output: &mut impl Write,
        offset: u32,
        size_words: ImgV2Size,
        raw_name: [u8; MAX_ENTRY_NAME_BYTES],
    ) {
        output.write_all(&offset.to_le_bytes()).unwrap();
        output
            .write_all(&size_words.streaming_size.to_le_bytes())
            .unwrap();
        output
            .write_all(&size_words.archive_size.to_le_bytes())
            .unwrap();
        output.write_all(&raw_name).unwrap();
    }

    fn create_single_v2_archive(
        dir: &Path,
        name: &str,
        offset: u32,
        size_words: ImgV2Size,
        raw_name: [u8; MAX_ENTRY_NAME_BYTES],
        image_len: u64,
        data: &[u8],
    ) -> PathBuf {
        let img_path = dir.join(format!("{name}.img"));
        let mut img = std::fs::File::create(&img_path).unwrap();
        img.write_all(b"VER2").unwrap();
        img.write_all(&1_u32.to_le_bytes()).unwrap();
        write_v2_record(&mut img, offset, size_words, raw_name);
        img.set_len(image_len).unwrap();
        if !data.is_empty() {
            img.seek(SeekFrom::Start(u64::from(offset) * SECTOR_SIZE))
                .unwrap();
            img.write_all(data).unwrap();
        }
        img_path
    }

    fn create_v2_archive(dir: &Path, name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
        let img_path = dir.join(format!("{}.img", name));
        let mut img = std::fs::File::create(&img_path).unwrap();

        img.write_all(b"VER2").unwrap();
        img.write_all(&u32::try_from(entries.len()).unwrap().to_le_bytes())
            .unwrap();

        let header_size = V2_HEADER_SIZE + (entries.len() * ENTRY_SIZE) as u64;
        let data_start = data_start_for_entry_count(entries.len()).unwrap();
        let mut offset = u32::try_from(data_start / SECTOR_SIZE).unwrap();

        for (entry_name, data) in entries {
            let rounded_len = sector_rounded_size(data.len() as u64);
            let sector_count = u16::try_from(rounded_len / SECTOR_SIZE).unwrap();
            let raw = encode_entry_name(entry_name);

            write_v2_record(&mut img, offset, ImgV2Size::canonical(sector_count), raw);

            offset += u32::from(sector_count);
        }

        img.write_all(&vec![0u8; (data_start - header_size) as usize])
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
        assert_eq!(archive.entries[0].v2_size, Some(ImgV2Size::canonical(1)));
    }

    #[test]
    fn open_uses_archive_size_when_streaming_size_is_zero() {
        let dir = tempfile::tempdir().unwrap();
        let size_words = ImgV2Size {
            streaming_size: 0,
            archive_size: 1,
        };
        let payload = vec![b'F'; SECTOR_SIZE as usize];
        let img_path = create_single_v2_archive(
            dir.path(),
            "fallback",
            1,
            size_words,
            encode_entry_name("fallback.dff"),
            SECTOR_SIZE * 2,
            &payload,
        );

        let archive = ArchiveInfo::open(&img_path).unwrap();
        assert_eq!(archive.entries[0].sector, 1);
        assert_eq!(archive.entries[0].v2_size, Some(size_words));
        assert_eq!(
            read_entry_data(&archive, &archive.entries[0]).unwrap(),
            payload
        );
    }

    #[test]
    fn open_rejects_truncated_table_without_mutating_existing_entries() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = dir.path().join("truncated.img");
        std::fs::write(
            &img_path,
            [b"VER2".as_slice(), &1_u32.to_le_bytes()].concat(),
        )
        .unwrap();

        let mut archive = ArchiveInfo::new("truncated", false, crate::parser::ImgVersion::Two);
        archive.entries.push(EntryInfo::new("kept.dff"));
        archive.path = Some(img_path.clone());

        let error = PcV2Parser.open(&mut archive).unwrap_err();
        assert!(error.to_string().contains("directory table"));
        assert_eq!(archive.entries.len(), 1);
        assert_eq!(archive.entries[0].file_name, "kept.dff");
        assert!(!PcV2Parser.is_valid(&img_path));
    }

    #[test]
    fn open_rejects_entry_range_outside_the_archive() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_single_v2_archive(
            dir.path(),
            "outside",
            1,
            ImgV2Size::canonical(1),
            encode_entry_name("outside.dff"),
            SECTOR_SIZE,
            &[],
        );

        let mut archive = ArchiveInfo::new("outside", false, crate::parser::ImgVersion::Two);
        archive.path = Some(img_path);
        let error = PcV2Parser.open(&mut archive).unwrap_err();
        assert!(error.to_string().contains("beyond IMG size"));
    }

    #[test]
    fn open_rejects_entry_data_inside_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_single_v2_archive(
            dir.path(),
            "inside-directory",
            0,
            ImgV2Size::canonical(1),
            encode_entry_name("inside.dff"),
            SECTOR_SIZE,
            &[],
        );

        let mut archive =
            ArchiveInfo::new("inside-directory", false, crate::parser::ImgVersion::Two);
        archive.path = Some(img_path);
        let error = PcV2Parser.open(&mut archive).unwrap_err();
        assert!(error.to_string().contains("inside the IMG v2 directory"));
    }

    #[test]
    fn open_rejects_non_printable_filenames() {
        let dir = tempfile::tempdir().unwrap();
        let mut raw_name = encode_entry_name("invalid.dff");
        raw_name[0] = 0x1F;
        let img_path = create_single_v2_archive(
            dir.path(),
            "invalid-name",
            1,
            ImgV2Size::canonical(1),
            raw_name,
            SECTOR_SIZE * 2,
            &[0; SECTOR_SIZE as usize],
        );

        let mut archive = ArchiveInfo::new("invalid-name", false, crate::parser::ImgVersion::Two);
        archive.path = Some(img_path);
        let error = PcV2Parser.open(&mut archive).unwrap_err();
        assert!(error.to_string().contains("non-printable filename"));
    }

    #[test]
    fn open_rejects_a_directory_with_only_blank_names() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_single_v2_archive(
            dir.path(),
            "blank-names",
            1,
            ImgV2Size::canonical(1),
            [0; MAX_ENTRY_NAME_BYTES],
            SECTOR_SIZE * 2,
            &[0; SECTOR_SIZE as usize],
        );

        let mut archive = ArchiveInfo::new("blank-names", false, crate::parser::ImgVersion::Two);
        archive.path = Some(img_path);
        let error = PcV2Parser.open(&mut archive).unwrap_err();
        assert!(error.to_string().contains("contains no named entries"));
    }

    #[test]
    fn open_rejects_an_implausible_count_before_allocating_entries() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = dir.path().join("implausible-count.img");
        std::fs::write(
            &img_path,
            [b"VER2".as_slice(), &u32::MAX.to_le_bytes()].concat(),
        )
        .unwrap();

        let mut archive =
            ArchiveInfo::new("implausible-count", false, crate::parser::ImgVersion::Two);
        archive.path = Some(img_path);
        let error = PcV2Parser.open(&mut archive).unwrap_err();
        assert!(error.to_string().contains("directory table ends"));
    }

    #[test]
    fn compact_data_start_matches_the_directory_size() {
        assert_eq!(data_start_for_entry_count(0).unwrap(), SECTOR_SIZE);
        assert_eq!(data_start_for_entry_count(1).unwrap(), SECTOR_SIZE);
        assert_eq!(data_start_for_entry_count(63).unwrap(), SECTOR_SIZE);
        assert_eq!(data_start_for_entry_count(64).unwrap(), SECTOR_SIZE * 2);
    }

    #[test]
    fn save_preserves_v2_size_words_and_uses_compact_data_start() {
        let dir = tempfile::tempdir().unwrap();
        let size_words = ImgV2Size {
            streaming_size: 0,
            archive_size: 1,
        };
        let payload = vec![b'R'; SECTOR_SIZE as usize];
        let img_path = create_single_v2_archive(
            dir.path(),
            "source",
            1,
            size_words,
            encode_entry_name("source.dff"),
            SECTOR_SIZE * 2,
            &payload,
        );

        let mut archive = ArchiveInfo::open(&img_path).unwrap();
        let saved_path = dir.path().join("saved.img");
        PcV2Parser.save(&mut archive, &saved_path, false).unwrap();

        let bytes = std::fs::read(&saved_path).unwrap();
        assert_eq!(bytes.len() as u64, SECTOR_SIZE * 2);
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(bytes[12..14].try_into().unwrap()), 0);
        assert_eq!(u16::from_le_bytes(bytes[14..16].try_into().unwrap()), 1);

        let reopened = ArchiveInfo::open(&saved_path).unwrap();
        assert_eq!(reopened.entries[0].v2_size, Some(size_words));
        assert_eq!(
            read_entry_data(&reopened, &reopened.entries[0]).unwrap(),
            payload
        );
    }

    #[test]
    fn save_rejects_entries_larger_than_the_v2_size_words() {
        let entry = EntryInfo::new("oversized.dff");
        let size = (u64::from(u16::MAX) + 1) * SECTOR_SIZE;
        assert!(v2_size_words_for_write(&entry, size).is_err());
    }

    #[test]
    fn save_rejects_an_invalid_name_without_leaving_progress_active() {
        let dir = tempfile::tempdir().unwrap();
        let saved_path = dir.path().join("invalid-name.img");
        let mut archive = ArchiveInfo::new("invalid-name", false, crate::parser::ImgVersion::Two);
        let mut entry = EntryInfo::new("invalid.dff");
        entry.file_name_raw[0] = 0x1F;
        archive.entries.push(entry);

        let error = PcV2Parser
            .save(&mut archive, &saved_path, false)
            .unwrap_err();
        assert!(error.to_string().contains("non-printable filename"));
        assert!(!archive.progress.in_use());
        assert!(!saved_path.exists());
    }

    #[test]
    fn zero_length_entry_round_trips_without_an_invalid_data_range() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_single_v2_archive(
            dir.path(),
            "empty-entry",
            0,
            ImgV2Size::canonical(0),
            encode_entry_name("empty.dff"),
            V2_HEADER_SIZE + ENTRY_SIZE as u64,
            &[],
        );

        let mut archive = ArchiveInfo::open(&img_path).unwrap();
        assert_eq!(archive.entries[0].sector, 0);
        let saved_path = dir.path().join("saved.img");
        PcV2Parser.save(&mut archive, &saved_path, false).unwrap();

        assert_eq!(
            std::fs::metadata(&saved_path).unwrap().len(),
            V2_HEADER_SIZE + ENTRY_SIZE as u64
        );
        let reopened = ArchiveInfo::open(&saved_path).unwrap();
        assert_eq!(reopened.entries[0].offset, 0);
        assert_eq!(reopened.entries[0].sector, 0);
    }

    #[test]
    fn empty_archive_round_trips_as_a_header_only_img() {
        let dir = tempfile::tempdir().unwrap();
        let saved_path = dir.path().join("empty.img");
        let mut archive = ArchiveInfo::new("empty", false, crate::parser::ImgVersion::Two);

        PcV2Parser.save(&mut archive, &saved_path, false).unwrap();

        assert_eq!(std::fs::read(&saved_path).unwrap(), b"VER2\0\0\0\0");
        let reopened = ArchiveInfo::open(&saved_path).unwrap();
        assert!(reopened.entries.is_empty());
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

    #[test]
    fn retail_san_andreas_archives_validate_when_present() {
        let Some(root) = crate::test_paths::corpus_root() else {
            return;
        };
        let archives = [
            root.join("GTA San Andreas/models/cutscene.img"),
            root.join("GTA San Andreas/models/gta_int.img"),
            root.join("GTA San Andreas/models/gta3.img"),
            root.join("GTA San Andreas/models/player.img"),
        ];

        let mut checked = 0usize;
        for path in archives {
            if !path.exists() {
                continue;
            }

            let archive = ArchiveInfo::open(&path).unwrap_or_else(|error| {
                panic!(
                    "{} should open as a valid IMG v2 archive: {error}",
                    path.display()
                )
            });
            assert_eq!(archive.version, crate::parser::ImgVersion::Two);

            let image_len = std::fs::metadata(&path).unwrap().len();
            let data_start = data_start_for_entry_count(archive.entries.len()).unwrap();
            for entry in &archive.entries {
                let size_words = entry.v2_size.expect("v2 entry keeps raw size words");
                assert_eq!(entry.sector, size_words.effective_sector_count());

                let start = u64::from(entry.offset) * SECTOR_SIZE;
                let end = start + u64::from(entry.sector) * SECTOR_SIZE;
                assert!(
                    end <= image_len,
                    "{} has an out-of-range entry",
                    path.display()
                );
                if entry.sector > 0 {
                    assert!(
                        start >= data_start,
                        "{} has entry data inside its directory",
                        path.display()
                    );
                }
            }
            checked += 1;
        }

        assert!(
            checked > 0,
            "corpus root is set but no San Andreas IMG v2 archives were found"
        );
    }
}
