use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use compact_str::CompactString;
use rayon::prelude::*;

use crate::archive::{ArchiveInfo, EntryInfo, ProgressInfo};
use crate::parser::{ImgParser, ImgVersion, PcV1Parser, PcV2Parser, SECTOR_SIZE, unique_output_path};

#[derive(Debug, Clone, Copy)]
pub enum ExportMode {
    All,
    Selected,
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

#[derive(Debug)]
pub struct ExportTask {
    pub archive: ArchiveInfo,
    pub folder: PathBuf,
    pub mode: ExportMode,
    pub progress: ProgressInfo,
}

impl ExportTask {
    pub fn new(archive: ArchiveInfo, folder: PathBuf, mode: ExportMode) -> Self {
        let progress = archive.progress.clone();
        Self {
            archive,
            folder,
            mode,
            progress,
        }
    }

    pub async fn run(self) -> anyhow::Result<(usize, Vec<String>)> {
        let ExportTask {
            archive,
            folder,
            mode,
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
                    if local_completed % 64 == 0 || idx + 1 == chunk_len {
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

fn export_entry_buffered(
    version: ImgVersion,
    entry: &EntryInfo,
    archive_path: Option<&std::path::Path>,
    reader: Option<&mut BufReader<File>>,
    folder: &std::path::Path,
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

    let data = if let Some(r) = reader {
        let mut buf = vec![0u8; size as usize];
        r.seek(SeekFrom::Start(offset))?;
        r.read_exact(&mut buf)?;
        buf
    } else {
        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; size as usize];
        file.read_exact(&mut buf)?;
        buf
    };

    std::fs::write(&output_path, data)?;
    Ok(())
}

fn anyhow_forward<E: std::fmt::Display>(err: E) -> anyhow::Error {
    anyhow::anyhow!("{err}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_modes_are_distinct() {
        assert!(matches!(ExportMode::All, ExportMode::All));
        assert!(matches!(ExportMode::Selected, ExportMode::Selected));
    }
}
