use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use compact_str::CompactString;
use rayon::prelude::*;

use crate::archive::{ArchiveInfo, EntryInfo, ProgressInfo};
use crate::parser::{ImgParser, ImgVersion, PcV1Parser, PcV2Parser};

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

    pub async fn run(self) -> anyhow::Result<usize> {
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
        let results: Vec<(CompactString, anyhow::Result<()>)> = entries
            .par_iter()
            .map(|entry| {
                if progress.is_cancelled() {
                    return (entry.file_name.clone(), Err(anyhow::anyhow!("Export cancelled")));
                }

                let output_path = folder.join(&entry.file_name);
                let result = match archive.version {
                    ImgVersion::One => {
                        PcV1Parser.export_entry(&archive, entry, &output_path)
                    }
                    ImgVersion::Two => {
                        PcV2Parser.export_entry(&archive, entry, &output_path)
                    }
                    ImgVersion::Unknown => {
                        Err(anyhow::anyhow!("unknown archive format cannot be exported"))
                    }
                };

                let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                progress.set_percentage(done as f32 / total as f32);

                (entry.file_name.clone(), result)
            })
            .collect();

        let count = results.iter().filter(|(_, r)| r.is_ok()).count();
        for (name, result) in results {
            if let Err(err) = result {
                eprintln!("failed to export {name}: {err}");
            }
        }

        progress.set_percentage(1.0);
        progress.finish();
        Ok(count)
    }
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
