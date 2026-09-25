use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use memmap2::Mmap;
use std::sync::Arc;

use crate::archive::{ArchiveInfo, EntryInfo};
use crate::parser::{
    ImgParser, ImgVersion, MAX_ENTRY_NAME_BYTES, SECTOR_SIZE, canonical_img_path,
    decode_entry_name, export_entry_to_file, import_entry,
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
        let mut path = canonical_img_path(img_path);
        path.set_extension("dir");
        path
    }

    pub(crate) fn open_with_endian(
        &self,
        archive: &mut ArchiveInfo,
        byte_order: V1ByteOrder,
    ) -> Result<()> {
        let Some(source_path) = archive.path.as_ref() else {
            anyhow::bail!("new archives do not have a source path");
        };
        let path = canonical_img_path(source_path);
        archive.path = Some(path.clone());
        let dir_path = Self::dir_path(&path);
        let dir_bytes = std::fs::read(&dir_path)
            .with_context(|| format!("failed to read IMG v1 directory: {}", dir_path.display()))?;
        let img_len = std::fs::metadata(&path)
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

        archive.add_log(crate::i18n::t::log_archive_opened());
        Ok(())
    }

    pub(crate) fn is_valid_with_endian(&self, path: &Path, byte_order: V1ByteOrder) -> bool {
        let path = canonical_img_path(path);
        let Ok(img_len) = std::fs::metadata(&path).map(|metadata| metadata.len()) else {
            return false;
        };
        let dir_path = Self::dir_path(&path);
        let Ok(dir_bytes) = std::fs::read(dir_path) else {
            return false;
        };

        validate_v1_directory(&dir_bytes, img_len, byte_order).is_ok()
    }

    /// Whether a sibling `.dir` directory file exists. IMG v1
    /// (little-endian) and Xbox 360 (big-endian) share this container
    /// layout, so the metadata probe is the cheap format check used by the
    /// open dispatch; full validation happens during the single parse in
    /// [`Self::open_with_endian`].
    pub(crate) fn has_directory_file(path: &Path) -> bool {
        std::fs::metadata(Self::dir_path(path))
            .map(|metadata| metadata.is_file())
            .unwrap_or(false)
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
        let output_path = canonical_img_path(output_path);
        let source_path = archive.path.clone();
        let dir_path = Self::dir_path(&output_path);

        let mut temp_img = output_path.as_os_str().to_owned();
        temp_img.push(".temp");
        let temp_img = PathBuf::from(temp_img);

        let mut temp_dir = dir_path.as_os_str().to_owned();
        temp_dir.push(".temp");
        let temp_dir = PathBuf::from(temp_dir);

        let result = self.save_internal(
            archive,
            &output_path,
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

        // `rename` replaces the destination in one step, so the old
        // archive stays intact until its replacement is in place.
        std::fs::rename(&temp_img, &output_path).context("failed to write archive file")?;
        std::fs::rename(&temp_dir, &dir_path).context("failed to write directory file")?;

        if remove_existing
            && let Some(ref src) = source_path
            && src != &output_path
        {
            let _ = std::fs::remove_file(src);
        }

        archive.path = Some(output_path.clone());
        archive.file_name = output_path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_string());
        archive.version = version;
        let img_file =
            std::fs::File::open(&output_path).context("failed to reopen packed IMG v1 archive")?;
        archive.source_mmap = Some(Arc::new(unsafe { Mmap::map(&img_file)? }));
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

            let sector_offset = u32::try_from(offset / SECTOR_SIZE).map_err(|_| {
                anyhow::anyhow!("IMG v1 output offset is too large for {}", entry.file_name)
            })?;
            let sector_count = u32::try_from(size / SECTOR_SIZE).map_err(|_| {
                anyhow::anyhow!("IMG v1 entry is too large for {}", entry.file_name)
            })?;
            dir_out.write_all(&byte_order.write_u32(sector_offset))?;
            dir_out.write_all(&byte_order.write_u32(sector_count))?;
            dir_out.write_all(&entry.file_name_raw)?;

            crate::parser::stream_entry_data(
                &mut img_out,
                entry,
                size,
                source_path.as_deref(),
                source_mmap.as_deref(),
                &mut source_file,
            )?;

            offset = offset
                .checked_add(size)
                .ok_or_else(|| anyhow::anyhow!("IMG v1 output is too large"))?;
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
            entry.offset = u32::try_from(offset / SECTOR_SIZE).map_err(|_| {
                anyhow::anyhow!("IMG v1 output offset is too large for {}", entry.file_name)
            })?;
            entry.sector = u32::try_from(size / SECTOR_SIZE)
                .map_err(|_| anyhow::anyhow!("IMG v1 entry is too large for {}", entry.file_name))?;
            offset = offset
                .checked_add(size)
                .ok_or_else(|| anyhow::anyhow!("IMG v1 output is too large"))?;
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
    if !dir_bytes.len().is_multiple_of(crate::parser::ENTRY_SIZE) {
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
    use std::collections::BTreeMap;
    use std::io::Write;

    use super::*;
    use crate::archive::{ArchiveInfo, EntryInfo};
    use crate::parser::{
        ImgVersion, SECTOR_SIZE, detect_version, encode_entry_name, read_entry_data,
        sector_rounded_size,
    };

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

        let dir_path = PcV1Parser::dir_path(&img_path);
        assert!(PcV1Parser.is_valid(&dir_path));
        let opened_from_dir = ArchiveInfo::open(&dir_path).unwrap();
        assert_eq!(opened_from_dir.path.as_deref(), Some(img_path.as_path()));
        assert_eq!(opened_from_dir.entries.len(), archive.entries.len());
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
    fn save_in_place_while_another_copy_maps_the_source() {
        // The UI keeps its own clone (and memory map) of the archive while
        // the save task rewrites the same path.
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_v1_archive(dir.path(), "test", &[("entry.dff", b"old")]);
        let ui_copy = ArchiveInfo::open(&img_path).unwrap();
        let mut archive = ui_copy.clone();
        archive.entries[0].override_bytes = Some(Arc::new(b"new".to_vec()));

        PcV1Parser.save(&mut archive, &img_path, false).unwrap();

        let reopened = ArchiveInfo::open(&img_path).unwrap();
        assert!(read_entry_data(&reopened, &reopened.entries[0])
            .unwrap()
            .starts_with(b"new"));
        drop(ui_copy);
    }

    #[test]
    fn save_to_a_new_path_keeps_the_source_pair() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = create_v1_archive(dir.path(), "source", &[("entry.dff", b"data")]);
        let mut archive = ArchiveInfo::open(&img_path).unwrap();

        PcV1Parser
            .save(&mut archive, &dir.path().join("copy.img"), false)
            .unwrap();

        assert!(img_path.exists());
        assert!(dir.path().join("source.dir").exists());
        assert_eq!(ArchiveInfo::open(&img_path).unwrap().entries.len(), 1);
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

    #[test]
    fn retail_gta_iii_and_vc_v1_archives_validate_and_round_trip_when_present() {
        let Some(root) = crate::test_paths::corpus_root() else {
            return;
        };

        let archives = [
            (
                "gta3-models",
                root.join("Gta_3_img/models/gta3.img"),
                root.join("Gta_3_img/models/gta3.dir"),
            ),
            (
                "gta3-txd",
                root.join("Gta_3_img/models/txd.img"),
                root.join("Gta_3_img/models/txd.dir"),
            ),
            (
                "vc-models",
                root.join("Grand Theft Auto Vice City/models/gta3.img"),
                root.join("Grand Theft Auto Vice City/models/gta3.dir"),
            ),
            (
                "vc-cuts",
                root.join("Grand Theft Auto Vice City/anim/cuts.img"),
                root.join("Grand Theft Auto Vice City/anim/cuts.dir"),
            ),
        ];
        let wanted_extensions = ["DFF", "TXD", "COL", "IFP"];
        let mut checked_archives = 0;

        for (label, img_path, dir_path) in archives {
            if !img_path.is_file() || !dir_path.is_file() {
                continue;
            }

            checked_archives += 1;
            assert_eq!(
                detect_version(&dir_path),
                ImgVersion::One,
                "{label} should detect as IMG v1 from its .dir path"
            );
            assert!(
                PcV1Parser.is_valid(&dir_path),
                "{label} should validate from its .dir path"
            );

            let archive = ArchiveInfo::open(&dir_path)
                .unwrap_or_else(|error| panic!("{label} should open from .dir: {error}"));
            assert_eq!(archive.path.as_deref(), Some(img_path.as_path()));
            assert!(!archive.entries.is_empty(), "{label} should contain entries");

            let mut first_by_extension = BTreeMap::new();
            for (index, entry) in archive.entries.iter().enumerate() {
                first_by_extension
                    .entry(entry.file_ext.to_string())
                    .or_insert(index);
            }

            let roundtrip_dir = tempfile::tempdir().unwrap();
            let mut sample = ArchiveInfo::new(
                format!("{label}-sample.img"),
                false,
                ImgVersion::One,
            );
            let mut expected = Vec::new();

            for extension in wanted_extensions {
                let Some(&index) = first_by_extension.get(extension) else {
                    continue;
                };
                let entry = &archive.entries[index];
                let bytes = read_entry_data(&archive, entry).unwrap_or_else(|error| {
                    panic!("{label}: failed to read {}: {error}", entry.file_name)
                });
                assert!(!bytes.is_empty());
                assert_eq!(
                    bytes.len() as u64,
                    u64::from(entry.sector) * SECTOR_SIZE,
                    "{label}: {} should occupy its complete sector range",
                    entry.file_name
                );

                match extension {
                    "DFF" => {
                        let meshes = crate::parser::dff::parse_dff(&bytes).unwrap_or_else(|error| {
                            panic!("{label}: {} should parse as DFF: {error}", entry.file_name)
                        });
                        assert!(
                            meshes
                                .iter()
                                .any(|mesh| !mesh.positions.is_empty() && !mesh.indices.is_empty()),
                            "{label}: {} should contain DFF triangles",
                            entry.file_name
                        );
                    }
                    "TXD" => {
                        let txd = crate::parser::txd::parse_txd(&bytes).unwrap_or_else(|error| {
                            panic!("{label}: {} should parse as TXD: {error}", entry.file_name)
                        });
                        assert!(
                            !txd.textures.is_empty(),
                            "{label}: {} should contain TXD textures",
                            entry.file_name
                        );
                    }
                    "COL" => {
                        let col = crate::parser::col::parse_col(&bytes).unwrap_or_else(|error| {
                            panic!("{label}: {} should parse as COL: {error}", entry.file_name)
                        });
                        assert!(
                            !col.entries.is_empty(),
                            "{label}: {} should contain collision entries",
                            entry.file_name
                        );
                    }
                    "IFP" => {}
                    _ => unreachable!(),
                }

                let source_path = roundtrip_dir
                    .path()
                    .join(format!("sample-{index}.{extension}"));
                std::fs::write(&source_path, &bytes).unwrap();

                let mut imported = EntryInfo::new_for_version(
                    entry.file_name.clone(),
                    ImgVersion::One,
                );
                imported.file_name_raw = entry.file_name_raw;
                imported.source_path = Some(source_path);
                imported.imported = true;
                sample.entries.push(imported);
                expected.push((
                    entry.file_name.to_string(),
                    entry.file_name_raw,
                    bytes,
                ));
            }

            assert!(
                !expected.is_empty(),
                "{label} should contain at least one representative asset"
            );
            let output_path = roundtrip_dir.path().join(format!("{label}-roundtrip.img"));
            PcV1Parser
                .save(&mut sample, &output_path, false)
                .unwrap_or_else(|error| panic!("{label} round-trip save failed: {error}"));
            let reopened = ArchiveInfo::open(&output_path)
                .unwrap_or_else(|error| panic!("{label} round-trip reopen failed: {error}"));

            assert_eq!(reopened.entries.len(), expected.len());
            for ((expected_name, expected_raw, expected_bytes), actual) in
                expected.iter().zip(reopened.entries.iter())
            {
                assert_eq!(actual.file_name.as_str(), expected_name);
                assert_eq!(actual.file_name_raw, *expected_raw);
                let actual_bytes = read_entry_data(&reopened, actual).unwrap();
                assert_eq!(
                    actual_bytes.as_slice(),
                    expected_bytes.as_slice(),
                    "{label}: round-tripped bytes changed for {expected_name}"
                );
            }
        }

        assert!(
            checked_archives > 0,
            "IMGEDITOR_CORPUS_ROOT is set but no GTA III/Vice City IMG v1 pairs were found"
        );
    }
}
