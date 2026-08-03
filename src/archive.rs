use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use compact_str::CompactString;
use memmap2::Mmap;
use smallvec::SmallVec;

use crate::parser::{DecodedTexture, EntryInspection, ImgParser, ImgVersion, MAX_ENTRY_NAME_BYTES, encode_entry_name};
use crate::sort::SortChain;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportStatus {
    Idle,
    Ready,
    Exporting,
    Done,
}

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
        self.inner.percentage.store(f32::to_bits(1.0), Ordering::Release);
    }

    pub fn reset(&self) {
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
    /// Display-only: the underlying sort lives on
    /// `ArchiveInfo::sort_chain` (a `SortChain` of up to 10 priority
    /// slots). These three variants are kept as a compatibility
    /// shim for the existing single-column header buttons in
    /// `view.rs`; they map to the primary slot of the chain at the
    /// call site.
    Name,
    Type,
    Size,
}

/// Header-button state. The actual sort algorithm lives on
/// `sort_chain`; this struct only tracks which column header is
/// visually active, which direction the arrow points, and the
/// cached "primary type" label for the Type column. `direction`
/// is `crate::sort::SortDirection` so the display shim and the
/// new chain share a single direction type.
#[derive(Debug, Clone)]
pub struct SortState {
    pub column: SortColumn,
    pub direction: crate::sort::SortDirection,
    pub type_index: usize,
    /// Cached header text for the Type column. Recomputed only when
    /// the underlying file-type set changes.
    pub type_header_label: String,
}

impl Default for SortState {
    fn default() -> Self {
        Self {
            column: SortColumn::Name,
            direction: crate::sort::SortDirection::Ascending,
            type_index: 0,
            type_header_label: "Type".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryInfo {
    pub offset: u32,
    pub sector: u32,
    pub file_name: CompactString,
    /// Lowercase version of `file_name` for fast case-insensitive filtering.
    pub file_name_lower: CompactString,
    pub file_name_raw: [u8; MAX_ENTRY_NAME_BYTES],
    pub file_type: CompactString,
    pub source_path: Option<PathBuf>,
    pub imported: bool,
    pub rename: bool,
    pub selected: bool,
}

impl EntryInfo {
    pub fn new(file_name: impl Into<CompactString>) -> Self {
        let file_name: CompactString = file_name.into();
        let file_name_lower = CompactString::new(file_name.to_lowercase());
        let file_name_raw = encode_entry_name(&file_name);
        let file_type = infer_file_type(&file_name);

        Self {
            offset: 0,
            sector: 0,
            file_name,
            file_name_lower,
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
    pub export_status: ExportStatus,
    pub last_export_count: usize,
    pub recent_exports: Vec<String>,
    pub version: ImgVersion,
    pub open: bool,
    pub create_new: bool,
    pub update_search: bool,
    pub dirty: bool,
    pub source_mmap: Option<Arc<Mmap>>,
    pub last_export_folder: Option<PathBuf>,
    /// Display-only: which header column the user clicked, used
    /// to drive the legacy `archive.sort.column`/`direction` paths
    /// in `view.rs`. Multi-key sorting is driven by
    /// `sort_chain` directly.
    pub sort: SortState,
    /// Multi-key sort configuration for this archive. Copied from
    /// `Config::default_sort_chain` when the archive is opened, so
    /// archives don't bleed sort state into each other.
    pub sort_chain: SortChain,
    pub inspection_cache: std::collections::HashMap<usize, EntryInspection>,
    /// Cached decoded TXD textures per entry index.
    pub txd_cache: std::collections::HashMap<usize, Vec<DecodedTexture>>,
    /// Cache for `unique_file_types()` invalidated whenever entries are added,
    /// removed, or renamed.
    cached_file_types: Option<Vec<CompactString>>,
    /// Reverse lookup from entry index to its position in `selected_indices`.
    /// Rebuilt by `update_selected_list` so shift+click and similar operations
    /// avoid linear scans of the filtered list.
    pub(crate) selected_lookup: HashMap<usize, usize>,
    /// Index of the entry currently in rename mode. Tracking this directly
    /// avoids scanning every entry to clear the rename flag on each click.
    pub rename_index: Option<usize>,
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
            export_status: ExportStatus::Idle,
            last_export_count: 0,
            recent_exports: Vec::new(),
            version,
            open: true,
            create_new,
            update_search: false,
            dirty: false,
            source_mmap: None,
            last_export_folder: None,
            sort: SortState::default(),
            sort_chain: SortChain::default(),
            inspection_cache: std::collections::HashMap::new(),
            txd_cache: std::collections::HashMap::new(),
            cached_file_types: None,
            selected_lookup: HashMap::new(),
            rename_index: None,
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
            export_status: ExportStatus::Idle,
            last_export_count: 0,
            recent_exports: Vec::new(),
            version,
            open: true,
            create_new: false,
            update_search: false,
            dirty: false,
            source_mmap: None,
            last_export_folder: None,
            sort: SortState::default(),
            sort_chain: SortChain::default(),
            inspection_cache: std::collections::HashMap::new(),
            txd_cache: std::collections::HashMap::new(),
            cached_file_types: None,
            selected_lookup: HashMap::new(),
            rename_index: None,
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
        self.selected_lookup.clear();

        // Eagerly clone the cached file-type list. `unique_file_types` needs
        // `&mut self` to populate the cache, so it must happen before we hold
        // any borrowed `EntryInfo` references from `self.entries`.
        let unique_types = self.unique_file_types().to_vec();

        let mut matches: Vec<(usize, &EntryInfo)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.file_name_lower.contains(&filter))
            .collect();

        // Build the IDE/COL sort context once per sort so the
        // comparator can resolve labels for the entry names we're
        // about to order. Without a resolved mapping the chain
        // gracefully falls back to name sorting.
        let primary_type = if self.sort.column == SortColumn::Type && !unique_types.is_empty() {
            Some(
                unique_types
                    .get(self.sort.type_index % unique_types.len())
                    .map(|s| s.as_str())
                    .unwrap_or(""),
            )
        } else {
            None
        };
        // We don't currently maintain per-entry IDE/COL maps in
        // ArchiveInfo; the comparator falls through to name sort
        // when the maps are empty, which is the safe default.
        let sort_ctx = crate::sort::SortContext {
            primary_type,
            ..crate::sort::SortContext::empty()
        };

        // Use the multi-key chain for the actual ordering. The
        // legacy single-column `sort` field is only used to drive
        // the "primary type" bubble via `primary_type` above; the
        // full chain takes over from there.
        matches.sort_by(|(_, a), (_, b)| self.sort_chain.cmp(a, b, &sort_ctx));

        for (display_row, (entry_index, _)) in matches.into_iter().enumerate() {
            self.selected_lookup.insert(entry_index, display_row);
            self.selected_indices.push(entry_index);
        }

        self.sort.type_header_label = if self.sort.column == SortColumn::Type {
            let primary = unique_types
                .get(self.sort.type_index % unique_types.len().max(1))
                .map(|s| s.as_str())
                .unwrap_or("");
            format!("Type ↑ {}", primary)
        } else {
            "Type".to_string()
        };

        self.refresh_export_status();
    }

    pub fn refresh_export_status(&mut self) {
        if self.progress.in_use() {
            self.export_status = ExportStatus::Exporting;
        } else {
            let selected = self.entries.iter().filter(|e| e.selected).count();
            match self.export_status {
                ExportStatus::Done if selected == 0 => {
                    self.export_status = ExportStatus::Idle;
                }
                ExportStatus::Done if selected != self.last_export_count => {
                    self.export_status = ExportStatus::Ready;
                    self.progress.reset();
                }
                ExportStatus::Exporting => {
                    self.export_status = if selected > 0 {
                        ExportStatus::Ready
                    } else {
                        ExportStatus::Idle
                    };
                    self.progress.reset();
                }
                _ => {
                    self.export_status = if selected > 0 {
                        ExportStatus::Ready
                    } else {
                        ExportStatus::Idle
                    };
                    if self.export_status == ExportStatus::Idle {
                        self.progress.reset();
                    }
                }
            }
        }
    }

    /// Returns the sorted, deduplicated list of file types in this archive.
    /// The result is cached and only recomputed when the cache is invalidated
    /// by entry mutations.
    pub(crate) fn unique_file_types(&mut self) -> &[CompactString] {
        if self.cached_file_types.is_none() {
            let mut types: Vec<CompactString> =
                self.entries.iter().map(|e| e.file_type.clone()).collect();
            types.sort();
            types.dedup();
            self.cached_file_types = Some(types);
        }
        self.cached_file_types.as_deref().unwrap_or_default()
    }

    /// Invalidates caches that depend on the entry list or entry metadata.
    /// Call this after add/remove/rename/import operations.
    pub fn invalidate_entry_caches(&mut self) {
        self.cached_file_types = None;
    }

    /// O(1) lookup from entry index to its display row in the current filter/sort.
    /// Returns `None` if the entry is not currently visible.
    pub fn display_row_of(&self, entry_index: usize) -> Option<usize> {
        self.selected_lookup.get(&entry_index).copied()
    }

    /// Clears the rename state for the single entry that was in rename mode,
    /// if any. This avoids scanning the entire entry list on every click.
    pub fn clear_rename(&mut self) {
        if let Some(index) = self.rename_index.take()
            && let Some(entry) = self.entries.get_mut(index)
        {
            entry.rename = false;
        }
    }

    /// Sets the given entry as the active rename target, clearing any previous
    /// rename target first.
    pub fn set_rename(&mut self, index: usize) {
        self.clear_rename();
        if let Some(entry) = self.entries.get_mut(index) {
            entry.rename = true;
            self.rename_index = Some(index);
        }
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
    fn entry_info_caches_lowercase_name() {
        let entry = EntryInfo::new("MiXeD.DfF");
        assert_eq!(entry.file_name_lower, "mixed.dff");
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
    fn archive_selected_lookup_maps_entry_to_display_row() {
        let mut archive = ArchiveInfo::new("test", true, ImgVersion::One);
        archive.entries.push(EntryInfo::new("zzz.dff"));
        archive.entries.push(EntryInfo::new("aaa.txd"));
        archive.entries.push(EntryInfo::new("bbb.dff"));

        archive.update_selected_list("");
        assert_eq!(archive.display_row_of(0), Some(2));
        assert_eq!(archive.display_row_of(1), Some(0));
        assert_eq!(archive.display_row_of(2), Some(1));
        assert_eq!(archive.display_row_of(99), None);
    }

    #[test]
    fn archive_rename_tracking_targets_single_entry() {
        let mut archive = ArchiveInfo::new("test", true, ImgVersion::One);
        archive.entries.push(EntryInfo::new("a.dff"));
        archive.entries.push(EntryInfo::new("b.txd"));

        archive.set_rename(1);
        assert!(archive.entries[1].rename);
        assert!(!archive.entries[0].rename);
        assert_eq!(archive.rename_index, Some(1));

        archive.clear_rename();
        assert!(!archive.entries[1].rename);
        assert_eq!(archive.rename_index, None);
    }

    #[test]
    fn archive_file_type_cache_invalidated_on_entry_change() {
        let mut archive = ArchiveInfo::new("test", true, ImgVersion::One);
        archive.entries.push(EntryInfo::new("a.dff"));
        archive.entries.push(EntryInfo::new("b.txd"));

        let first = archive.unique_file_types().to_vec();
        assert_eq!(first, vec!["Model", "Texture"]);

        archive.invalidate_entry_caches();
        archive.entries.push(EntryInfo::new("c.col"));
        let second = archive.unique_file_types().to_vec();
        assert_eq!(second, vec!["Collision", "Model", "Texture"]);
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
