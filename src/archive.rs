use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use compact_str::CompactString;
use memmap2::Mmap;
use smallvec::SmallVec;

use crate::parser::{EntryInspection, ImgParser, ImgVersion, MAX_ENTRY_NAME_BYTES, encode_entry_name};

#[derive(Debug, Clone)]
pub struct ProgressInfo {
    inner: Arc<ProgressInner>,
}



#[derive(Debug)]
struct ProgressInner {
    percentage: AtomicU32,
    cancel: AtomicBool,
    in_use: AtomicBool,
}

impl Default for ProgressInfo {
    fn default() -> Self {
        Self {
            inner: Arc::new(ProgressInner {
                percentage: AtomicU32::new(0),
                cancel: AtomicBool::new(false),
                in_use: AtomicBool::new(false),
            }),
        }
    }
}

impl ProgressInfo {
    pub fn start(&self) {
        self.inner.cancel.store(false, Ordering::Release);
        self.inner.percentage.store(0, Ordering::Release);
        self.inner.in_use.store(true, Ordering::Release);
    }

    pub fn finish(&self) {
        self.inner.in_use.store(false, Ordering::Release);
        self.inner.cancel.store(false, Ordering::Release);
        self.inner.percentage.store(0, Ordering::Release);
    }

    pub fn set_percentage(&self, value: f32) {
        let clamped = value.clamp(0.0, 1.0);
        self.inner
            .percentage
            .store(clamped.to_bits(), Ordering::Release);
    }

    pub fn percentage(&self) -> f32 {
        f32::from_bits(self.inner.percentage.load(Ordering::Acquire))
    }

    pub fn request_cancel(&self) {
        self.inner.cancel.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.cancel.load(Ordering::Acquire)
    }

    pub fn in_use(&self) -> bool {
        self.inner.in_use.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Name,
    Type,
    Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone)]
pub struct SortState {
    pub column: SortColumn,
    pub direction: SortDirection,
    pub type_index: usize,
}

impl Default for SortState {
    fn default() -> Self {
        Self {
            column: SortColumn::Name,
            direction: SortDirection::Ascending,
            type_index: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryInfo {
    pub offset: u32,
    pub sector: u32,
    pub file_name: CompactString,
    pub file_name_raw: [u8; MAX_ENTRY_NAME_BYTES],
    pub file_type: CompactString,
    pub source_path: Option<PathBuf>,
    pub imported: bool,
    pub rename: bool,
    pub selected: bool,
}

impl EntryInfo {
    pub fn new(file_name: impl Into<CompactString>) -> Self {
        let file_name = file_name.into();
        let file_name_raw = encode_entry_name(&file_name);
        let file_type = infer_file_type(&file_name);

        Self {
            offset: 0,
            sector: 0,
            file_name,
            file_name_raw,
            file_type,
            source_path: None,
            imported: false,
            rename: false,
            selected: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArchiveInfo {
    pub path: Option<PathBuf>,
    pub file_name: String,
    pub entries: Vec<EntryInfo>,
    pub selected_indices: SmallVec<[usize; 8]>,
    pub logs: Vec<String>,
    pub progress: ProgressInfo,
    pub version: ImgVersion,
    pub open: bool,
    pub create_new: bool,
    pub update_search: bool,
    pub dirty: bool,
    pub source_mmap: Option<Arc<Mmap>>,
    pub last_export_folder: Option<PathBuf>,
    pub sort: SortState,
    pub inspection_cache: std::collections::HashMap<usize, EntryInspection>,
}

impl ArchiveInfo {
    pub fn new(file_name: impl Into<String>, create_new: bool, version: ImgVersion) -> Self {
        let mut archive = Self {
            path: None,
            file_name: file_name.into(),
            entries: Vec::new(),
            selected_indices: SmallVec::new(),
            logs: Vec::new(),
            progress: ProgressInfo::default(),
            version,
            open: true,
            create_new,
            update_search: false,
            dirty: false,
            source_mmap: None,
            last_export_folder: None,
            sort: SortState::default(),
            inspection_cache: std::collections::HashMap::new(),
        };

        archive.add_log("Created archive".to_string());
        archive
    }

    pub fn open(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = path.into();
        let version = crate::parser::detect_version(&path);

        let mut archive = Self {
            path: Some(path.clone()),
            file_name: path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("Untitled")
                .to_string(),
            entries: Vec::new(),
            selected_indices: SmallVec::new(),
            logs: Vec::new(),
            progress: ProgressInfo::default(),
            version,
            open: true,
            create_new: false,
            update_search: false,
            dirty: false,
            source_mmap: None,
            last_export_folder: None,
            sort: SortState::default(),
            inspection_cache: std::collections::HashMap::new(),
        };

        match version {
            ImgVersion::One => crate::parser::PcV1Parser.open(&mut archive)?,
            ImgVersion::Two => crate::parser::PcV2Parser.open(&mut archive)?,
            ImgVersion::Unknown => crate::parser::UnknownParser.open(&mut archive)?,
        }

        archive.update_selected_list("");
        Ok(archive)
    }

    pub fn add_log(&mut self, message: String) {
        let now = chrono::Local::now().format("%H:%M:%S");
        self.logs.push(format!("[{}] {}", now, message));
    }

    pub fn update_selected_list(&mut self, filter: &str) {
        let filter = filter.to_lowercase();
        self.selected_indices.clear();

        let mut matches: Vec<(usize, &EntryInfo)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.file_name.to_lowercase().contains(&filter))
            .collect();

        let unique_types = self.unique_file_types();

        matches.sort_by(|(_, a), (_, b)| match self.sort.column {
            SortColumn::Name => {
                let ord = a.file_name.cmp(&b.file_name);
                if self.sort.direction == SortDirection::Descending {
                    ord.reverse()
                } else {
                    ord
                }
            }
            SortColumn::Type => {
                let primary = if unique_types.is_empty() {
                    CompactString::new("")
                } else {
                    unique_types[self.sort.type_index % unique_types.len()].clone()
                };
                let a_primary = a.file_type == primary;
                let b_primary = b.file_type == primary;
                let primary_ord = b_primary.cmp(&a_primary);
                if primary_ord != std::cmp::Ordering::Equal {
                    primary_ord
                } else {
                    a.file_name.cmp(&b.file_name)
                }
            }
            SortColumn::Size => {
                let ord = a.sector.cmp(&b.sector);
                if self.sort.direction == SortDirection::Descending {
                    ord.reverse()
                } else {
                    ord
                }
            }
        });

        for (index, _) in matches {
            self.selected_indices.push(index);
        }
    }

    pub(crate) fn unique_file_types(&self) -> Vec<CompactString> {
        let mut types: Vec<CompactString> =
            self.entries.iter().map(|e| e.file_type.clone()).collect();
        types.sort();
        types.dedup();
        types
    }
}

pub fn infer_file_type(file_name: &str) -> CompactString {
    let lower = file_name.to_ascii_lowercase();

    if lower.contains(".dff") {
        CompactString::new("Model")
    } else if lower.contains(".txd") {
        CompactString::new("Texture")
    } else if lower.contains(".col") {
        CompactString::new("Collision")
    } else if lower.contains(".ifp") {
        CompactString::new("Animation")
    } else if lower.contains(".ipl") {
        CompactString::new("Placement")
    } else if lower.contains(".ide") {
        CompactString::new("Definition")
    } else if lower.contains(".dat") {
        CompactString::new("Data")
    } else {
        std::path::Path::new(file_name)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| CompactString::new(format!(".{ext} file", ext = ext.to_ascii_lowercase())))
            .unwrap_or_else(|| CompactString::new("file"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_known_types() {
        assert_eq!(infer_file_type("player.dff"), "Model");
        assert_eq!(infer_file_type("PLAYER.TXD"), "Texture");
        assert_eq!(infer_file_type("coll.col"), "Collision");
        assert_eq!(infer_file_type("anim.ifp"), "Animation");
        assert_eq!(infer_file_type("item.ipl"), "Placement");
        assert_eq!(infer_file_type("object.ide"), "Definition");
        assert_eq!(infer_file_type("data.dat"), "Data");
    }

    #[test]
    fn infer_substring_match() {
        assert_eq!(infer_file_type("player.dff.backup"), "Model");
    }

    #[test]
    fn infer_unknown_extension() {
        assert_eq!(infer_file_type("readme.txt"), ".txt file");
    }

    #[test]
    fn infer_no_extension() {
        assert_eq!(infer_file_type("readme"), "file");
    }

    #[test]
    fn entry_info_new_sets_raw_name() {
        let entry = EntryInfo::new("test.dff");
        assert_eq!(entry.file_name, "test.dff");
        assert_eq!(entry.file_type, "Model");
        assert_eq!(&entry.file_name_raw[..8], b"test.dff");
    }

    #[test]
    fn archive_update_selected_list_filters_by_name() {
        let mut archive = ArchiveInfo::new("test", true, ImgVersion::One);
        archive.entries.push(EntryInfo::new("aaa.dff"));
        archive.entries.push(EntryInfo::new("bbb.txd"));
        archive.entries.push(EntryInfo::new("aab.dff"));

        archive.update_selected_list("aa");
        assert_eq!(archive.selected_indices.as_slice(), &[0, 2]);

        archive.update_selected_list("txd");
        assert_eq!(archive.selected_indices.as_slice(), &[1]);
    }

    #[test]
    fn progress_clamps_to_unit_range() {
        let progress = ProgressInfo::default();
        progress.start();
        progress.set_percentage(2.0);
        assert!((progress.percentage() - 1.0).abs() < 0.001);
        progress.set_percentage(-0.5);
        assert!(progress.percentage().abs() < 0.001);
        progress.set_percentage(0.42);
        assert!((progress.percentage() - 0.42).abs() < 0.001);
        progress.finish();
    }
}
