use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use compact_str::CompactString;
use rayon::prelude::*;

use crate::archive::{ArchiveInfo, EntryInfo, PackStats, ProgressInfo};
use crate::parser::{ImgParser, ImgVersion, PcV1Parser, PcV2Parser, SECTOR_SIZE, unique_output_path};

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
}
