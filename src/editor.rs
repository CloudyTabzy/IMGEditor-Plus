use std::path::PathBuf;

use crate::archive::{ArchiveInfo, EntryInfo};
use crate::parser::{ImgVersion, import_entry};

#[derive(Debug, Default)]
pub struct Editor {
    archives: Vec<ArchiveInfo>,
    selected_archive: Option<usize>,
    selected_entry: Option<usize>,
    pending_messages: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenArchiveError {
    #[error("IMG format not supported")]
    UnsupportedFormat,
    #[error(transparent)]
    OpenFailed(#[from] anyhow::Error),
}

impl Editor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn archives(&self) -> &[ArchiveInfo] {
        &self.archives
    }

    pub fn archives_mut(&mut self) -> &mut Vec<ArchiveInfo> {
        &mut self.archives
    }

    pub fn selected_archive(&self) -> Option<usize> {
        self.selected_archive
    }

    pub fn selected_archive_mut(&mut self) -> Option<&mut ArchiveInfo> {
        self.selected_archive
            .and_then(|index| self.archives.get_mut(index))
    }

    pub fn selected_entry(&self) -> Option<usize> {
        self.selected_entry
    }

    pub fn take_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_messages)
    }

    pub fn add_message(&mut self, message: String) {
        self.pending_messages.push(message);
    }

    pub fn add_archive(&mut self, archive: ArchiveInfo) {
        self.archives.push(archive);
        self.selected_archive = Some(self.archives.len() - 1);
        self.selected_entry = None;
    }

    pub fn new_archive(&mut self) {
        let name = unique_archive_name(&self.archives, "Untitled");
        let archive = ArchiveInfo::new(name, true, ImgVersion::One);
        self.add_archive(archive);
    }

    pub fn open_archive(&mut self, path: impl Into<PathBuf>) -> Result<(), OpenArchiveError> {
        let path = path.into();
        let file_name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Untitled")
            .to_string();

        if self.archive_exists_by_name(&file_name) {
            return Ok(());
        }

        let version = crate::parser::detect_version(&path);
        if version == ImgVersion::Unknown {
            return Err(OpenArchiveError::UnsupportedFormat);
        }

        let archive = ArchiveInfo::open(path).map_err(OpenArchiveError::OpenFailed)?;
        self.add_archive(archive);
        Ok(())
    }

    pub fn close_archive(&mut self, index: usize) {
        if index < self.archives.len() {
            self.archives.remove(index);

            self.selected_archive = match self.archives.len() {
                0 => None,
                len => Some((self.selected_archive.unwrap_or(0)).min(len - 1)),
            };
            self.selected_entry = None;
        }
    }

    pub fn close_selected_archive(&mut self) {
        if let Some(index) = self.selected_archive {
            self.close_archive(index);
        }
    }

    pub fn select_archive(&mut self, index: usize) {
        if index < self.archives.len() {
            self.selected_archive = Some(index);
            self.selected_entry = None;
        }
    }

    pub fn set_selected_entry(&mut self, index: Option<usize>) {
        self.selected_entry = index;
    }

    pub fn select_entry(&mut self, clicked: usize, shift: bool, ctrl: bool) {
        let anchor = self.selected_entry.unwrap_or(clicked);
        let new_selected = Some(clicked);

        if shift {
            let Some(archive) = self.selected_archive_mut() else {
                return;
            };
            for entry in &mut archive.entries {
                entry.rename = false;
            }
            let start = anchor.min(clicked);
            let end = anchor.max(clicked);

            if !ctrl {
                for entry in &mut archive.entries {
                    entry.selected = false;
                }
            }

            for i in start..=end {
                if let Some(entry) = archive.entries.get_mut(i) {
                    entry.selected = true;
                }
            }
            archive.refresh_export_status();
            archive.rebuild_row_cache();
        } else if let Some(archive) = self.selected_archive_mut() {
            for entry in &mut archive.entries {
                entry.rename = false;
            }

            if !ctrl {
                for entry in &mut archive.entries {
                    entry.selected = false;
                }
            }

            if let Some(entry) = archive.entries.get_mut(clicked) {
                entry.selected = !entry.selected;
            }
            archive.refresh_export_status();
            archive.rebuild_row_cache();
        } else {
            return;
        }

        self.selected_entry = new_selected;
    }

    pub fn select_all(&mut self, selected: bool) {
        if let Some(archive) = self.selected_archive_mut() {
            for entry in &mut archive.entries {
                entry.selected = selected;
            }
            archive.refresh_export_status();
            archive.rebuild_row_cache();
        }
    }

    pub fn invert_selection(&mut self) {
        if let Some(archive) = self.selected_archive_mut() {
            for entry in &mut archive.entries {
                entry.selected = !entry.selected;
            }
            archive.refresh_export_status();
            archive.rebuild_row_cache();
        }
    }

    pub fn delete_selected(&mut self) {
        if let Some(archive) = self.selected_archive_mut() {
            archive.entries.retain(|entry| !entry.selected);
            archive.update_selected_list("");
            archive.dirty = true;
            self.selected_entry = None;
        }
    }

    pub fn rename_selected(&mut self, new_name: &str) {
        let selected = self.selected_entry;
        if let Some(archive) = self.selected_archive_mut() {
            if let Some(index) = selected {
                if let Some(entry) = archive.entries.get_mut(index) {
                    let mut updated = EntryInfo::new(new_name);
                    updated.offset = entry.offset;
                    updated.sector = entry.sector;
                    updated.source_path = entry.source_path.clone();
                    updated.imported = entry.imported;
                    updated.selected = entry.selected;
                    *entry = updated;
                    archive.dirty = true;
                    archive.update_selected_list("");
                }
            }
            for entry in &mut archive.entries {
                entry.rename = false;
            }
        }
    }

    pub fn import_files(&mut self, paths: &[PathBuf], replace: bool) {
        let Some(archive) = self.selected_archive_mut() else {
            return;
        };

        let mut count = 0;
        for path in paths {
            if import_entry(archive, path, replace).is_ok() {
                count += 1;
            }
        }

        if count > 0 {
            archive.dirty = true;
        }
        archive.add_log(format!("Imported {count} entries"));
        archive.update_search = true;
    }

    pub fn update_filtered_list(&mut self, filter: &str) {
        if let Some(archive) = self.selected_archive_mut() {
            archive.update_selected_list(filter);
            archive.update_search = false;
        }
    }

    /// Take a clone of the currently selected archive (returns None if none).
    /// Used by the App layer to launch async I/O tasks without holding &mut self.
    pub fn clone_selected_archive(&self) -> Option<(usize, ArchiveInfo)> {
        let index = self.selected_archive?;
        self.archives.get(index).map(|a| (index, a.clone()))
    }

    /// Replace the archive at `index` (used after an async save completes).
    pub fn replace_archive(&mut self, index: usize, archive: ArchiveInfo) {
        if index < self.archives.len() {
            self.archives[index] = archive;
        }
    }

    pub fn add_log_to(&mut self, index: usize, message: String) {
        if let Some(archive) = self.archives.get_mut(index) {
            archive.add_log(message);
        }
    }

    pub fn append_import(&mut self, index: usize, paths: Vec<PathBuf>, replace: bool) {
        if let Some(archive) = self.archives.get_mut(index) {
            let mut count = 0;
            for path in &paths {
                if import_entry(archive, path, replace).is_ok() {
                    count += 1;
                }
            }
            if count > 0 {
                archive.dirty = true;
            }
            archive.add_log(format!("Imported {count} entries"));
            archive.update_search = true;
        }
    }

    pub fn has_active_progress(&self) -> bool {
        self.archives
            .iter()
            .any(|archive| archive.progress.in_use())
    }

    fn archive_exists_by_name(&self, name: &str) -> bool {
        self.archives
            .iter()
            .any(|archive| archive.file_name == name)
    }
}

fn unique_archive_name(archives: &[ArchiveInfo], base: &str) -> String {
    if !archives.iter().any(|archive| archive.file_name == base) {
        return base.to_string();
    }

    for index in 2..100 {
        let candidate = format!("{base}({index})");
        if !archives
            .iter()
            .any(|archive| archive.file_name == candidate)
        {
            return candidate;
        }
    }

    base.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_archive_creates_untitled_tab() {
        let mut editor = Editor::new();
        editor.new_archive();
        assert_eq!(editor.archives.len(), 1);
        assert_eq!(editor.archives[0].file_name, "Untitled");
        assert_eq!(editor.selected_archive, Some(0));
    }

    #[test]
    fn new_archive_avoids_duplicate_names() {
        let mut editor = Editor::new();
        editor.new_archive();
        editor.new_archive();
        assert_eq!(editor.archives[0].file_name, "Untitled");
        assert_eq!(editor.archives[1].file_name, "Untitled(2)");
    }

    #[test]
    fn close_archive_updates_selection() {
        let mut editor = Editor::new();
        editor.new_archive();
        editor.new_archive();
        editor.close_archive(0);
        assert_eq!(editor.archives.len(), 1);
        assert_eq!(editor.selected_archive, Some(0));
    }

    #[test]
    fn select_all_and_invert() {
        let mut editor = Editor::new();
        editor.new_archive();
        editor.archives[0].entries.push(EntryInfo::new("a.dff"));
        editor.archives[0].entries.push(EntryInfo::new("b.txd"));

        editor.select_all(true);
        assert!(editor.archives[0].entries.iter().all(|e| e.selected));

        editor.invert_selection();
        assert!(editor.archives[0].entries.iter().all(|e| !e.selected));
    }

    #[test]
    fn delete_selected_removes_entries() {
        let mut editor = Editor::new();
        editor.new_archive();
        editor.archives[0].entries.push(EntryInfo::new("a.dff"));
        editor.archives[0].entries.push(EntryInfo::new("b.txd"));
        editor.archives[0].entries[0].selected = true;

        editor.delete_selected();
        assert_eq!(editor.archives[0].entries.len(), 1);
        assert_eq!(editor.archives[0].entries[0].file_name, "b.txd");
    }

    #[test]
    fn rename_selected_updates_entry() {
        let mut editor = Editor::new();
        editor.new_archive();
        editor.archives[0].entries.push(EntryInfo::new("a.dff"));
        editor.selected_entry = Some(0);

        editor.rename_selected("renamed.txd");
        assert_eq!(editor.archives[0].entries[0].file_name, "renamed.txd");
        assert_eq!(editor.archives[0].entries[0].file_type, "Texture");
    }
}
