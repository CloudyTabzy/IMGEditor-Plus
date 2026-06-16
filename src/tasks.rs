use std::path::PathBuf;

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
        let mut count = 0;

        for (index, entry) in entries.iter().enumerate() {
            if progress.is_cancelled() {
                progress.finish();
                anyhow::bail!("Export cancelled");
            }

            let output_path = folder.join(&entry.file_name);
            let archive_for_task = archive.clone();
            let entry_clone = entry.clone();
            let output_path_clone = output_path.clone();

            let result: anyhow::Result<()> = match archive.version {
                ImgVersion::One => tokio::task::spawn_blocking(move || {
                    PcV1Parser.export_entry(&archive_for_task, &entry_clone, &output_path_clone)
                })
                .await
                .map_err(anyhow_forward)?,
                ImgVersion::Two => tokio::task::spawn_blocking(move || {
                    PcV2Parser.export_entry(&archive_for_task, &entry_clone, &output_path_clone)
                })
                .await
                .map_err(anyhow_forward)?,
                ImgVersion::Unknown => {
                    progress.finish();
                    anyhow::bail!("unknown archive format cannot be exported");
                }
            };

            if result.is_ok() {
                count += 1;
            } else if let Err(err) = result {
                eprintln!("failed to export {}: {err}", entry.file_name);
            }

            progress.set_percentage((index + 1) as f32 / total.max(1) as f32);
        }

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
