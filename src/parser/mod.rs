use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use compact_str::CompactString;
use memmap2::Mmap;

use crate::archive::{ArchiveInfo, EntryInfo};

pub mod col;
pub mod db;
pub mod dff;
pub mod ifp;
pub mod inspector;
pub mod iparser;
pub mod pc_v1;
pub mod pc_v2;
pub mod texture_decoder;
pub mod txd;
pub mod txd_writer;
pub mod unknown;
pub mod xbox360;

pub use inspector::{EntryInspection, inspect_entry_cached, inspect_entry_standalone};
pub use iparser::ImgParser;
pub(crate) use pc_v1::V1ByteOrder;
pub use pc_v1::PcV1Parser;
pub use pc_v2::PcV2Parser;
pub use texture_decoder::DecodedTexture;
pub use unknown::UnknownParser;
pub use xbox360::Xbox360Parser;

pub const SECTOR_SIZE: u64 = 2048;
pub const ENTRY_SIZE: usize = 32;
pub const MAX_ENTRY_NAME_BYTES: usize = 24;
pub const MAX_ENTRY_NAME_LEN: usize = MAX_ENTRY_NAME_BYTES - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImgVersion {
    One,
    Two,
    Xbox360,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportEntryResult {
    Imported,
    Skipped { reason: String },
}

/// Return the data-file path used by a paired IMG v1 archive.
///
/// IMG v1 archives are represented by a `.img` data file and a sibling
/// `.dir` directory file.  Tools commonly let users select either half of
/// the pair, so all callers should normalize a `.dir` selection before
/// format detection, path identity checks, or parser dispatch.
pub fn canonical_img_path(path: &Path) -> PathBuf {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("dir"))
    {
        let mut canonical = path.to_path_buf();
        canonical.set_extension("img");
        canonical
    } else {
        path.to_path_buf()
    }
}

/// Whether a path is one of the paired IMG v1 archive files accepted by the
/// open/drop surfaces.
pub fn is_img_archive_path(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("img") || extension.eq_ignore_ascii_case("dir")
    })
}

pub fn detect_version(path: &Path) -> ImgVersion {
    let path = canonical_img_path(path);
    if PcV2Parser.is_valid(&path) {
        ImgVersion::Two
    } else if PcV1Parser.is_valid(&path) {
        ImgVersion::One
    } else if Xbox360Parser.is_valid(&path) {
        ImgVersion::Xbox360
    } else {
        ImgVersion::Unknown
    }
}

/// Open `archive` by dispatching to the first parser that recognizes the
/// file, parsing the directory exactly once. Detection order matches
/// [`detect_version`]: IMG v2 (VER2 magic), IMG v1 (little-endian directory
/// pair), Xbox 360 (big-endian directory pair), then [`UnknownParser`],
/// which leaves the archive with zero entries and [`ImgVersion::Unknown`].
///
/// A file that is *recognized* but structurally invalid (e.g. a VER2 header
/// with a truncated directory) surfaces the parser's specific error instead
/// of silently degrading to an unknown-format archive.
pub(crate) fn open_in_detect_order(archive: &mut ArchiveInfo) -> anyhow::Result<()> {
    let Some(path) = archive.path.clone() else {
        anyhow::bail!("new archives do not have a source path");
    };

    if PcV2Parser::recognizes(&path) {
        PcV2Parser.open(archive)?;
        archive.version = ImgVersion::Two;
        return Ok(());
    }

    if PcV1Parser::has_directory_file(&path) {
        if PcV1Parser.open(archive).is_ok() {
            archive.version = ImgVersion::One;
            return Ok(());
        }
        if Xbox360Parser.open(archive).is_ok() {
            archive.version = ImgVersion::Xbox360;
            return Ok(());
        }
    }

    UnknownParser.open(archive)
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

pub fn decode_entry_name(raw: &[u8; MAX_ENTRY_NAME_BYTES]) -> CompactString {
    let trimmed = raw.split(|&b| b == 0).next().unwrap_or(&[]);
    CompactString::from_utf8_lossy(trimmed)
}

pub fn encode_entry_name(name: &str) -> [u8; MAX_ENTRY_NAME_BYTES] {
    encode_entry_name_with_limit(name, MAX_ENTRY_NAME_LEN)
}

pub fn encode_entry_name_with_limit(
    name: &str,
    max_bytes: usize,
) -> [u8; MAX_ENTRY_NAME_BYTES] {
    let mut raw = [0u8; MAX_ENTRY_NAME_BYTES];
    let mut len = 0;
    let max_bytes = max_bytes.min(MAX_ENTRY_NAME_BYTES);

    for c in name.chars() {
        let mut buf = [0u8; 4];
        let encoded = c.encode_utf8(&mut buf);
        if len + encoded.len() > max_bytes {
            break;
        }
        raw[len..len + encoded.len()].copy_from_slice(encoded.as_bytes());
        len += encoded.len();
    }

    raw
}

pub fn entry_name_capacity(version: ImgVersion) -> usize {
    match version {
        ImgVersion::Xbox360 => MAX_ENTRY_NAME_BYTES,
        _ => MAX_ENTRY_NAME_LEN,
    }
}

pub fn unique_output_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }

    let stem = path.file_stem().unwrap_or_default();
    let ext = path.extension().unwrap_or_default();
    let mut index = 2;

    loop {
        let mut name = format!("{} ({})", stem.to_string_lossy(), index);
        if !ext.is_empty() {
            name.push('.');
            name.push_str(&ext.to_string_lossy());
        }

        let candidate = path.with_file_name(&name);
        if !candidate.exists() {
            return candidate;
        }

        index += 1;
    }
}

/// Validate an archive entry name before using it as an extracted filename.
///
/// Archive directory records are fixed-width strings, not paths.  Keeping the
/// validation at the extraction boundary lets us continue opening old files
/// while preventing traversal, alternate data streams, and Windows device
/// names from escaping the selected output directory.
pub(crate) fn validate_entry_output_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        anyhow::bail!("entry name is empty");
    }
    if name.as_bytes().contains(&0) {
        anyhow::bail!("entry name contains a NUL byte");
    }
    if name.contains(['/', '\\', ':']) {
        anyhow::bail!("entry name is not a single safe filename: {name}");
    }
    if name.ends_with(['.', ' ']) {
        anyhow::bail!("entry name has a trailing dot or space: {name}");
    }

    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        anyhow::bail!("entry name is not a single safe filename: {name}");
    }

    let device_name = name
        .split('.')
        .next()
        .unwrap_or(name)
        .to_ascii_uppercase();
    if matches!(
        device_name.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        anyhow::bail!("entry name is a reserved Windows device name: {name}");
    }

    Ok(())
}

/// Resolve an archive entry to a path beneath `folder` after validating its
/// fixed-width directory name as a filename rather than a user-controlled
/// path.
pub(crate) fn safe_entry_output_path(folder: &Path, name: &str) -> anyhow::Result<PathBuf> {
    validate_entry_output_name(name)?;
    Ok(folder.join(name))
}

pub fn read_entry_data(archive: &ArchiveInfo, entry: &EntryInfo) -> anyhow::Result<Vec<u8>> {
    read_entry_data_with_source(
        entry,
        archive.path.as_deref(),
        archive.source_mmap.as_deref(),
    )
}

pub fn read_entry_header(
    archive: &ArchiveInfo,
    entry: &EntryInfo,
    max_bytes: usize,
) -> anyhow::Result<Vec<u8>> {
    read_entry_header_standalone(
        entry,
        archive.path.as_deref(),
        archive.source_mmap.as_deref(),
        max_bytes,
    )
}

pub fn read_entry_header_standalone(
    entry: &EntryInfo,
    archive_path: Option<&Path>,
    source_mmap: Option<&Mmap>,
    max_bytes: usize,
) -> anyhow::Result<Vec<u8>> {
    if entry.imported {
        let source = entry
            .source_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("imported entry has no source path"))?;
        let mut file = std::fs::File::open(source)?;
        let mut data = vec![0u8; max_bytes];
        let read = file.read(&mut data)?;
        data.truncate(read);
        return Ok(data);
    }

    let source = archive_path.ok_or_else(|| anyhow::anyhow!("archive has no source path"))?;
    let offset = u64::from(entry.offset) * SECTOR_SIZE;

    if let Some(mmap) = source_mmap {
        let mmap_len = mmap.len() as u64;
        let start = offset.min(mmap_len) as usize;
        let end = (offset + max_bytes as u64).min(mmap_len) as usize;
        return Ok(mmap[start..end].to_vec());
    }

    let mut file = std::fs::File::open(source)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut data = vec![0u8; max_bytes];
    let read = file.read(&mut data)?;
    data.truncate(read);
    Ok(data)
}

/// Read one entry's bytes from the archive source, preferring the mmap and
/// honoring imported/override entries. Shared by the save, export, and
/// conversion tasks that hold only a path + mmap snapshot.
pub(crate) fn read_entry_data_with_source(
    entry: &EntryInfo,
    archive_source: Option<&Path>,
    source_mmap: Option<&Mmap>,
) -> anyhow::Result<Vec<u8>> {
    if let Some(bytes) = &entry.override_bytes {
        return Ok(bytes.as_ref().clone());
    }
    if entry.imported {
        let source = entry
            .source_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("imported entry has no source path"))?;
        return read_imported_file(source);
    }

    let source = archive_source.ok_or_else(|| anyhow::anyhow!("archive has no source path"))?;
    let size = u64::from(entry.sector) * SECTOR_SIZE;
    let offset = u64::from(entry.offset) * SECTOR_SIZE;

    if let Some(mmap) = source_mmap {
        let range = checked_source_entry_range(entry, mmap.len())?;
        return Ok(mmap[range].to_vec());
    }

    let mut file = std::fs::File::open(source)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut data = vec![0u8; size as usize];
    file.read_exact(&mut data)?;
    Ok(data)
}

const ZERO_SECTOR: [u8; SECTOR_SIZE as usize] = [0; SECTOR_SIZE as usize];

/// Size of the entry data as written during save, matching
/// `read_entry_data_with_source` without copying anything.
pub(crate) fn entry_data_size(
    entry: &EntryInfo,
    source_mmap: Option<&Mmap>,
) -> anyhow::Result<u64> {
    if let Some(bytes) = &entry.override_bytes {
        return Ok(sector_rounded_size(bytes.len() as u64));
    }
    if entry.imported {
        let source = entry
            .source_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("imported entry has no source path"))?;
        let actual = std::fs::metadata(source)?.len();
        return Ok(sector_rounded_size(actual));
    }
    let size = u64::from(entry.sector) * SECTOR_SIZE;
    if let Some(mmap) = source_mmap {
        let range = checked_source_entry_range(entry, mmap.len())?;
        return u64::try_from(range.len())
            .map_err(|_| anyhow::anyhow!("entry data size does not fit in u64"));
    }
    Ok(size)
}

/// Streams one entry's data to `out`, reading straight from the source memory
/// map when available instead of materializing a per-entry `Vec`. Writes
/// exactly `entry_data_size(entry, source_mmap)` bytes.
pub(crate) fn stream_entry_data(
    out: &mut impl Write,
    entry: &EntryInfo,
    source_path: Option<&Path>,
    source_mmap: Option<&Mmap>,
    source_file: &mut Option<BufReader<std::fs::File>>,
) -> anyhow::Result<()> {
    if let Some(bytes) = &entry.override_bytes {
        out.write_all(bytes)?;
        let actual = bytes.len() as u64;
        let pad = sector_rounded_size(actual) - actual;
        if pad > 0 {
            out.write_all(&ZERO_SECTOR[..pad as usize])?;
        }
        return Ok(());
    }
    if entry.imported {
        let source = entry
            .source_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("imported entry has no source path"))?;
        let actual = std::fs::metadata(source)?.len();
        let mut file = std::fs::File::open(source)?;
        std::io::copy(&mut file, out)?;
        let pad = sector_rounded_size(actual) - actual;
        if pad > 0 {
            out.write_all(&ZERO_SECTOR[..pad as usize])?;
        }
        return Ok(());
    }

    let size = u64::from(entry.sector) * SECTOR_SIZE;
    let offset = u64::from(entry.offset) * SECTOR_SIZE;

    if let Some(mmap) = source_mmap {
        let range = checked_source_entry_range(entry, mmap.len())?;
        out.write_all(&mmap[range])?;
        return Ok(());
    }

    let source = source_path.ok_or_else(|| anyhow::anyhow!("archive has no source path"))?;
    if source_file.is_none() {
        *source_file = Some(BufReader::with_capacity(
            4 * 1024 * 1024,
            std::fs::File::open(source)?,
        ));
    }
    let reader = source_file.as_mut().expect("source file opened above");
    reader.seek(SeekFrom::Start(offset))?;
    let written = std::io::copy(&mut reader.take(size), out)?;
    if written != size {
        anyhow::bail!("entry data truncated during save");
    }
    Ok(())
}

fn checked_source_entry_range(entry: &EntryInfo, source_len: usize) -> anyhow::Result<std::ops::Range<usize>> {
    let start = u64::from(entry.offset)
        .checked_mul(SECTOR_SIZE)
        .ok_or_else(|| anyhow::anyhow!("entry {} byte offset overflows", entry.file_name))?;
    let size = u64::from(entry.sector)
        .checked_mul(SECTOR_SIZE)
        .ok_or_else(|| anyhow::anyhow!("entry {} byte size overflows", entry.file_name))?;
    let end = start
        .checked_add(size)
        .ok_or_else(|| anyhow::anyhow!("entry {} byte range overflows", entry.file_name))?;
    let source_len = u64::try_from(source_len)
        .map_err(|_| anyhow::anyhow!("source size does not fit in u64"))?;
    if start > source_len || end > source_len {
        anyhow::bail!(
            "entry {} range [{start}, {end}) exceeds source size {source_len}",
            entry.file_name
        );
    }
    let start = usize::try_from(start)
        .map_err(|_| anyhow::anyhow!("entry {} start does not fit in usize", entry.file_name))?;
    let end = usize::try_from(end)
        .map_err(|_| anyhow::anyhow!("entry {} end does not fit in usize", entry.file_name))?;
    Ok(start..end)
}

fn read_imported_file(source: &std::path::Path) -> anyhow::Result<Vec<u8>> {
    let actual_len = std::fs::metadata(source)?.len();
    let rounded_len = sector_rounded_size(actual_len);
    let mut data = vec![0u8; rounded_len as usize];
    let mut file = std::fs::File::open(source)?;
    file.read_exact(&mut data[..actual_len as usize])?;
    Ok(data)
}

pub fn read_entry_data_from_source(
    entry: &EntryInfo,
    archive_source: Option<&std::path::Path>,
) -> anyhow::Result<Vec<u8>> {
    if let Some(bytes) = &entry.override_bytes {
        return Ok(bytes.as_ref().clone());
    }
    if entry.imported {
        let source = entry
            .source_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("imported entry has no source path"))?;
        return read_imported_file(source);
    }

    let source = archive_source.ok_or_else(|| anyhow::anyhow!("archive has no source path"))?;
    let size = u64::from(entry.sector) * SECTOR_SIZE;
    let offset = u64::from(entry.offset) * SECTOR_SIZE;
    let mut file = std::fs::File::open(source)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut data = vec![0u8; size as usize];
    file.read_exact(&mut data)?;
    Ok(data)
}

pub fn export_entry_to_file(
    archive: &ArchiveInfo,
    entry: &EntryInfo,
    output_path: &Path,
) -> anyhow::Result<()> {
    validate_entry_output_name(entry.file_name.as_str())?;
    let output_path = unique_output_path(output_path);

    if entry.imported {
        let source = entry
            .source_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("imported entry has no source path"))?;
        std::fs::copy(source, &output_path)?;
        return Ok(());
    }

    let data = read_entry_data(archive, entry)?;
    std::fs::write(&output_path, data)?;
    Ok(())
}

pub fn import_entry(archive: &mut ArchiveInfo, path: &Path, replace: bool) -> anyhow::Result<()> {
    import_entry_with_result(archive, path, replace).map(|_| ())
}

pub fn import_entry_with_result(
    archive: &mut ArchiveInfo,
    path: &Path,
    replace: bool,
) -> anyhow::Result<ImportEntryResult> {
    if path.extension().is_none() {
        return Ok(ImportEntryResult::Skipped {
            reason: "file has no extension".to_string(),
        });
    }

    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() {
        let reason = "Not a regular file".to_string();
        archive.add_log(format!("Skipping {}. {reason}.", path.display()));
        return Ok(ImportEntryResult::Skipped { reason });
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("import path is not valid UTF-8"))?;

    let name_capacity = entry_name_capacity(archive.version);
    if file_name.len() > name_capacity {
        let reason = format!("name exceeds {name_capacity} bytes");
        archive.add_log(format!("Skipping {file_name}. {reason}."));
        return Ok(ImportEntryResult::Skipped { reason });
    }

    if replace {
        archive
            .entries
            .retain(|entry| !entry.file_name.eq_ignore_ascii_case(file_name));
    }

    let byte_len = metadata.len();
    let mut entry = EntryInfo::new_for_version(file_name, archive.version);
    entry.source_path = Some(path.to_path_buf());
    entry.imported = true;
    entry.sector = (sector_rounded_size(byte_len) / SECTOR_SIZE) as u32;
    archive.entries.push(entry);
    Ok(ImportEntryResult::Imported)
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
    fn unique_output_path_avoids_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("file.txt");
        std::fs::write(&base, "x").unwrap();

        assert_eq!(unique_output_path(&base), dir.path().join("file (2).txt"));
    }

    #[test]
    fn extraction_names_are_single_safe_filenames() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            safe_entry_output_path(dir.path(), "model.dff").unwrap(),
            dir.path().join("model.dff")
        );

        for name in [
            "../escape.dff",
            "nested/model.dff",
            r"nested\model.dff",
            r"C:\escape.dff",
            "file:",
            "CON.txt",
            "trailing. ",
        ] {
            assert!(
                safe_entry_output_path(dir.path(), name).is_err(),
                "unsafe entry name was accepted: {name}"
            );
        }
    }

    #[test]
    fn export_rejects_unsafe_archive_names_before_reading_data() {
        let dir = tempfile::tempdir().unwrap();
        let archive = ArchiveInfo::new("test", false, ImgVersion::One);
        let entry = EntryInfo::new("../escape.dff");
        let error = export_entry_to_file(&archive, &entry, &dir.path().join("output"))
            .unwrap_err();
        assert!(error.to_string().contains("safe filename"));
    }

    #[test]
    fn mapped_entry_ranges_are_not_silently_clamped() {
        let mut entry = EntryInfo::new("truncated.dff");
        entry.offset = 1;
        entry.sector = 1;
        let error = checked_source_entry_range(&entry, SECTOR_SIZE as usize).unwrap_err();
        assert!(error.to_string().contains("exceeds source size"));
    }

    #[test]
    fn import_entry_skips_directories_with_extensions() {
        let dir = tempfile::tempdir().unwrap();
        let import_dir = dir.path().join("textures.txd");
        std::fs::create_dir(&import_dir).unwrap();
        let mut archive = ArchiveInfo::new("test", true, ImgVersion::One);

        import_entry(&mut archive, &import_dir, false).unwrap();

        assert!(archive.entries.is_empty());
        assert!(
            archive
                .logs
                .iter()
                .any(|log| log.contains("Not a regular file"))
        );
    }

    #[test]
    fn detect_img_v1_format() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = dir.path().join("test.img");
        let dir_path = dir.path().join("test.dir");
        std::fs::write(&img_path, vec![0_u8; SECTOR_SIZE as usize]).unwrap();

        let mut record = Vec::new();
        record.extend_from_slice(&0_u32.to_le_bytes());
        record.extend_from_slice(&1_u32.to_le_bytes());
        record.extend_from_slice(&encode_entry_name("entry.dff"));
        std::fs::write(&dir_path, record).unwrap();

        assert_eq!(detect_version(&img_path), ImgVersion::One);
        assert_eq!(detect_version(&dir_path), ImgVersion::One);
        assert_eq!(canonical_img_path(&dir_path), img_path);
        assert!(is_img_archive_path(&dir_path));
        assert!(is_img_archive_path(&img_path));
    }

    #[test]
    fn canonical_img_path_is_case_insensitive_and_preserves_other_paths() {
        let dir = tempfile::tempdir().unwrap();
        let upper_dir = dir.path().join("archive.DIR");
        let other = dir.path().join("archive.bin");

        assert_eq!(
            canonical_img_path(&upper_dir),
            dir.path().join("archive.img")
        );
        assert_eq!(canonical_img_path(&other), other);
        assert!(!is_img_archive_path(&other));
    }

    #[test]
    fn detect_rejects_out_of_range_v1_pair() {
        let dir = tempfile::tempdir().unwrap();
        let img_path = dir.path().join("invalid.img");
        let dir_path = dir.path().join("invalid.dir");
        std::fs::write(&img_path, vec![0_u8; SECTOR_SIZE as usize]).unwrap();

        let mut record = Vec::new();
        record.extend_from_slice(&1_u32.to_le_bytes());
        record.extend_from_slice(&1_u32.to_le_bytes());
        record.extend_from_slice(&encode_entry_name("outside.dff"));
        std::fs::write(&dir_path, record).unwrap();

        assert_eq!(detect_version(&img_path), ImgVersion::Unknown);
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
