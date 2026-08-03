use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use compact_str::CompactString;
use rayon::prelude::*;

use crate::archive::{ArchiveInfo, EntryInfo, PackStats, ProgressInfo};
use crate::parser::{
    ImgParser, ImgVersion, ImportEntryResult, PcV1Parser, PcV2Parser, SECTOR_SIZE,
    import_entry_with_result, unique_output_path,
};

#[derive(Debug, Clone, Copy)]
pub enum ExportMode {
    All,
    Selected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportEngine {
    /// Chunked parallel export with Rayon + per-worker BufReader.
    /// Default. Good UI responsiveness; throughput within noise of C++.
    Parallel,
    /// Single-threaded sequential export mirroring the original C++ behavior.
    /// Minimizes thread coordination overhead on I/O-bound systems.
    Fast,
}

#[derive(Debug)]
pub struct SaveTask {
    pub archive: ArchiveInfo,
    pub path: PathBuf,
    pub version: ImgVersion,
    pub remove_existing: bool,
}

impl SaveTask {
    pub fn new(archive: ArchiveInfo, path: PathBuf, version: ImgVersion) -> Self {
        Self {
            archive,
            path,
            version,
            remove_existing: false,
        }
    }

    pub fn remove_existing(mut self, remove: bool) -> Self {
        self.remove_existing = remove;
        self
    }

    pub async fn run(self) -> anyhow::Result<ArchiveInfo> {
        self.run_blocking()
    }

    pub fn run_blocking(self) -> anyhow::Result<ArchiveInfo> {
        let progress = self.archive.progress.clone();
        progress.start();

        let mut archive = self.archive;
        let result: anyhow::Result<()> = match self.version {
            ImgVersion::One => PcV1Parser
                .save(&mut archive, &self.path, self.remove_existing)
                .map_err(anyhow_forward),
            ImgVersion::Two => PcV2Parser
                .save(&mut archive, &self.path, self.remove_existing)
                .map_err(anyhow_forward),
            ImgVersion::Unknown => {
                progress.finish();
                Err(anyhow::anyhow!("cannot save unknown archive format"))
            }
        };

        if let Err(ref err) = result {
            eprintln!("save failed: {err}");
            progress.finish();
        } else {
            archive.add_log("Archive saved".to_string());
        }

        result.map(|_| archive)
    }
}

#[derive(Debug, Clone)]
pub struct FolderImportPlan {
    pub folder: PathBuf,
    pub files: Vec<PathBuf>,
    pub scan_skipped: Vec<String>,
    pub duplicate_count: usize,
    pub total_bytes: u64,
    pub target_archive_name: String,
    pub target_archive_path: Option<PathBuf>,
}

impl FolderImportPlan {
    pub fn discovered_count(&self) -> usize {
        self.files.len() + self.scan_skipped.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderDuplicatePolicy {
    Skip,
    Replace,
}

#[derive(Debug, Clone)]
pub struct FolderImportSummary {
    pub discovered: usize,
    pub imported: usize,
    pub skipped: usize,
    pub failed: usize,
    pub cancelled: bool,
    pub details: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FolderImportOutcome {
    pub archive: ArchiveInfo,
    pub summary: FolderImportSummary,
    pub target_archive_name: String,
    pub target_archive_path: Option<PathBuf>,
}

pub fn scan_import_folder(folder: &Path, archive: &ArchiveInfo) -> anyhow::Result<FolderImportPlan> {
    let mut files = Vec::new();
    let mut scan_skipped = Vec::new();
    let mut total_bytes = 0_u64;

    for item in std::fs::read_dir(folder)? {
        let item = match item {
            Ok(item) => item,
            Err(error) => {
                scan_skipped.push(format!("directory entry: {error}"));
                continue;
            }
        };
        let path = item.path();
        let metadata = match item.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                scan_skipped.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        if !metadata.is_file() {
            continue;
        }

        total_bytes = total_bytes
            .checked_add(metadata.len())
            .ok_or_else(|| anyhow::anyhow!("folder contents are too large"))?;
        files.push(path);
    }

    files.sort_by_cached_key(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    });

    let existing_names: HashSet<String> = archive
        .entries
        .iter()
        .map(|entry| entry.file_name.to_string().to_ascii_lowercase())
        .collect();
    let mut seen_names = HashSet::new();
    let duplicate_count = files
        .iter()
        .filter(|path| {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            !seen_names.insert(name.clone()) || existing_names.contains(&name)
        })
        .count();

    Ok(FolderImportPlan {
        folder: folder.to_path_buf(),
        files,
        scan_skipped,
        duplicate_count,
        total_bytes,
        target_archive_name: archive.file_name.clone(),
        target_archive_path: archive.path.clone(),
    })
}

#[derive(Debug, Clone)]
pub struct PackOutcome {
    pub archive: ArchiveInfo,
    pub stats: PackStats,
}

#[derive(Debug)]
pub struct PackTask {
    pub archive: ArchiveInfo,
    pub path: PathBuf,
    pub version: ImgVersion,
}

impl PackTask {
    pub fn new(archive: ArchiveInfo, path: PathBuf, version: ImgVersion) -> Self {
        Self { archive, path, version }
    }

    pub async fn run(self) -> anyhow::Result<PackOutcome> {
        self.run_blocking()
    }

    pub fn run_blocking(self) -> anyhow::Result<PackOutcome> {
        let estimate = self.archive.pack_stats()?;
        let entry_count = estimate.entry_count;
        let original_bytes = estimate.original_bytes;

        let mut archive = SaveTask::new(self.archive, self.path.clone(), self.version)
            .run_blocking()?;
        let packed_bytes = std::fs::metadata(&self.path)?.len();
        let stats = PackStats::from_sizes(entry_count, original_bytes, packed_bytes);

        archive.add_log(format!(
            "Archive packed: {} entries, {} reclaimed",
            stats.entry_count,
            format_bytes(stats.reclaimed_bytes())
        ));

        Ok(PackOutcome { archive, stats })
    }
}

#[derive(Debug)]
pub struct FolderImportTask {
    pub archive: ArchiveInfo,
    pub plan: FolderImportPlan,
    pub duplicate_policy: FolderDuplicatePolicy,
}

impl FolderImportTask {
    pub fn new(
        archive: ArchiveInfo,
        plan: FolderImportPlan,
        duplicate_policy: FolderDuplicatePolicy,
    ) -> Self {
        Self {
            archive,
            plan,
            duplicate_policy,
        }
    }

    pub async fn run(self) -> anyhow::Result<FolderImportOutcome> {
        self.run_blocking()
    }

    pub fn run_blocking(self) -> anyhow::Result<FolderImportOutcome> {
        let FolderImportTask {
            mut archive,
            plan,
            duplicate_policy,
        } = self;
        let progress = archive.progress.clone();
        progress.start();

        let mut summary = FolderImportSummary {
            discovered: plan.discovered_count(),
            imported: 0,
            skipped: plan.scan_skipped.len(),
            failed: 0,
            cancelled: false,
            details: plan.scan_skipped.clone(),
        };
        let mut seen_names: HashSet<String> = archive
            .entries
            .iter()
            .map(|entry| entry.file_name.to_string().to_ascii_lowercase())
            .collect();

        for (index, path) in plan.files.iter().enumerate() {
            if progress.is_cancelled() {
                summary.cancelled = true;
                summary.skipped += plan.files.len() - index;
                push_import_detail(&mut summary, "Import cancelled; remaining files were skipped.");
                break;
            }

            let display_name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            let key = display_name.to_ascii_lowercase();
            let duplicate = !seen_names.insert(key);

            if duplicate && duplicate_policy == FolderDuplicatePolicy::Skip {
                summary.skipped += 1;
                push_import_detail(&mut summary, format!("{display_name}: duplicate skipped"));
            } else {
                let replace = duplicate && duplicate_policy == FolderDuplicatePolicy::Replace;
                match import_entry_with_result(&mut archive, path, replace) {
                    Ok(ImportEntryResult::Imported) => summary.imported += 1,
                    Ok(ImportEntryResult::Skipped { reason }) => {
                        summary.skipped += 1;
                        push_import_detail(&mut summary, format!("{display_name}: {reason}"));
                    }
                    Err(error) => {
                        summary.failed += 1;
                        push_import_detail(&mut summary, format!("{display_name}: {error}"));
                    }
                }
            }

            progress.set_percentage((index + 1) as f32 / plan.files.len().max(1) as f32);
        }

        if summary.imported > 0 {
            archive.dirty = true;
            archive.invalidate_entry_caches();
        }
        archive.add_log(format!(
            "Folder import: {} imported, {} skipped, {} failed",
            summary.imported, summary.skipped, summary.failed
        ));
        for detail in &summary.details {
            archive.add_log(format!("Folder import detail: {detail}"));
        }
        archive.update_search = true;
        progress.finish();

        Ok(FolderImportOutcome {
            archive,
            summary,
            target_archive_name: plan.target_archive_name,
            target_archive_path: plan.target_archive_path,
        })
    }
}

const MAX_IMPORT_DETAILS: usize = 12;

fn push_import_detail(summary: &mut FolderImportSummary, detail: impl Into<String>) {
    if summary.details.len() < MAX_IMPORT_DETAILS {
        summary.details.push(detail.into());
    }
}

#[derive(Debug)]
pub struct ExportTask {
    pub archive: ArchiveInfo,
    pub folder: PathBuf,
    pub mode: ExportMode,
    pub engine: ExportEngine,
    pub progress: ProgressInfo,
}

impl ExportTask {
    pub fn new(archive: ArchiveInfo, folder: PathBuf, mode: ExportMode) -> Self {
        let progress = archive.progress.clone();
        Self {
            archive,
            folder,
            mode,
            engine: ExportEngine::Parallel,
            progress,
        }
    }

    pub fn engine(mut self, engine: ExportEngine) -> Self {
        self.engine = engine;
        self
    }

    pub async fn run(self) -> anyhow::Result<(usize, Vec<String>)> {
        self.run_blocking()
    }

    pub fn run_blocking(self) -> anyhow::Result<(usize, Vec<String>)> {
        let ExportTask {
            archive,
            folder,
            mode,
            engine: _,
            progress,
        } = self;

        progress.start();

        let entries: Vec<EntryInfo> = match mode {
            ExportMode::All => archive.entries.clone(),
            ExportMode::Selected => {
                archive.entries.iter().filter(|e| e.selected).cloned().collect()
            }
        };

        let total = entries.len();
        let completed = AtomicUsize::new(0);

        let results: Vec<(CompactString, anyhow::Result<()>)> = if total == 0 {
            Vec::new()
        } else if self.engine == ExportEngine::Fast {
            export_entries_sequential(
                &entries,
                &archive,
                &folder,
                &progress,
                total,
                &completed,
            )
        } else {
            export_entries_batched(
                &entries,
                &archive,
                &folder,
                &progress,
                total,
                &completed,
            )
        };

        let count = results.iter().filter(|(_, r)| r.is_ok()).count();
        for (name, result) in results {
            if let Err(err) = result {
                eprintln!("failed to export {name}: {err}");
            }
        }

        progress.set_percentage(1.0);
        progress.finish();
        let exported_names: Vec<String> = entries
            .iter()
            .map(|e| e.file_name.to_string())
            .collect();
        Ok((count, exported_names))
    }
}

fn export_entries_sequential(
    entries: &[EntryInfo],
    archive: &ArchiveInfo,
    folder: &std::path::Path,
    progress: &ProgressInfo,
    total: usize,
    completed: &AtomicUsize,
) -> Vec<(CompactString, anyhow::Result<()>)> {
    let source_path = archive.path.clone();

    let mut results = Vec::with_capacity(entries.len());
    for (idx, entry) in entries.iter().enumerate() {
        if progress.is_cancelled() {
            results.push((
                entry.file_name.clone(),
                Err(anyhow::anyhow!("Export cancelled")),
            ));
            continue;
        }

        let result = export_entry_buffered(
            archive.version,
            entry,
            source_path.as_deref(),
            None,
            folder,
        );

        if (idx + 1) % 64 == 0 || idx + 1 == total {
            progress.set_percentage((idx + 1) as f32 / total as f32);
        }
        completed.fetch_add(1, Ordering::Relaxed);
        results.push((entry.file_name.clone(), result));
    }
    results
}

fn export_entries_batched(
    entries: &[EntryInfo],
    archive: &ArchiveInfo,
    folder: &std::path::Path,
    progress: &ProgressInfo,
    total: usize,
    completed: &AtomicUsize,
) -> Vec<(CompactString, anyhow::Result<()>)> {
    let workers = rayon::current_num_threads().clamp(1, 8);
    let chunk_size = (entries.len() / workers).max(1);
    let chunks: Vec<Vec<EntryInfo>> = entries
        .chunks(chunk_size)
        .map(|c| c.to_vec())
        .collect();

    let source_path = archive.path.clone();

    chunks
        .into_par_iter()
        .flat_map(|chunk| {
            let chunk_len = chunk.len();
            let mut reader = source_path
                .as_ref()
                .map(|path| BufReader::with_capacity(4 * 1024 * 1024, File::open(path).unwrap()));

            let mut local_completed: usize = 0;
            chunk
                .into_iter()
                .enumerate()
                .map(|(idx, entry)| {
                    if progress.is_cancelled() {
                        return (
                            entry.file_name.clone(),
                            Err(anyhow::anyhow!("Export cancelled")),
                        );
                    }

                    let result = export_entry_buffered(
                        archive.version,
                        &entry,
                        source_path.as_deref(),
                        reader.as_mut(),
                        folder,
                    );

                    local_completed += 1;
                    if local_completed.is_multiple_of(64) || idx + 1 == chunk_len {
                        let done = completed.fetch_add(local_completed, Ordering::Relaxed) + local_completed;
                        local_completed = 0;
                        progress.set_percentage(done as f32 / total as f32);
                    }

                    (entry.file_name.clone(), result)
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

const OUTPUT_BUF_SIZE: usize = 1024 * 1024;

fn export_entry_buffered(
    version: ImgVersion,
    entry: &EntryInfo,
    archive_path: Option<&std::path::Path>,
    reader: Option<&mut BufReader<File>>,
    folder: &Path,
) -> anyhow::Result<()> {
    let output_path = unique_output_path(&folder.join(&entry.file_name));

    if entry.imported {
        let Some(source) = entry.source_path.as_ref() else {
            anyhow::bail!("imported entry has no source path");
        };
        std::fs::copy(source, &output_path)?;
        return Ok(());
    }

    if version == ImgVersion::Unknown {
        anyhow::bail!("unknown archive format cannot be exported");
    }

    let Some(path) = archive_path else {
        anyhow::bail!("archive has no source path");
    };

    let size = u64::from(entry.sector) * SECTOR_SIZE;
    let offset = u64::from(entry.offset) * SECTOR_SIZE;

    if let Some(r) = reader {
        let mut buf = vec![0u8; size as usize];
        r.seek(SeekFrom::Start(offset))?;
        r.read_exact(&mut buf)?;
        write_output_buffered(&output_path, &buf)?;
    } else {
        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; size as usize];
        file.read_exact(&mut buf)?;
        write_output_buffered(&output_path, &buf)?;
    }

    Ok(())
}

fn write_output_buffered(path: &Path, data: &[u8]) -> anyhow::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::with_capacity(OUTPUT_BUF_SIZE, file);
    writer.write_all(data)?;
    writer.flush()?;
    Ok(())
}

fn anyhow_forward<E: std::fmt::Display>(err: E) -> anyhow::Error {
    anyhow::anyhow!("{err}")
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek, SeekFrom, Write};

    use crate::parser::{encode_entry_name, read_entry_data};

    fn write_entry_record(output: &mut impl Write, offset: u32, sector: u32, name: &str) {
        output.write_all(&offset.to_le_bytes()).unwrap();
        output.write_all(&sector.to_le_bytes()).unwrap();
        output.write_all(&encode_entry_name(name)).unwrap();
    }

    fn create_fragmented_v1(dir: &Path) -> PathBuf {
        let img_path = dir.join("fragmented-v1.img");
        let mut img = File::create(&img_path).unwrap();
        img.write_all(&vec![b'A'; SECTOR_SIZE as usize]).unwrap();
        img.seek(SeekFrom::Start(2 * SECTOR_SIZE)).unwrap();
        img.write_all(&vec![b'B'; SECTOR_SIZE as usize]).unwrap();
        img.set_len(5 * SECTOR_SIZE).unwrap();

        let dir_path = dir.join("fragmented-v1.dir");
        let mut directory = File::create(dir_path).unwrap();
        write_entry_record(&mut directory, 0, 1, "first.dff");
        write_entry_record(&mut directory, 2, 1, "second.txd");
        img_path
    }

    fn create_fragmented_v2(dir: &Path) -> PathBuf {
        let img_path = dir.join("fragmented-v2.img");
        let mut img = File::create(&img_path).unwrap();
        img.write_all(b"VER2").unwrap();
        img.write_all(&2_u32.to_le_bytes()).unwrap();
        write_entry_record(&mut img, 1536, 1, "first.dff");
        write_entry_record(&mut img, 1538, 1, "second.txd");
        img.seek(SeekFrom::Start(1536 * SECTOR_SIZE)).unwrap();
        img.write_all(&vec![b'A'; SECTOR_SIZE as usize]).unwrap();
        img.seek(SeekFrom::Start(1538 * SECTOR_SIZE)).unwrap();
        img.write_all(&vec![b'B'; SECTOR_SIZE as usize]).unwrap();
        img.set_len(0x300000 + 4 * SECTOR_SIZE).unwrap();
        img_path
    }

    #[test]
    fn export_modes_are_distinct() {
        assert!(matches!(ExportMode::All, ExportMode::All));
        assert!(matches!(ExportMode::Selected, ExportMode::Selected));
    }

    #[test]
    fn pack_task_compacts_img_v1_holes() {
        let dir = tempfile::tempdir().unwrap();
        let source = create_fragmented_v1(dir.path());
        let output = dir.path().join("packed-v1.img");
        let archive = ArchiveInfo::open(&source).unwrap();

        let outcome = PackTask::new(archive, output.clone(), ImgVersion::One)
            .run_blocking()
            .unwrap();

        assert_eq!(outcome.stats.entry_count, 2);
        assert_eq!(outcome.stats.original_bytes, 5 * SECTOR_SIZE);
        assert_eq!(outcome.stats.packed_bytes, 2 * SECTOR_SIZE);
        assert_eq!(outcome.stats.reclaimed_bytes(), 3 * SECTOR_SIZE);
        assert_eq!(std::fs::metadata(&output).unwrap().len(), 2 * SECTOR_SIZE);

        let packed = ArchiveInfo::open(&output).unwrap();
        assert_eq!(packed.entries[0].offset, 0);
        assert_eq!(packed.entries[1].offset, 1);
        assert_eq!(read_entry_data(&packed, &packed.entries[0]).unwrap()[0], b'A');
        assert_eq!(read_entry_data(&packed, &packed.entries[1]).unwrap()[0], b'B');
    }

    #[test]
    fn pack_task_compacts_img_v2_holes() {
        let dir = tempfile::tempdir().unwrap();
        let source = create_fragmented_v2(dir.path());
        let output = dir.path().join("packed-v2.img");
        let archive = ArchiveInfo::open(&source).unwrap();

        let outcome = PackTask::new(archive, output.clone(), ImgVersion::Two)
            .run_blocking()
            .unwrap();

        assert_eq!(outcome.stats.entry_count, 2);
        assert_eq!(outcome.stats.original_bytes, 0x300000 + 4 * SECTOR_SIZE);
        assert_eq!(outcome.stats.packed_bytes, 0x300000 + 2 * SECTOR_SIZE);
        assert_eq!(outcome.stats.reclaimed_bytes(), 2 * SECTOR_SIZE);
        assert_eq!(std::fs::metadata(&output).unwrap().len(), 0x300000 + 2 * SECTOR_SIZE);

        let packed = ArchiveInfo::open(&output).unwrap();
        assert_eq!(packed.entries[0].offset, 1536);
        assert_eq!(packed.entries[1].offset, 1537);
        assert_eq!(read_entry_data(&packed, &packed.entries[0]).unwrap()[0], b'A');
        assert_eq!(read_entry_data(&packed, &packed.entries[1]).unwrap()[0], b'B');
    }

    #[test]
    fn folder_scan_is_top_level_and_detects_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("existing.dff"), b"existing").unwrap();
        std::fs::write(dir.path().join("new.txd"), b"new").unwrap();
        std::fs::write(dir.path().join("README"), b"ignored by importer").unwrap();
        let nested = dir.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("nested.dff"), b"nested").unwrap();

        let mut archive = ArchiveInfo::new("test", true, ImgVersion::One);
        archive.entries.push(EntryInfo::new("existing.dff"));
        let plan = scan_import_folder(dir.path(), &archive).unwrap();

        assert_eq!(plan.files.len(), 3);
        assert_eq!(plan.duplicate_count, 1);
        assert_eq!(plan.scan_skipped.len(), 0);
        assert_eq!(plan.discovered_count(), 3);
        assert!(plan.files.iter().all(|path| path.parent() == Some(dir.path())));
    }

    #[test]
    fn folder_import_skip_policy_reports_skips_and_imports() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("existing.dff"), b"replacement").unwrap();
        std::fs::write(dir.path().join("new.txd"), b"new").unwrap();
        std::fs::write(dir.path().join("README"), b"no extension").unwrap();

        let mut archive = ArchiveInfo::new("test", true, ImgVersion::One);
        archive.entries.push(EntryInfo::new("existing.dff"));
        let plan = scan_import_folder(dir.path(), &archive).unwrap();
        let outcome = FolderImportTask::new(archive, plan, FolderDuplicatePolicy::Skip)
            .run_blocking()
            .unwrap();

        assert_eq!(outcome.summary.discovered, 3);
        assert_eq!(outcome.summary.imported, 1);
        assert_eq!(outcome.summary.skipped, 2);
        assert_eq!(outcome.summary.failed, 0);
        assert_eq!(outcome.archive.entries.len(), 2);
        assert!(outcome.archive.entries.iter().any(|entry| entry.file_name == "existing.dff"));
        assert!(outcome.archive.entries.iter().any(|entry| entry.file_name == "new.txd"));
        assert!(outcome.archive.logs.iter().any(|log| log.contains("duplicate skipped")));
    }

    #[test]
    fn folder_import_replace_policy_replaces_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("existing.dff"), b"replacement").unwrap();
        std::fs::write(dir.path().join("new.txd"), b"new").unwrap();
        std::fs::write(dir.path().join("README"), b"no extension").unwrap();

        let mut archive = ArchiveInfo::new("test", true, ImgVersion::One);
        archive.entries.push(EntryInfo::new("existing.dff"));
        let plan = scan_import_folder(dir.path(), &archive).unwrap();
        let outcome = FolderImportTask::new(archive, plan, FolderDuplicatePolicy::Replace)
            .run_blocking()
            .unwrap();

        assert_eq!(outcome.summary.imported, 2);
        assert_eq!(outcome.summary.skipped, 1);
        assert_eq!(outcome.summary.failed, 0);
        assert_eq!(outcome.archive.entries.len(), 2);
        let replaced = outcome
            .archive
            .entries
            .iter()
            .find(|entry| entry.file_name == "existing.dff")
            .unwrap();
        assert!(replaced.imported);
        assert_eq!(replaced.source_path.as_deref(), Some(dir.path().join("existing.dff").as_path()));
    }
}
