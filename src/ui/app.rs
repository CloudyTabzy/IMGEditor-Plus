use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iced::advanced::widget::operation::scrollable::{AbsoluteOffset, scroll_to};
use iced::keyboard::{Event as KeyboardEvent, Modifiers};
use iced::widget::{Space, container, pane_grid};
use iced::{Element, Point, Subscription, Task, Theme};
use iced_aw::menu::{Item, Menu, MenuBar};
use iced_fonts::LUCIDE_FONT_BYTES;
use memmap2::Mmap;

use crate::archive::{ArchiveInfo, EntryInfo, ExportStatus, SortColumn};
use crate::sort::SortDirection;
use crate::dev_logger;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{Config, ThemeMode};
use crate::editor::Editor;
use crate::inspector::scene3d::mesh::SceneTexture;
use crate::inspector::viewer3d::{self, ViewerEvent};
use crate::parser::{
    DecodedTexture, EntryInspection, ImgVersion, inspect_entry_cached, inspect_entry_standalone,
};
use crate::tasks::{ExportEngine, ExportMode, ExportTask, SaveTask};
use crate::ui::animator::Animator;
use crate::ui::design::Design;
use crate::ui::dialogs::{self, SaveArchiveChoice};
use crate::ui::fonts;
use crate::ui::keymap::{Shortcut, detect_pressed, shortcut_display};
use crate::ui::theme::resolve_theme;
use crate::updater::{UpdateResult, UpdateState, check_updates_future};

const REPO_URL: &str = "https://github.com/CloudyTabzy/IMGEditor-Plus";
const UPDATER_REPO: &str = "CloudyTabzy/IMGEditor-Plus";

pub const ANIM_PROGRESS: crate::ui::animator::AnimationId = 1;
pub const ANIM_TOAST_OPACITY: crate::ui::animator::AnimationId = 2;

pub const ABOUT_TEXT: &str = concat!(
    "IMG Editor Plus v",
    env!("CARGO_PKG_VERSION"),
    "\n\nA pure Rust desktop editor for GTA IMG archives.\n\n",
    "Made by CloudyTabzy & Agents\n",
    "Based on the original ",
    "IMG Editor by Grinch_\n",
    "(https://github.com/user-grinch/IMGEditor)\n\n",
    "Supported formats:\n",
    "- GTA III\n",
    "- GTA Vice City\n",
    "- GTA San Andreas\n",
    "- Bully Scholarship Edition"
);

#[derive(Debug, Clone, Copy)]
pub struct AutoScroll {
    pub anchor: Option<Point>,
    pub initial_scroll_y: f32,
    pub current: Option<Point>,
}

/// Application event type. Heterogeneous by design — some variants carry
/// large payloads (`Viewer3dLoadCompleted::Scene`, `ExportCompleted::Vec<String>`)
/// while most are unit or single-value. Boxing the large variants would
/// shrink the inline footprint but force a heap allocation on every
/// `iced::Task::done(Message::…)`, which is the event-loop hot path.
/// Tracked in TODO §6.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Message {
    Noop,
    ShortcutPressed(Shortcut),

    NewArchive,
    OpenArchive,
    OpenArchiveResult(Option<PathBuf>),
    /// Open a path from the recent-files list. Carries the raw
    /// path as it appeared in the menu; missing entries are
    /// filtered out before this fires.
    OpenRecent(PathBuf),
    SaveArchive,
    SaveArchiveAs,
    SaveArchiveAsResult(Option<SaveArchiveChoice>),
    SaveCompleted {
        index: usize,
        result: Result<ArchiveInfo, String>,
    },
    CloseSelectedArchive,
    CloseArchiveTab(usize),
    SelectArchiveTab(usize),

    ImportFiles,
    ImportFilesResult(Vec<PathBuf>),
    ExportAll,
    ExportSelected,
    ExportFolderResult(Option<PathBuf>),
    ExportCompleted {
        index: usize,
        result: Result<(usize, Vec<String>), String>,
    },
    FastExportToggled(bool),

    SelectAll,
    InvertSelection,
    DeleteSelected,
    StartRename,
    RenameInputChanged(String),
    CommitRename,
    CancelRename,
    CancelActive,

    SearchChanged(String),
    DebounceTick,
    RefreshFilter,

    CopySelectedEntryDetails,
    CopyLogs,

    EntryClicked(usize),
    EntryDoubleClicked(usize),
    EntryRightClicked(usize),
    EntryContextAction(EntryAction),
    HideContextMenu,
    ModifiersChanged(Modifiers),
    AnimationTick(std::time::Instant),
    AutoScrollStarted,
    AutoScrollStartedAtRow(usize),
    AutoScrollMoved(Point),
    AutoScrollEnded,

    ShowAbout,
    HideAbout,
    ShowWelcome,
    HideWelcome,
    ToggleWelcomePersist(bool),
    ToggleUpdateDisabled(bool),
    ToggleUpdateNotifyDisabled(bool),
    ShowUnsupported(PathBuf),
    HideUnsupported,
    VisitRepository,
    HideUpdateStatus,

    CheckUpdatesManual,
    UpdateResultReceived(UpdateResult),

    SetTheme(ThemeMode),
    ToastTimeout,
    TickProgress,
    PaneResized(pane_grid::ResizeEvent),
    OpenLastExportFolder,
    SortBy(SortColumn),
    ScrollOffsetChanged(f32),
    EntryInspected {
        index: usize,
        inspection: EntryInspection,
    },

    FilesDropped(PathBuf),

    TxdDecodeRequested,
    TxdDecoded {
        index: usize,
        result: Result<Vec<DecodedTexture>, String>,
    },
    TxdSelectTexture(usize),
    TxdExportTextures,
    TxdExportFolderResult(Option<PathBuf>),

    ExportEmbeddedTexturesRequest { entry_index: usize, nif_basename: String },
    ExportEmbeddedTexturesFolderResult { entry_index: usize, nif_basename: String, folder: Option<PathBuf> },
    ExportEmbeddedTexturesCompleted { entry_index: usize, nif_basename: String, result: Result<crate::inspector::texture_export::ExportReport, String> },

    Viewer3dRequestLoad { archive_index: usize, entry_index: usize },
    Viewer3dLoadCompleted { archive_index: usize, entry_index: usize, result: Result<crate::inspector::scene3d::Scene, String> },
    Viewer3dSelectTab(InspectorTab),
    Viewer3dClear,
    Viewer3dReset,
    Viewer3dToggleWireframe,
    Viewer3dToggleCullBackfaces,
    Viewer3dToggleTextured,

    // Sort Manager dialog. The dialog edits a draft copy of the
    // active archive's SortChain; "Apply" commits the draft to the
    // archive + the global default. SlotIndex is a NewType so the
    // compiler refuses accidental cross-pollination with other
    // numeric state in the handler.
    OpenSortManager,
    CloseSortManager,
    SortApplyDraft,
    SortResetDraft,
    SortAddSlot,
    SortRemoveSlot(SortSlotIndex),
    SortMoveSlotUp(SortSlotIndex),
    SortMoveSlotDown(SortSlotIndex),
    SortToggleSlotEnabled(SortSlotIndex),
    SortSetSlotKey(SortSlotIndex, crate::sort::SortKey),
    SortSetSlotDirection(SortSlotIndex, crate::sort::SortDirection),
    SortSelectPreset(SortPreset),

    // ---- Drag-and-drop between archives ----
    /// User started dragging selected entries from a source archive
    /// tab. The App's `drag_state` is updated to remember the source
    /// archive + the entry indices being moved.
    ArchiveDragStarted { source: usize },
    /// Mouse moved while dragging. The optional `over` argument is
    /// the archive index the cursor is currently over (from
    /// `on_enter` / `on_exit` events on the tab strip). `None` means
    /// the cursor is over empty space.
    ArchiveDragMoved { over: Option<usize> },
    /// User released the mouse. If `over` is set, the entries are
    /// moved from the source to that archive; otherwise the drag is
    /// cancelled. The source is in the App's drag_state, not in the
    /// message, to avoid passing it through every event.
    ArchiveDragReleased,
    /// Cancel the drag in progress (Escape pressed, focus lost,
    /// window close, etc.). Distinct from "released" because the
    /// latter implies an explicit drop target.
    ArchiveDragCancelled,
}

/// NewType around `usize` that names a slot inside the Sort Manager's
/// draft chain. Using a distinct type prevents the compiler from
/// accepting a row index where a slot index is expected (or vice
/// versa) — both are `usize` underneath, but the wrappers make the
/// call sites self-documenting and catch bugs at the type level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SortSlotIndex(pub usize);

/// Built-in sort presets the user can apply with one click. The
/// index matches the dropdown order in `view.rs`; the payload is
/// the chain shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortPreset {
    NameAZ,
    NameZA,
    TypeThenName,
    SizeDesc,
    OffsetAsc,
}

impl SortPreset {
    /// Convert this preset to a `SortChain`. Each preset is a
    /// single fixed chain the user can further edit (toggle
    /// directions, add tiebreakers, etc.) before applying.
    pub fn to_chain(self) -> crate::sort::SortChain {
        use crate::sort::{SortChain, SortDirection, SortKey, SortPriority};
        let p = |key, dir| SortPriority {
            enabled: true,
            key,
            direction: dir,
        };
        match self {
            SortPreset::NameAZ => SortChain::new(vec![p(SortKey::Name, SortDirection::Ascending)]),
            SortPreset::NameZA => SortChain::new(vec![p(SortKey::Name, SortDirection::Descending)]),
            SortPreset::TypeThenName => SortChain::new(vec![
                p(SortKey::Type, SortDirection::Ascending),
                p(SortKey::Name, SortDirection::Ascending),
            ]),
            SortPreset::SizeDesc => SortChain::new(vec![p(SortKey::Size, SortDirection::Descending)]),
            SortPreset::OffsetAsc => SortChain::new(vec![p(SortKey::Offset, SortDirection::Ascending)]),
        }
    }

    /// Display name for the dropdown. Kept here (not in `view.rs`)
    /// so the preset list reads top-to-bottom in one place.
    pub fn display_name(self) -> &'static str {
        match self {
            SortPreset::NameAZ => "Name (A→Z)",
            SortPreset::NameZA => "Name (Z→A)",
            SortPreset::TypeThenName => "Type, then name",
            SortPreset::SizeDesc => "Size (big → small)",
            SortPreset::OffsetAsc => "Offset (low → high)",
        }
    }

    /// All presets in picker order. Used by the dropdown to
    /// populate its list without hard-coding it in `view.rs`.
    pub const ALL: &'static [SortPreset] = &[
        SortPreset::NameAZ,
        SortPreset::NameZA,
        SortPreset::TypeThenName,
        SortPreset::SizeDesc,
        SortPreset::OffsetAsc,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorTab {
    Export,
    Model3D,
    Texture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryAction {
    CopyName,
    Rename,
    Delete,
    Export,
    Render,
    RenderExternal,
    ViewTextures,
    ExportEmbeddedTextures,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Table,
    Info,
}

pub struct App {
    pub editor: Editor,
    pub config: Config,
    pub search: String,
    pub rename_buffer: String,
    pub show_about: bool,
    pub show_welcome: bool,
    pub welcome_persist: bool,
    pub show_unsupported: Option<PathBuf>,
    pub show_update_status: Option<String>,
    pub update_state: UpdateState,
    pub update_check_manual: bool,
    pub toast: Option<String>,
    /// Working copy of the sort chain while the Sort Manager
    /// dialog is open. Edits land here first; "Apply" commits the
    /// draft to the live archive + config. `None` when the dialog
    /// is closed.
    pub sort_draft: Option<crate::sort::SortChain>,
    /// `true` while the Sort Manager modal is visible.
    pub show_sort_manager: bool,
    /// In-flight drag-and-drop between archive tabs. `None` when no
    /// drag is in progress. Holds the source archive + the entry
    /// indices being moved + the currently-hovered target. The
    /// `Drop` impl on this struct also implements "the drag was
    /// cancelled" — see the `clear` method — so dropping the
    /// value without committing cleanly resets the UI.
    pub drag_state: Option<crate::ui::drag::DragState>,
    pub last_export_selected_only: bool,
    pub fast_export: bool,
    pub panes: pane_grid::State<Pane>,
    pub context_menu: Option<(usize, usize)>,
    pub inspected_entry: Option<(usize, EntryInspection)>,
    /// Index into the decoded TXD textures currently being viewed.
    pub txd_selected_texture: usize,
    pub scroll_y: f32,
    pub selected_inspector_tab: InspectorTab,
    pub viewer3d_handle: std::sync::Arc<crate::ui::viewer3d_widget::SceneHandle>,
    /// True when the search text has changed but the filtered list has not
    /// been updated yet. The filter is applied on a debounce tick so typing
    /// stays responsive even with large archives.
    pub filter_pending: bool,
    pub autoscroll: Option<AutoScroll>,
    pub modifiers: Modifiers,
    viewer_rxs: Vec<tokio::sync::mpsc::UnboundedReceiver<ViewerEvent>>,
    pub animator: Animator,
    prev_tick: Option<std::time::Instant>,
    toast_pulses_remaining: u32,
    toast_pulse_target: f32,
    toast_start: Option<std::time::Instant>,
}

impl Default for App {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

impl App {
    pub fn new(config: Config) -> Self {
        let show_welcome = !config.first_run_complete;
        let fast_export = config.fast_export;
        let (panes, pane) = pane_grid::State::new(Pane::Table);
        let mut panes = panes;
        panes.split(pane_grid::Axis::Vertical, pane, Pane::Info);

        Self {
            editor: Editor::new(),
            config,
            sort_draft: None,
            show_sort_manager: false,
            drag_state: None,
            last_export_selected_only: false,
            search: String::new(),
            rename_buffer: String::new(),
            show_about: false,
            show_welcome,
            welcome_persist: true,
            show_unsupported: None,
            show_update_status: None,
            update_state: UpdateState::Idle,
            update_check_manual: false,
            toast: None,
            fast_export,
            panes,
            context_menu: None,
            inspected_entry: None,
            txd_selected_texture: 0,
            scroll_y: 0.0,
            filter_pending: false,
            autoscroll: None,
            modifiers: Modifiers::default(),
            viewer_rxs: Vec::new(),
            animator: Animator::new(),
            prev_tick: None,
            toast_pulses_remaining: 0,
            toast_pulse_target: 0.0,
            toast_start: None,
            selected_inspector_tab: InspectorTab::Export,
            viewer3d_handle: std::sync::Arc::new(
                crate::ui::viewer3d_widget::SceneHandle::new(),
            ),
        }
    }

    pub fn theme(&self) -> Theme {
        resolve_theme(self.config.theme)
    }

    /// The design-token system for the current theme.
    pub fn design(&self) -> Design {
        let tokens = if matches!(self.config.theme, ThemeMode::DarkEverforest) {
            crate::ui::tokens::ThemeTokens::everforest()
        } else if self.theme().extended_palette().is_dark {
            crate::ui::tokens::ThemeTokens::dark()
        } else {
            crate::ui::tokens::ThemeTokens::light()
        };
        Design::from_tokens(
            tokens,
            self.theme().extended_palette().is_dark,
        )
    }

    pub fn startup_task(config: &Config) -> Task<Message> {
        let mut tasks = vec![
            iced::font::load(LUCIDE_FONT_BYTES).map(|_| Message::Noop),
        ];
        if config.update_check_enabled {
            tasks.push(Task::perform(
                check_updates_future(
                    UPDATER_REPO.to_string(),
                    env!("CARGO_PKG_VERSION").to_string(),
                ),
                Message::UpdateResultReceived,
            ));
        }
        Task::batch(tasks)
    }

    pub fn save_config(&self) {
        if let Err(err) = self.config.save() {
            eprintln!("failed to save config: {err}");
        }
    }

    /// Open an archive at `path`, record it in the recent-files MRU
    /// list on success, and surface failures via the toast or
    /// unsupported-format dialog. Shared by the file-picker handler
    /// and the Open-Recent menu so both paths get the same UX.
    fn open_archive_path(&mut self, path: PathBuf) {
        match self.editor.open_archive(&path) {
            Ok(()) => {
                self.config.recent_files.touch(&path);
                self.save_config();
            }
            Err(crate::editor::OpenArchiveError::UnsupportedFormat) => {
                // Don't touch the MRU list — a failed attempt
                // shouldn't promote a file we couldn't open.
                self.show_unsupported = Some(path);
            }
            Err(err) => {
                self.toast = Some(format!("Failed to open archive: {err}"));
            }
        }
    }

    pub fn visit_repository() {
        let _ = webbrowser::open(REPO_URL);
    }

    pub fn has_active_progress(&self) -> bool {
        self.editor.has_active_progress()
    }

    fn refresh_inspection(&mut self) -> Task<Message> {
        let selected_archive = self.editor.selected_archive();
        let selected_entry = self.editor.selected_entry();

        let (Some(archive_index), Some(entry_index)) = (selected_archive, selected_entry) else {
            self.inspected_entry = None;
            return Task::none();
        };

        // Fast path: serve from the per-archive cache (mmap reads -> instant).
        struct Miss {
            entry: EntryInfo,
            archive_path: Option<PathBuf>,
            mmap: Option<Arc<Mmap>>,
            archive_file_name: String,
        }

        let miss = {
            let archive = self.editor.archives_mut().get_mut(archive_index);
            let archive = match archive {
                Some(a) => a,
                None => {
                    self.inspected_entry = None;
                    return Task::none();
                }
            };
            if let Some(inspection) = inspect_entry_cached(archive, entry_index) {
                self.inspected_entry = Some((entry_index, inspection));
                return Task::none();
            }
            // Cache miss: capture minimal data while the borrow is live.
            let entry = archive.entries.get(entry_index).cloned();
            let archive_path = archive.path.clone();
            let mmap = archive.source_mmap.clone();
            let archive_file_name = archive.file_name.clone();
            entry.map(|entry| Miss {
                entry,
                archive_path,
                mmap,
                archive_file_name,
            })
        };

        let Some(miss) = miss else {
            self.inspected_entry = None;
            return Task::none();
        };

        self.inspected_entry = None;

        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mmap_ref = miss.mmap.as_deref();
                    inspect_entry_standalone(
                        &miss.entry,
                        miss.archive_path.as_deref(),
                        mmap_ref,
                        &miss.archive_file_name,
                    )
                })
                .await
                .ok()
            },
            move |maybe| {
                let Some(inspection) = maybe else {
                    return Message::Noop;
                };
                Message::EntryInspected {
                    index: entry_index,
                    inspection,
                }
            },
        )
    }

    fn display_row_to_entry(&self, display_row: usize) -> Option<usize> {
        self.editor
            .selected_archive()
            .and_then(|_| self.editor.archives().get(self.editor.selected_archive().unwrap_or(0)))
            .and_then(|a| a.selected_indices.get(display_row).copied())
    }

    fn run_refresh_filter(&mut self) -> Task<Message> {
        self.editor.update_filtered_list(&self.search);
        Task::none()
    }

    fn run_save(
        &self,
        archive: ArchiveInfo,
        path: PathBuf,
        version: ImgVersion,
        remove_existing: bool,
    ) -> Task<Message> {
        let index = self.editor.selected_archive().unwrap_or(0);
        let task = SaveTask::new(archive, path, version).remove_existing(remove_existing);
        Task::perform(
            async move { task.run().await.map_err(|e| e.to_string()) },
            move |result| Message::SaveCompleted { index, result },
        )
    }

    fn handle_shortcut(&mut self, shortcut: Shortcut) -> Task<Message> {
        match shortcut {
            Shortcut::New => Task::done(Message::NewArchive),
            Shortcut::Open => Task::done(Message::OpenArchive),
            Shortcut::Save => Task::done(Message::SaveArchive),
            Shortcut::SaveAs => Task::done(Message::SaveArchiveAs),
            Shortcut::Close => Task::done(Message::CloseSelectedArchive),
            Shortcut::Import => Task::done(Message::ImportFiles),
            Shortcut::ImportReplace => Task::done(Message::ImportFiles),
            Shortcut::ExportAll => Task::done(Message::ExportAll),
            Shortcut::ExportSelected => Task::done(Message::ExportSelected),
            Shortcut::SelectAll => Task::done(Message::SelectAll),
            Shortcut::InvertSelection => Task::done(Message::InvertSelection),
            Shortcut::Delete => Task::done(Message::DeleteSelected),
            Shortcut::FocusSearch => Task::none(),
            Shortcut::CheckUpdates => Task::done(Message::CheckUpdatesManual),
        }
    }
}

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Noop => Task::none(),

            Message::ShortcutPressed(shortcut) => self.handle_shortcut(shortcut),

            Message::NewArchive => {
                self.editor.new_archive();
                Task::none()
            }

            Message::OpenArchive => {
                self.toast = None;
                dialogs::open_file().map(Message::OpenArchiveResult)
            }

            Message::OpenArchiveResult(Some(path)) => {
                self.open_archive_path(path);
                Task::none()
            }
            Message::OpenArchiveResult(None) => Task::none(),

            Message::OpenRecent(path) => {
                // The menu only emits paths that still exist on disk
                // (RecentFiles::iter_existing), but the file may have
                // been deleted between menu render and click. Guard
                // anyway so we don't surprise the user with an
                // "unsupported format" toast.
                if !path.exists() {
                    self.config.recent_files.remove(&path);
                    self.save_config();
                    self.toast = Some(format!(
                        "File no longer exists: {}",
                        path.display()
                    ));
                    return Task::none();
                }
                self.open_archive_path(path);
                Task::none()
            }

            Message::SaveArchive => {
                self.toast = None;
                let Some((_index, archive)) = self.editor.clone_selected_archive() else {
                    self.toast = Some("No archive selected.".into());
                    return Task::none();
                };
                let Some(path) = archive.path.clone() else {
                    return Task::done(Message::SaveArchiveAs);
                };
                if !path.exists() {
                    return Task::done(Message::SaveArchiveAs);
                }
                let version = archive.version;
                self.run_save(archive, path, version, false)
            }

            Message::SaveArchiveAs => {
                let Some((_index, archive)) = self.editor.clone_selected_archive() else {
                    self.toast = Some("No archive selected.".into());
                    return Task::none();
                };
                let default_path = archive
                    .path
                    .clone()
                    .unwrap_or_else(|| PathBuf::from(format!("{}.img", archive.file_name)));
                let version = archive.version;
                dialogs::save_archive(default_path, version).map(Message::SaveArchiveAsResult)
            }

            Message::SaveArchiveAsResult(Some(choice)) => {
                let Some((_index, archive)) = self.editor.clone_selected_archive() else {
                    self.toast = Some("No archive selected.".into());
                    return Task::none();
                };
                self.run_save(archive, choice.path, choice.version, true)
            }
            Message::SaveArchiveAsResult(None) => Task::none(),

            Message::SaveCompleted { index, result } => {
                match result {
                    Ok(archive) => {
                        self.editor.replace_archive(index, archive);
                        self.toast = Some("Archive saved.".into());
                    }
                    Err(err) => {
                        self.toast = Some(format!("Save failed: {err}"));
                    }
                };
                Task::none()
            }
            _ => self.update_tail(message),
        }
    }

    fn update_tail(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Noop
            | Message::ShortcutPressed(_)
            | Message::NewArchive
            | Message::OpenArchive
            | Message::OpenArchiveResult(_)
            | Message::OpenRecent(_)
            | Message::SaveArchive
            | Message::SaveArchiveAs
            | Message::SaveArchiveAsResult(_)
            | Message::SaveCompleted { .. } => Task::none(),

            Message::CloseSelectedArchive => {
                self.editor.close_selected_archive();
                let task = self.refresh_inspection();
                Task::batch(vec![task, Task::none()])
            }
            Message::CloseArchiveTab(index) => {
                self.editor.close_archive(index);
                let task = self.refresh_inspection();
                Task::batch(vec![task, Task::none()])
            }
            Message::SelectArchiveTab(index) => {
                self.editor.select_archive(index);
                let task = self.refresh_inspection();
                Task::batch(vec![task, Task::none()])
            }

            Message::ImportFiles => {
                self.toast = None;
                dialogs::import_files().map(Message::ImportFilesResult)
            }
            Message::ImportFilesResult(paths) => {
                if paths.is_empty() {
                    return Task::none();
                }
                if self.editor.selected_archive().is_some() {
                    let count = paths.len();
                    if let Some((_index, _archive)) = self.editor.clone_selected_archive() {
                        self.editor.append_import(_index, paths, false);
                    }
                    self.toast = Some(format!("Imported {count} files."));
                } else {
                    self.toast = Some("Open an archive first to import into it.".into());
                }
                Task::none()
            }

            Message::ExportAll => self.start_export(ExportMode::All),
            Message::ExportSelected => self.start_export(ExportMode::Selected),

            Message::ExportFolderResult(Some(folder)) => {
                let Some((index, archive)) = self.editor.clone_selected_archive() else {
                    return Task::none();
                };
                let mode = if self.last_export_selected_only {
                    ExportMode::Selected
                } else {
                    ExportMode::All
                };
                self.last_export_selected_only = false;
                self.config.last_export_folder = Some(folder.clone());
                self.save_config();
                if let Some(archive) = self.editor.selected_archive_mut() {
                    archive.last_export_folder = Some(folder.clone());
                }
                let task = ExportTask::new(archive, folder, mode)
                    .engine(if self.fast_export {
                        ExportEngine::Fast
                    } else {
                        ExportEngine::Parallel
                    });
                Task::perform(
                    async move { task.run().await.map_err(|e| e.to_string()) },
                    move |result| Message::ExportCompleted { index, result },
                )
            }
            Message::ExportFolderResult(None) => Task::none(),

            Message::FastExportToggled(enabled) => {
                self.fast_export = enabled;
                self.config.fast_export = enabled;
                self.save_config();
                Task::none()
            }

            Message::ExportCompleted { index, result } => {
                if let Some(archive) = self.editor.archives_mut().get_mut(index) {
                    match result {
                        Ok((count, names)) => {
                            archive.export_status = ExportStatus::Done;
                            archive.last_export_count = count;
                            let now = chrono::Local::now().format("%H:%M:%S");
                            let summary = if count == 1 {
                                names.first().cloned().unwrap_or_else(|| "1 file".to_string())
                            } else {
                                format!("{count} files")
                            };
                            archive
                                .recent_exports
                                .push(format!("[{now}] Exported {summary}"));
                            archive.add_log(format!("Exported {count} entries"));
                            self.toast = Some(format!("Exported {count} entries."));
                        }
                        Err(err) => {
                            archive.export_status = ExportStatus::Idle;
                            archive.last_export_count = 0;
                            archive.add_log(format!("Export failed: {err}"));
                            self.toast = Some(format!("Export failed: {err}"));
                        }
                    }
                }
                Task::none()
            }

            Message::SelectAll => {
                self.editor.select_all(true);
                let task = self.refresh_inspection();
                Task::batch(vec![task, Task::none()])
            }
            Message::InvertSelection => {
                self.editor.invert_selection();
                let task = self.refresh_inspection();
                Task::batch(vec![task, Task::none()])
            }
            Message::DeleteSelected => {
                self.editor.delete_selected();
                let task = self.refresh_inspection();
                Task::batch(vec![task, Task::none()])
            }
            Message::StartRename => {
                if let Some(index) = self.editor.selected_entry() {
                    if let Some(archive) = self
                        .editor
                        .archives()
                        .get(self.editor.selected_archive().unwrap_or(0))
                        && let Some(entry) = archive.entries.get(index)
                    {
                        self.rename_buffer = entry.file_name.to_string();
                    }
                    if let Some(archive) = self.editor.selected_archive_mut() {
                        archive.set_rename(index);
                    }
                }
                Task::none()
            }
            Message::RenameInputChanged(value) => {
                self.rename_buffer = value;
                Task::none()
            }
            Message::CommitRename => {
                let new_name = self.rename_buffer.clone();
                self.editor.rename_selected(&new_name);
                self.rename_buffer.clear();
                Task::none()
            }
            Message::CancelRename => {
                if let Some(archive) = self.editor.selected_archive_mut() {
                    archive.clear_rename();
                }
                self.rename_buffer.clear();
                Task::none()
            }
            Message::CancelActive => {
                for archive in self.editor.archives_mut() {
                    if archive.progress.in_use() {
                        archive.progress.request_cancel();
                    }
                }
                Task::none()
            }

            Message::SearchChanged(value) => {
                // Update the bound search text immediately so the text_input
                // widget stays in sync with the user's keystrokes. The
                // expensive filter rebuild is deferred to DebounceTick.
                if value != self.search {
                    self.search = value;
                    self.filter_pending = true;
                }
                Task::none()
            }
            Message::DebounceTick => {
                if self.filter_pending {
                    self.filter_pending = false;
                    return self.run_refresh_filter();
                }
                Task::none()
            }
            Message::RefreshFilter => {
                self.editor.update_filtered_list(&self.search);
                Task::none()
            }

            Message::CopySelectedEntryDetails => {
                let Some((_, inspection)) = self.inspected_entry.as_ref() else {
                    return Task::none();
                };
                let mut lines = Vec::new();
                lines.push(format!("Name: {}", inspection.file_name));
                lines.push(format!("Type: {}", inspection.file_type));
                lines.push(format!(
                    "Size: {} bytes ({} sectors)",
                    inspection.size_bytes, inspection.size_sectors
                ));
                lines.push(format!(
                    "Offset: sector {} (byte {})",
                    inspection.offset_bytes / 2048,
                    inspection.offset_bytes
                ));
                lines.push(format!("Source: {}", inspection.source));
                for (key, value) in &inspection.summary {
                    lines.push(format!("{key}: {value}"));
                }
                let text = lines.join("\n");
                self.toast = Some("Copied selected entry details".to_string());
                iced::clipboard::write::<Message>(text)
            }
            Message::CopyLogs => {
                let Some(archive) = self.editor.archives().get(self.editor.selected_archive().unwrap_or(0)) else {
                    return Task::none();
                };
                let text = archive.logs.join("\n");
                self.toast = Some("Copied logs".to_string());
                iced::clipboard::write::<Message>(text)
            }

            Message::EntryClicked(display_row) => {
                let task = if let Some(entry_index) = self.display_row_to_entry(display_row) {
                    let shift = self.modifiers.shift();
                    let ctrl = self.modifiers.command();
                    self.editor.select_entry(entry_index, shift, ctrl);
                    self.refresh_inspection()
                } else {
                    Task::none()
                };
                Task::batch(vec![task, Task::none()])
            }
            Message::EntryDoubleClicked(display_row) => {
                let task = if let Some(entry_index) = self.display_row_to_entry(display_row) {
                    self.editor.set_selected_entry(Some(entry_index));
                    self.editor.select_entry(entry_index, false, false);
                    if let Some(archive) = self.editor.selected_archive_mut() {
                        archive.set_rename(entry_index);
                        if let Some(entry) = archive.entries.get(entry_index) {
                            self.rename_buffer = entry.file_name.to_string();
                        }
                    }
                    self.refresh_inspection()
                } else {
                    Task::none()
                };
                Task::batch(vec![task, Task::none()])
            }
            Message::EntryRightClicked(display_row) => {
                let task = if let Some(entry_index) = self.display_row_to_entry(display_row) {
                    self.editor.set_selected_entry(Some(entry_index));
                    self.context_menu = Some((entry_index, display_row));
                    self.refresh_inspection()
                } else {
                    Task::none()
                };
                Task::batch(vec![task, Task::none()])
            }
            Message::EntryContextAction(action) => {
                self.context_menu = None;
                match action {
                EntryAction::CopyName => {
                    if let Some(archive_index) = self.editor.selected_archive()
                        && let Some(entry_index) = self.editor.selected_entry()
                        && let Some(archive) = self.editor.archives().get(archive_index)
                        && let Some(entry) = archive.entries.get(entry_index)
                    {
                        let name = entry.file_name.to_string();
                        self.toast = Some(format!("Copied name: {}", name));
                        return iced::clipboard::write::<Message>(name);
                    }
                    Task::none()
                }
                EntryAction::Rename => Task::done(Message::StartRename),
                EntryAction::Delete => {
                    self.editor.delete_selected();
                    Task::none()
                }
                EntryAction::Export => {
                    self.last_export_selected_only = true;
                    dialogs::save_folder().map(Message::ExportFolderResult)
                }
                EntryAction::ViewTextures => {
                    Task::done(Message::TxdDecodeRequested)
                }
                EntryAction::ExportEmbeddedTextures => {
                    let Some(archive_index) = self.editor.selected_archive() else {
                        return Task::none();
                    };
                    let Some(entry_index) = self.editor.selected_entry() else {
                        return Task::none();
                    };
                    let (nif_basename, archive_path) = {
                        let Some(archive) = self.editor.archives().get(archive_index) else {
                            return Task::none();
                        };
                        let Some(entry) = archive.entries.get(entry_index) else {
                            return Task::none();
                        };
                        let stem = std::path::Path::new(&entry.file_name)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string());
                        let stem = match stem {
                            Some(s) => s,
                            None => {
                                self.toast =
                                    Some(format!("Cannot determine basename of {}", entry.file_name));
                                return Task::none();
                            }
                        };
                        (stem, archive.path.clone())
                    };
                    let _ = archive_path;
                    Task::done(Message::ExportEmbeddedTexturesRequest {
                        entry_index,
                        nif_basename,
                    })
                }
                EntryAction::Render => {
                    dev_logger::breadcrumb("user: open in 3D viewer (in-app)");
                    let Some(archive_index) = self.editor.selected_archive() else {
                        return Task::none();
                    };
                    let Some(entry_index) = self.editor.selected_entry() else {
                        return Task::none();
                    };
                    let lower = {
                        let Some(archive) = self.editor.archives().get(archive_index) else {
                            return Task::none();
                        };
                        let Some(entry) = archive.entries.get(entry_index) else {
                            return Task::none();
                        };
                        entry.file_name.to_lowercase()
                    };
                    if !lower.ends_with(".nif") {
                        self.toast = Some(format!(
                            "In-app 3D viewer only supports .nif ({}). Use 'Open in external viewer' for other formats.",
                            lower
                        ));
                        return Task::none();
                    }
                    self.selected_inspector_tab = InspectorTab::Model3D;
                    self.viewer3d_handle.clear();
                    Task::done(Message::Viewer3dRequestLoad {
                        archive_index,
                        entry_index,
                    })
                }
                EntryAction::RenderExternal => {
                    dev_logger::breadcrumb("user: open in external viewer (PLY)");
                    let Some(archive_index) = self.editor.selected_archive() else {
                        return Task::none();
                    };
                    let Some(entry_index) = self.editor.selected_entry() else {
                        return Task::none();
                    };
                    let (entry_clone, archive_path, name) = {
                        let Some(archive) = self.editor.archives().get(archive_index) else {
                            return Task::none();
                        };
                        let Some(entry) = archive.entries.get(entry_index) else {
                            return Task::none();
                        };
                        (entry.clone(), archive.path.clone(), entry.file_name.to_string())
                    };
                    let data = match crate::parser::read_entry_data_from_source(
                        &entry_clone,
                        archive_path.as_deref(),
                    ) {
                        Ok(d) => d,
                        Err(e) => {
                            self.toast = Some(format!("Failed to read {name}: {e}"));
                            return Task::none();
                        }
                    };

                    if name.to_lowercase().ends_with(".dff") {
                        let rx = viewer3d::spawn_dff_render_window(data, name.clone());
                        self.viewer_rxs.push(rx);
                    } else if name.to_lowercase().ends_with(".col") {
                        let rx = viewer3d::spawn_col_render_window(data, name.clone());
                        self.viewer_rxs.push(rx);
                    } else {
                        let game_root = archive_path.as_ref().and_then(|p| {
                            p.parent().and_then(|stream| stream.parent())
                        }).map(|p| p.to_path_buf());
                        let rx = viewer3d::spawn_render_window(data, name.clone(), game_root);
                        self.viewer_rxs.push(rx);
                    }

                    if let Some(archive) = self.editor.selected_archive_mut() {
                        archive.add_log(format!("Opening external 3D viewer for {name}"));
                    }
                    Task::none()
                }
            }},

            Message::ShowAbout => {
                self.show_about = true;
                Task::none()
            }
            Message::HideAbout => {
                self.show_about = false;
                Task::none()
            }
            Message::ShowWelcome => {
                self.show_welcome = true;
                Task::none()
            }
            Message::HideWelcome => {
                self.show_welcome = false;
                if self.welcome_persist {
                    self.config.first_run_complete = true;
                }
                self.save_config();
                Task::none()
            }
            Message::ToggleWelcomePersist(val) => {
                self.welcome_persist = val;
                Task::none()
            }
            Message::ToggleUpdateDisabled(val) => {
                self.config.update_check_enabled = !val;
                self.save_config();
                Task::none()
            }
            Message::ToggleUpdateNotifyDisabled(val) => {
                self.config.update_notify_disabled = val;
                self.save_config();
                Task::none()
            }
            Message::ShowUnsupported(path) => {
                self.show_unsupported = Some(path);
                Task::none()
            }
            Message::HideUnsupported => {
                self.show_unsupported = None;
                Task::none()
            }
            Message::VisitRepository => {
                App::visit_repository();
                Task::none()
            }
            Message::HideUpdateStatus => {
                self.show_update_status = None;
                Task::none()
            }

            Message::CheckUpdatesManual => {
                self.update_check_manual = true;
                self.update_state = UpdateState::Checking;
                let repo = UPDATER_REPO.to_string();
                let current = env!("CARGO_PKG_VERSION").to_string();
                Task::perform(check_updates_future(repo, current), Message::UpdateResultReceived)
            }
            Message::UpdateResultReceived(result) => {
                let was_manual = self.update_check_manual;
                self.update_check_manual = false;
                let suppressed = !was_manual && self.config.update_notify_disabled;
                match result {
                    UpdateResult::Available { version, url } => {
                        self.update_state = UpdateState::Available {
                            version: version.clone(),
                            url,
                        };
                        if !suppressed {
                            self.show_update_status = Some(format!("Update available: {version}"));
                        }
                    }
                    UpdateResult::UpToDate => {
                        self.update_state = UpdateState::UpToDate;
                        if !suppressed {
                            self.show_update_status = Some("You are using the latest version.".into());
                        }
                    }
                    UpdateResult::Error(err) => {
                        self.update_state = UpdateState::Error(err.clone());
                        if !suppressed {
                            self.show_update_status = Some(format!("Update check failed: {err}"));
                        }
                    }
                }
                Task::none()
            }

            Message::SetTheme(theme) => {
                self.config.theme = theme;
                self.save_config();
                Task::none()
            }
            Message::ToastTimeout => {
                self.toast = None;
                Task::none()
            }
            Message::TickProgress => {
                self.poll_viewer_rxs();
                // Animate the progress bar smoothly towards the current value.
                if let Some(archive_idx) = self.editor.selected_archive() {
                    let current_progress = self.editor.archives()[archive_idx].progress.percentage();
                    let visual = self.animator.get(ANIM_PROGRESS);
                    if (visual - current_progress).abs() > 0.005 {
                        self.animator.animate_from_current(
                            ANIM_PROGRESS,
                            current_progress,
                            Duration::from_millis(200),
                            crate::ui::easing::Easing::CubicOut,
                        );
                    }
                }
                // Toast pulse: a finite random number (3–6) of smooth
                // green → neutral → green cycles, then settle.
                let toast_active = self.toast.is_some();
                if toast_active && self.toast_pulses_remaining == 0 {
                    // Toast just appeared: start N random pulses (3–6).
                    let seed = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos();
                    let count = 3 + (seed % 4) as u32;
                    self.toast_pulses_remaining = count;
                    self.toast_pulse_target = 1.0;
                    self.animator.animate_from_current(
                        ANIM_TOAST_OPACITY,
                        self.toast_pulse_target,
                        Duration::from_millis(300),
                        crate::ui::easing::Easing::CubicOut,
                    );
                } else if toast_active && self.toast_pulses_remaining > 0 {
                    // A pulse finished: chain to the next if more remain.
                    if !self.animator.is_running(ANIM_TOAST_OPACITY) {
                        self.toast_pulses_remaining -= 1;
                        if self.toast_pulses_remaining > 0 {
                            // Toggle target: 1.0 → 0.0 → 1.0 → 0.0 → ...
                            self.toast_pulse_target = if self.toast_pulse_target > 0.5 { 0.0 } else { 1.0 };
                            self.animator.animate(
                                ANIM_TOAST_OPACITY,
                                self.animator.get(ANIM_TOAST_OPACITY),
                                self.toast_pulse_target,
                                Duration::from_millis(300),
                                crate::ui::easing::Easing::CubicOut,
                            );
                        } else {
                            // Final pulse: settle on green.
                            self.toast_pulse_target = 1.0;
                            self.animator.animate(
                                ANIM_TOAST_OPACITY,
                                self.animator.get(ANIM_TOAST_OPACITY),
                                1.0,
                                Duration::from_millis(300),
                                crate::ui::easing::Easing::CubicOut,
                            );
                        }
                    }
                } else if !toast_active && self.toast_pulses_remaining > 0 {
                    // Toast was cleared mid-pulse: fade back immediately.
                    self.toast_pulses_remaining = 0;
                    self.toast_pulse_target = 0.0;
                    self.animator.animate_from_current(
                        ANIM_TOAST_OPACITY,
                        0.0,
                        Duration::from_millis(200),
                        crate::ui::easing::Easing::CubicOut,
                    );
                }
                // Reap finished animations to keep the animator lean.
                self.animator.reap_finished();
                Task::none()
            }
            Message::AnimationTick(now) => {
                if let Some(prev) = self.prev_tick {
                    let dt = now.duration_since(prev);
                    self.animator.update(dt);
                }
                self.prev_tick = Some(now);

                // Auto-dismiss toasts after 2.5 seconds so the green status pulse
                // does not appear to stay on indefinitely.
                if self.toast.is_some() {
                    match self.toast_start {
                        None => self.toast_start = Some(now),
                        Some(start) => {
                            if now.duration_since(start) >= Duration::from_millis(2500) {
                                self.toast = None;
                                self.toast_start = None;
                            }
                        }
                    }
                } else {
                    self.toast_start = None;
                }

                Task::none()
            }
            Message::PaneResized(event) => {
                self.panes.resize(event.split, event.ratio);
                Task::none()
            }
            Message::ScrollOffsetChanged(y) => {
                self.scroll_y = y;
                Task::none()
            }
            Message::EntryInspected { index, inspection } => {
                if self.editor.selected_entry() == Some(index) {
                    self.inspected_entry = Some((index, inspection));
                }
                Task::none()
            }
            Message::HideContextMenu => {
                self.context_menu = None;
                Task::none()
            }
            Message::ModifiersChanged(mods) => {
                self.modifiers = mods;
                Task::none()
            }
            Message::AutoScrollStarted | Message::AutoScrollStartedAtRow(_) => {
                // Middle-clicking while the context menu is open just dismisses it.
                if self.context_menu.take().is_some() {
                    return Task::none();
                }
                self.autoscroll = Some(AutoScroll {
                    anchor: None,
                    initial_scroll_y: self.scroll_y,
                    current: None,
                });
                Task::none()
            }
            Message::AutoScrollMoved(position) => {
                let Some(state) = self.autoscroll.as_mut() else {
                    return Task::none();
                };
                if state.anchor.is_none() {
                    state.anchor = Some(position);
                    state.current = Some(position);
                    return Task::none();
                }
                state.current = Some(position);
                let anchor = state.anchor.unwrap_or(position);
                let delta_y = position.y - anchor.y;
                const SENSITIVITY: f32 = 2.5;
                let new_y = (state.initial_scroll_y + delta_y * SENSITIVITY).max(0.0);
                iced::advanced::widget::operate(scroll_to(
                    iced::widget::Id::new("entry_table"),
                    AbsoluteOffset { x: None, y: Some(new_y) },
                ))
            }
            Message::AutoScrollEnded => {
                self.autoscroll = None;
                Task::none()
            }
            Message::OpenLastExportFolder => {
                if let Some(index) = self.editor.selected_archive()
                    && let Some(archive) = self.editor.archives().get(index)
                    && let Some(folder) = archive.last_export_folder.clone()
                {
                    open_export_folder(&folder);
                }
                Task::none()
            }
            Message::SortBy(column) => {
                if let Some(archive) = self.editor.selected_archive_mut() {
                    let unique_types = archive.unique_file_types().to_vec();
                    match column {
                        SortColumn::Name => {
                            if archive.sort.column == SortColumn::Name {
                                archive.sort.direction = match archive.sort.direction {
                                    SortDirection::Ascending => SortDirection::Descending,
                                    SortDirection::Descending => SortDirection::Ascending,
                                };
                            } else {
                                archive.sort.column = SortColumn::Name;
                                archive.sort.direction = SortDirection::Ascending;
                            }
                        }
                        SortColumn::Type => {
                            if archive.sort.column == SortColumn::Type {
                                let count = unique_types.len().max(1);
                                archive.sort.type_index = (archive.sort.type_index + 1) % count;
                            } else {
                                archive.sort.column = SortColumn::Type;
                                archive.sort.type_index = 0;
                            }
                        }
                        SortColumn::Size => {
                            if archive.sort.column == SortColumn::Size {
                                archive.sort.direction = match archive.sort.direction {
                                    SortDirection::Ascending => SortDirection::Descending,
                                    SortDirection::Descending => SortDirection::Ascending,
                                };
                            } else {
                                archive.sort.column = SortColumn::Size;
                                archive.sort.direction = SortDirection::Descending;
                            }
                        }
                    }
                    let filter = self.search.clone();
                    archive.update_selected_list(&filter);
                    // Promote the current chain to the global default
                    // so the next archive opened inherits this sort.
                    // Cheap, since the chain is at most 10 priorities.
                    self.config.default_sort_chain = archive.sort_chain.clone();
                    self.save_config();
                }
                Task::none()
            }

            Message::FilesDropped(path) => {
                if path.extension().is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("img")
                }) {
                    if let Err(err) = self.editor.open_archive(&path) {
                        if matches!(err, crate::editor::OpenArchiveError::UnsupportedFormat) {
                            self.show_unsupported = Some(path);
                        } else {
                            self.toast = Some(format!("Failed to open archive: {err}"));
                        }
                    }
                } else if self.editor.selected_archive().is_some() {
                    self.toast = Some(format!("Imported {} dropped files.", 1));
                    if let Some((_index, _archive)) = self.editor.clone_selected_archive() {
                        self.editor.append_import(_index, vec![path], false);
                    }
                } else {
                    self.toast = Some("Open an archive first to drop non-IMG files into it.".into());
                }
                Task::none()
            }

            Message::TxdDecodeRequested => {
                let Some(entry_index) = self.editor.selected_entry() else {
                    return Task::none();
                };
                // Cache miss or first request: decode in the background.
                self.txd_selected_texture = 0;
                self.decode_txd(entry_index)
            }

            Message::TxdDecoded { index, result } => {
                match result {
                    Ok(textures) => {
                        if let Some(archive) = self.editor.selected_archive_mut() {
                            let count = textures.len();
                            archive.txd_cache.insert(index, textures);
                            archive.add_log(format!("Decoded {count} TXD texture(s)"));
                            self.toast = Some(format!("Decoded {count} texture(s)"));
                        }
                    }
                    Err(err) => {
                        self.toast = Some(err);
                    }
                }
                Task::none()
            }

            Message::TxdSelectTexture(index) => {
                self.txd_selected_texture = index;
                Task::none()
            }

            Message::TxdExportTextures => {
                dialogs::save_folder().map(Message::TxdExportFolderResult)
            }

            Message::TxdExportFolderResult(Some(folder)) => {
                let Some(archive_index) = self.editor.selected_archive() else {
                    return Task::none();
                };
                let Some(entry_index) = self.editor.selected_entry() else {
                    return Task::none();
                };
                let textures = self
                    .editor
                    .archives()
                    .get(archive_index)
                    .and_then(|a| a.txd_cache.get(&entry_index))
                    .cloned();
                let Some(textures) = textures else {
                    self.toast = Some("No decoded textures to export.".into());
                    return Task::none();
                };

                let folder = folder.clone();
                let count = textures.len();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || -> Result<(), String> {
                            for tex in &textures {
                                let safe_name: String = tex
                                    .name
                                    .chars()
                                    .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
                                    .collect();
                                let path = folder.join(format!("{}.tga", safe_name));
                                let mut tga = Vec::with_capacity(18 + tex.rgba.len());
                                tga.push(0);
                                tga.push(0);
                                tga.push(2);
                                tga.extend_from_slice(&[0, 0, 0, 0, 0]);
                                tga.extend_from_slice(&[0, 0]);
                                tga.extend_from_slice(&[0, 0]);
                                tga.extend_from_slice(&(tex.width as u16).to_le_bytes());
                                tga.extend_from_slice(&(tex.height as u16).to_le_bytes());
                                tga.push(32);
                                tga.push(0x20);
                                for chunk in tex.rgba.chunks_exact(4) {
                                    tga.push(chunk[2]);
                                    tga.push(chunk[1]);
                                    tga.push(chunk[0]);
                                    tga.push(chunk[3]);
                                }
                                std::fs::write(&path, tga)
                                    .map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
                            }
                            Ok(())
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("task panicked: {e}")))
                    },
                    move |result| {
                        if result.is_ok() {
                            let _ = format!("Exported {count} texture(s)");
                        }
                        Message::Noop
                    },
                )
            }

            Message::TxdExportFolderResult(None) => Task::none(),

            Message::ExportEmbeddedTexturesRequest { entry_index, nif_basename } => {
                let _ = entry_index;
                self.toast = Some(format!("Pick a folder to export embedded textures from {nif_basename}"));
                let nb = nif_basename.clone();
                dialogs::save_folder().map(move |folder| {
                    Message::ExportEmbeddedTexturesFolderResult {
                        entry_index,
                        nif_basename: nb.clone(),
                        folder,
                    }
                })
            }
            Message::ExportEmbeddedTexturesFolderResult { entry_index, nif_basename, folder: Some(folder) } => {
                let archive_path = self
                    .editor
                    .selected_archive()
                    .and_then(|i| self.editor.archives().get(i))
                    .and_then(|a| a.path.clone());
                let game_root = archive_path
                    .as_deref()
                    .and_then(|p| p.parent().and_then(|stream| stream.parent()))
                    .map(|p| p.to_path_buf());
                let Some(game_root) = game_root else {
                    self.toast = Some("Could not determine game root from archive path".to_string());
                    return Task::none();
                };
                let nb_for_callback = nif_basename.clone();
                Task::perform(
                    async move {
                        let ide_map = crate::inspector::texture::IdeMap::build(&game_root);
                        tokio::task::spawn_blocking(move || {
                            crate::inspector::texture_export::export_embedded_textures(
                                &nif_basename,
                                &ide_map,
                                &folder,
                            )
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("export task panicked: {e}")))
                    },
                    move |result| Message::ExportEmbeddedTexturesCompleted {
                        entry_index,
                        nif_basename: nb_for_callback.clone(),
                        result,
                    },
                )
            }
            Message::ExportEmbeddedTexturesFolderResult { folder: None, .. } => Task::none(),
            Message::ExportEmbeddedTexturesCompleted { entry_index, nif_basename, result } => {
                let _ = entry_index;
                let now = chrono::Local::now().format("%H:%M:%S");
                let archive_index = self.editor.selected_archive().unwrap_or(0);
                if let Some(archive) = self.editor.archives_mut().get_mut(archive_index) {
                    match &result {
                        Ok(report) => {
                            let line = format!("[{}] {}: {}", now, nif_basename, report.summary());
                            archive.recent_exports.push(line.clone());
                            archive.add_log(line);
                            self.toast = Some(report.summary());
                        }
                        Err(err) => {
                            let line = format!("[{}] {} export failed: {err}", now, nif_basename);
                            archive.add_log(line.clone());
                            self.toast = Some(line);
                        }
                    }
                }
                Task::none()
            }
            Message::Viewer3dRequestLoad {
                archive_index,
                entry_index,
            } => {
                let (entry_clone, archive_path) = {
                    let Some(archive) = self.editor.archives().get(archive_index) else {
                        return Task::none();
                    };
                    let Some(entry) = archive.entries.get(entry_index) else {
                        return Task::none();
                    };
                    (entry.clone(), archive.path.clone())
                };
                let nif_basename = std::path::Path::new(&entry_clone.file_name)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| entry_clone.file_name.to_string());
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let bytes = crate::parser::read_entry_data_from_source(
                                &entry_clone,
                                archive_path.as_deref(),
                            )
                            .map_err(|e| format!("I/O: {e}"))?;
                            let game_root = archive_path
                                .as_deref()
                                .and_then(|p| p.parent().and_then(|stream| stream.parent()))
                                .map(|p| p.to_path_buf());
                            let ide_map = game_root
                                .as_ref()
                                .map(|root| crate::inspector::texture::IdeMap::build(root));
                            let nft_catalog = ide_map
                                .as_ref()
                                .and_then(|map| {
                                    crate::inspector::texture::resolve_textures_for_nif(
                                        &nif_basename,
                                        map,
                                    )
                                });
                            let resolver = move |name: &str| {
                                nft_catalog
                                    .as_ref()
                                    .and_then(|cat| cat.get_pixels(name))
                                    .and_then(SceneTexture::from_tga)
                            };
                            let base = crate::inspector::scene3d::camera::BaseOrientation::Zup;
                            let scene = crate::inspector::scene3d::decode::parse_and_build_scene(
                                &bytes,
                                base,
                                resolver,
                            )
                            .map_err(|e| format!("scene: {e:?}"))?;
                            Ok::<_, String>(scene)
                        })
                        .await
                        .map_err(|e| format!("join: {e}"))?
                    },
                    move |result| Message::Viewer3dLoadCompleted {
                        archive_index,
                        entry_index,
                        result,
                    },
                )
            }
            Message::Viewer3dLoadCompleted {
                archive_index,
                entry_index,
                result,
            } => {
                let _ = (archive_index, entry_index);
                    match result {
                        Ok(scene) => {
                            dev_logger::breadcrumb(&format!(
                                "3D load ok: {} verts, {} tris",
                                scene.total_vertices(),
                                scene.total_triangles()
                            ));
                            self.viewer3d_handle.set_scene(scene);
                            self.selected_inspector_tab = InspectorTab::Model3D;
                            if let Some(archive) = self.editor.selected_archive_mut() {
                                archive.add_log("In-app 3D viewer ready".to_string());
                            }
                        }
                        Err(e) => {
                            dev_logger::breadcrumb(&format!("3D load failed: {e}"));
                            self.toast = Some(format!("3D load failed: {e}"));
                        }
                    }
                Task::none()
            }
            Message::Viewer3dSelectTab(tab) => {
                self.selected_inspector_tab = tab;
                Task::none()
            }
            Message::Viewer3dClear => {
                self.viewer3d_handle.clear();
                Task::none()
            }
            Message::Viewer3dReset => {
                self.viewer3d_handle.reset_camera();
                Task::none()
            }
            Message::Viewer3dToggleWireframe => {
                self.viewer3d_handle.toggle_wireframe();
                Task::none()
            }
            Message::Viewer3dToggleCullBackfaces => {
                self.viewer3d_handle.toggle_cull_back();
                Task::none()
            }
            Message::Viewer3dToggleTextured => {
                self.viewer3d_handle.toggle_textured();
                Task::none()
            }

            // ---- Sort Manager dialog ----
            Message::OpenSortManager => {
                // Seed the draft from the active archive's chain. If
                // no archive is open, seed from the global default.
                // The `take` would be tempting but the dialog is the
                // only place the draft lives, so we just overwrite.
                let draft = self
                    .editor
                    .selected_archive()
                    .and_then(|i| self.editor.archives().get(i))
                    .map(|a| a.sort_chain.clone())
                    .unwrap_or_else(|| self.config.default_sort_chain.clone());
                self.sort_draft = Some(draft);
                self.show_sort_manager = true;
                Task::none()
            }
            Message::CloseSortManager => {
                // Discard the draft on close. "Apply" was the only
                // path that committed changes; everything else just
                // leaves the live archive alone.
                self.sort_draft = None;
                self.show_sort_manager = false;
                Task::none()
            }
            Message::SortResetDraft => {
                // Re-seed the draft from the active archive so the
                // user can undo in-progress edits without closing the
                // dialog.
                if let Some(draft) = self.sort_draft.as_mut() {
                    if let Some(i) = self.editor.selected_archive() {
                        if let Some(a) = self.editor.archives().get(i) {
                            *draft = a.sort_chain.clone();
                        }
                    }
                }
                Task::none()
            }
            Message::SortApplyDraft => {
                // Commit the draft to the live archive, the editor's
                // default chain (so new archives inherit), and the
                // persisted config (so the change survives a restart).
                if let Some(draft) = self.sort_draft.take() {
                    if let Some(i) = self.editor.selected_archive() {
                        if let Some(a) = self.editor.archives_mut().get_mut(i) {
                            a.sort_chain = draft.clone();
                            let filter = self.search.clone();
                            a.update_selected_list(&filter);
                        }
                    }
                    self.config.default_sort_chain = draft.clone();
                    self.editor.set_default_sort_chain(draft);
                    self.save_config();
                }
                self.show_sort_manager = false;
                Task::none()
            }
            Message::SortAddSlot => {
                if let Some(draft) = self.sort_draft.as_mut() {
                    // Add a disabled Size ASC slot as a neutral
                    // default — the user almost always wants to
                    // change the key after adding.
                    draft.push(crate::sort::SortPriority::new(
                        crate::sort::SortKey::Size,
                        crate::sort::SortDirection::Ascending,
                    ));
                }
                Task::none()
            }
            Message::SortRemoveSlot(SortSlotIndex(index)) => {
                if let Some(draft) = self.sort_draft.as_mut() {
                    draft.remove(index);
                }
                Task::none()
            }
            Message::SortMoveSlotUp(SortSlotIndex(index)) => {
                if let Some(draft) = self.sort_draft.as_mut()
                    && index > 0
                {
                    draft.move_slot(index, index - 1);
                }
                Task::none()
            }
            Message::SortMoveSlotDown(SortSlotIndex(index)) => {
                if let Some(draft) = self.sort_draft.as_mut() {
                    draft.move_slot(index, index + 1);
                }
                Task::none()
            }
            Message::SortToggleSlotEnabled(SortSlotIndex(index)) => {
                if let Some(draft) = self.sort_draft.as_mut() {
                    draft.toggle_enabled(index);
                }
                Task::none()
            }
            Message::SortSetSlotKey(SortSlotIndex(index), key) => {
                if let Some(draft) = self.sort_draft.as_mut() {
                    draft.set_key(index, key);
                }
                Task::none()
            }
            Message::SortSetSlotDirection(SortSlotIndex(index), direction) => {
                if let Some(draft) = self.sort_draft.as_mut() {
                    draft.set_direction(index, direction);
                }
                Task::none()
            }
            Message::SortSelectPreset(preset) => {
                // Built-in presets — each is a one-line chain shape
                // the user can further edit. Direction is encoded in
                // the variant so a single call site handles them all.
                if let Some(draft) = self.sort_draft.as_mut() {
                    *draft = preset.to_chain();
                }
                Task::none()
            }

            // ---- Drag-and-drop between archives ----
            // Rust-flavored approach (vs IMGF's MFC OLE):
            //   - The whole drag state lives in `App::drag_state` as
            //     a Copy value, so the borrow checker enforces
            //     consistent update with no heap pointers.
            //   - We never copy entry data on press — the source
            //     archive stays open in the Editor, and the move
            //     handler reads + re-imports through the standard
            //     parser pipeline.
            //   - The release handler either commits (target != source)
            //     or simply drops the state. No try/finally needed
            //     for cleanup; `Option::take` is the cleanup.
            Message::ArchiveDragStarted { source } => {
                if let Some(archive) = self.editor.archives().get(source) {
                    let selected: Vec<usize> = archive
                        .selected_indices
                        .iter()
                        .copied()
                        .filter(|&i| i < archive.entries.len())
                        .collect();
                    if !selected.is_empty() {
                        self.drag_state =
                            Some(crate::ui::drag::DragState::new(source, &selected));
                    }
                }
                Task::none()
            }
            Message::ArchiveDragMoved { over } => {
                if let Some(state) = self.drag_state.as_mut() {
                    state.hover_target = over;
                }
                Task::none()
            }
            Message::ArchiveDragReleased => {
                // Take the state so cancel-on-anything-else is just
                // `self.drag_state = None` (no drop glue needed).
                let Some(state) = self.drag_state.take() else {
                    return Task::none();
                };
                if !state.has_valid_target() {
                    // Drop on the source or empty space = cancel.
                    self.toast = Some("Drag cancelled".to_string());
                    return Task::none();
                }
                let Some(target) = state.hover_target else {
                    return Task::none();
                };
                self.move_entries_between_archives(state.source, target, state.indices());
                Task::none()
            }
            Message::ArchiveDragCancelled => {
                self.drag_state = None;
                self.toast = Some("Drag cancelled".to_string());
                Task::none()
            }
        }
    }

    /// Move a slice of entries from `source` to `target` archive.
    /// The entries are cloned (their data + flags) so the move is
    /// in-memory and reversible; we don't touch the disk. The
    /// source entries are removed by index, which avoids issues
    /// with renamed/removed indices after a move.
    fn move_entries_between_archives(
        &mut self,
        source: usize,
        target: usize,
        entry_indices: &[usize],
    ) {
        // We need to collect the entries first because we'd
        // otherwise borrow the source archive mutably while also
        // needing to mutate the target archive. Two-phase move
        // avoids the borrow conflict.
        let entries: Vec<crate::archive::EntryInfo> = {
            let Some(archive) = self.editor.archives().get(source) else {
                return;
            };
            entry_indices
                .iter()
                .filter_map(|&i| archive.entries.get(i).cloned())
                .collect()
        };

        // Insert into the target archive. If the target already
        // has an entry with the same name, we rename the moved
        // copy to avoid silent overwrites. IMGF's drag-and-drop
        // does the same on conflict — we get it for free here.
        if let Some(target_archive) = self.editor.archives_mut().get_mut(target) {
            for mut entry in entries {
                if target_archive
                    .entries
                    .iter()
                    .any(|e| e.file_name == entry.file_name)
                {
                    // Append ".bak" to disambiguate. We could
                    // prompt the user, but the IMGF behaviour
                    // is "just do it", so we follow that lead.
                    entry.file_name = compact_str::CompactString::from(
                        format!("{}.bak", entry.file_name),
                    );
                    entry.file_name_lower = compact_str::CompactString::from(
                        entry.file_name.to_ascii_lowercase(),
                    );
                }
                target_archive.entries.push(entry);
            }
        }

        // Remove from the source. We do this in reverse index order
        // so earlier removals don't shift the indices of later
        // removals. This is the standard "delete in reverse" idiom
        // for indexed removal.
        if let Some(source_archive) = self.editor.archives_mut().get_mut(source) {
            let mut indices: Vec<usize> = entry_indices.to_vec();
            indices.sort_unstable();
            indices.reverse();
            indices.retain(|&i| i < source_archive.entries.len());
            indices.dedup();
            let mut shift = 0u32;
            for &i in &indices {
                let actual = i - shift as usize;
                source_archive.entries.remove(actual);
                source_archive.selected_indices.retain(|&mut j| j != i);
                source_archive.selected_lookup.remove(&i);
                shift += 1;
            }
            source_archive.dirty = true;
        }
        self.toast = Some(format!(
            "Moved {} entries to archive #{}",
            entry_indices.len(),
            target + 1
        ));
    }

    fn decode_txd(&self, entry_index: usize) -> Task<Message> {
        let Some(archive_index) = self.editor.selected_archive() else {
            return Task::none();
        };
        let (entry_clone, archive_path) = {
            let Some(archive) = self.editor.archives().get(archive_index) else {
                return Task::none();
            };
            let Some(entry) = archive.entries.get(entry_index) else {
                return Task::none();
            };
            (entry.clone(), archive.path.clone())
        };

        Task::perform(
            async move {
                let result = tokio::task::spawn_blocking(move || -> Result<Vec<DecodedTexture>, String> {
                    let data = crate::parser::read_entry_data_from_source(
                        &entry_clone,
                        archive_path.as_deref(),
                    ).map_err(|e| format!("Failed to read entry: {e}"))?;
                    let txd = crate::parser::txd::parse_txd(&data)
                        .map_err(|e| format!("TXD parse failed: {e}"))?;

                    let mut decoded = Vec::new();
                    for tex in &txd.textures {
                        let rgba = tex.decode_rgba().map_err(|e| format!("Texture decode failed: {e}"))?;
                        decoded.push(DecodedTexture {
                            name: tex.diffuse_name.clone(),
                            width: tex.width,
                            height: tex.height,
                            rgba,
                            has_alpha: tex.has_alpha != 0 || tex.raster_format != 0x200,
                            format_name: tex.format_name().to_string(),
                            mipmap_count: tex.num_mipmaps as u32,
                            handle: std::sync::OnceLock::new(),
                        });
                    }
                    Ok(decoded)
                })
                .await;

                result.unwrap_or_else(|e| Err(format!("task panicked: {e}")))
            },
            move |result| Message::TxdDecoded {
                index: entry_index,
                result,
            },
        )
    }

    fn start_export(&mut self, mode: ExportMode) -> Task<Message> {
        self.last_export_selected_only = matches!(mode, ExportMode::Selected);
        self.toast = None;
        dialogs::save_folder().map(Message::ExportFolderResult)
    }

    fn poll_viewer_rxs(&mut self) {
        let mut logs: Vec<String> = Vec::new();
        let mut toast: Option<String> = None;
        self.viewer_rxs.retain_mut(|rx| loop {
            match rx.try_recv() {
                Ok(ViewerEvent::Opened { name }) => {
                    logs.push(format!("3D viewer opened: {name}"));
                }
                Ok(ViewerEvent::Failed { reason }) => {
                    toast = Some(reason.clone());
                    logs.push(format!("3D viewer failed: {reason}"));
                }
                Ok(ViewerEvent::Closed) => {
                    logs.push("3D viewer closed".to_string());
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => return false,
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break true,
            }
        });
        if let Some(msg) = toast {
            self.toast = Some(msg);
        }
        if let Some(archive) = self.editor.selected_archive_mut() {
            for log in logs {
                archive.add_log(log);
            }
        }
    }
}

impl App {
    pub fn subscription(&self) -> Subscription<Message> {
        // Track modifier keys from ALL keyboard events (press + release).
        let mod_tracker = iced::event::listen().map(|event| match event {
            iced::Event::Keyboard(ke) => match ke {
                KeyboardEvent::KeyPressed { modifiers, .. }
                | KeyboardEvent::KeyReleased { modifiers, .. }
                | KeyboardEvent::ModifiersChanged(modifiers) => {
                    Message::ModifiersChanged(modifiers)
                }
            },
            _ => Message::Noop,
        });

        let key = iced::keyboard::listen().map(|event| match event {
            KeyboardEvent::KeyPressed {
                physical_key,
                modifiers,
                ..
            } => detect_pressed(physical_key, modifiers)
                .map(Message::ShortcutPressed)
                .unwrap_or(Message::Noop),
            _ => Message::Noop,
        });

        let tick = iced::time::every(Duration::from_millis(250)).map(|_| Message::TickProgress);

        // Only run the animation ticker when something needs it. A constant
        // 60 Hz update forces a full view rebuild every frame, which makes
        // scrolling and typing feel sluggish on large archives.
        let anim_tick = if self.animator.running_count() > 0 || self.toast.is_some() {
            iced::time::every(Duration::from_millis(16)).map(Message::AnimationTick)
        } else {
            Subscription::none()
        };

        let debounce = iced::time::every(Duration::from_millis(150)).map(|_| Message::DebounceTick);

        let window = iced::window::events().map(|(_id, event)| match event {
            iced::window::Event::FileDropped(path) => Message::FilesDropped(path),
            _ => Message::Noop,
        });

        let autoscroll = if self.autoscroll.is_some() {
            iced::event::listen().map(|event| match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Message::AutoScrollMoved(position)
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(_)) => {
                    Message::AutoScrollEnded
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                    iced::mouse::Button::Left | iced::mouse::Button::Right,
                )) => Message::AutoScrollEnded,
                _ => Message::Noop,
            })
        } else {
            Subscription::none()
        };

        Subscription::batch([mod_tracker, key, tick, anim_tick, debounce, window, autoscroll])
    }
}

impl App {
    pub fn view(&self) -> Element<'_, Message> {
        crate::ui::view::build(self)
    }

    pub fn menubar(&self) -> Element<'_, Message> {
        // The "Open Recent" submenu is built from `iter_existing` so
        // dead links vanish without mutating the stored MRU list.
        // An empty list renders a single disabled "No recent files"
        // item so the user can see why the menu is empty.
        let recent_menu_items: Vec<Item<'_, Message, _, _>> = if self
            .config
            .recent_files
            .iter_existing()
            .next()
            .is_none()
        {
            vec![Item::new(iced::Element::from(
                iced::widget::text("No recent files").size(13),
            ))]
        } else {
            self.config
                .recent_files
                .iter()
                .map(|(index, entry)| {
                    let label = self
                        .config
                        .recent_files
                        .menu_label(index, 60);
                    let path = entry.path.clone();
                    Item::new(menu_button(label, Message::OpenRecent(path)))
                })
                .collect()
        };
        let recent_menu = Menu::new(recent_menu_items).max_width(320.0);

        let file_menu = Menu::new(vec![
            Item::new(menu_button(
                format!("New ({})", shortcut_display(Shortcut::New)),
                Message::NewArchive,
            )),
            Item::new(menu_button(
                format!("Open… ({})", shortcut_display(Shortcut::Open)),
                Message::OpenArchive,
            )),
            Item::with_menu(
                menu_button("Open Recent".to_string(), Message::Noop),
                recent_menu,
            ),
            Item::new(menu_button(
                format!("Save ({})", shortcut_display(Shortcut::Save)),
                Message::SaveArchive,
            )),
            Item::new(menu_button(
                format!("Save as… ({})", shortcut_display(Shortcut::SaveAs)),
                Message::SaveArchiveAs,
            )),
            Item::new(menu_button(
                format!("Close tab ({})", shortcut_display(Shortcut::Close)),
                Message::CloseSelectedArchive,
            )),
            Item::new(menu_button(
                "Sort by…".to_string(),
                Message::OpenSortManager,
            )),
        ])
        .max_width(220.0);

        let edit_menu = Menu::new(vec![
            Item::new(menu_button(
                format!("Import ({})", shortcut_display(Shortcut::Import)),
                Message::ImportFiles,
            )),
            Item::new(menu_button(
                format!("Export all ({})", shortcut_display(Shortcut::ExportAll)),
                Message::ExportAll,
            )),
            Item::new(menu_button(
                format!(
                    "Export selected ({})",
                    shortcut_display(Shortcut::ExportSelected)
                ),
                Message::ExportSelected,
            )),
        ])
        .max_width(220.0);

        let selection_menu = Menu::new(vec![
            Item::new(menu_button(
                format!("Select all ({})", shortcut_display(Shortcut::SelectAll)),
                Message::SelectAll,
            )),
            Item::new(menu_button(
                format!(
                    "Invert selection ({})",
                    shortcut_display(Shortcut::InvertSelection)
                ),
                Message::InvertSelection,
            )),
            Item::new(menu_button(
                format!(
                    "Delete selected ({})",
                    shortcut_display(Shortcut::Delete)
                ),
                Message::DeleteSelected,
            )),
        ])
        .max_width(220.0);

        let option_items: Vec<Item<'_, Message, iced::Theme, iced::Renderer>> = ThemeMode::ALL
            .iter()
            .map(|mode| {
                let label = if *mode == self.config.theme {
                    format!("● {}", mode.as_str())
                } else {
                    format!("○ {}", mode.as_str())
                };
                Item::new(menu_button(label, Message::SetTheme(*mode)))
            })
            .collect();

        let option_menu = Menu::new(option_items)
            .max_width(220.0);

        let help_menu = Menu::new(vec![
            Item::new(menu_button(
                format!(
                    "Check for updates ({})\u{200B}",
                    shortcut_display(Shortcut::CheckUpdates)
                ),
                Message::CheckUpdatesManual,
            )),
            Item::new(menu_button(
                "Visit repository\u{200B}".to_string(),
                Message::VisitRepository,
            )),
            Item::new(menu_button("About".to_string(), Message::ShowAbout)),
        ])
        .max_width(220.0);

        fn menu_label(label: &'static str) -> iced::Element<'static, Message> {
            container(fonts::header(label))
                .padding([4, 12])
                .into()
        }

        let bar = MenuBar::new(vec![
            Item::with_menu(menu_label("File"), file_menu),
            Item::with_menu(menu_label("Edit"), edit_menu),
            Item::with_menu(menu_label("Selection"), selection_menu),
            Item::with_menu(menu_label("Themes"), option_menu),
            Item::with_menu(menu_label("Help"), help_menu),
        ]);

        let design = self.design();
        let (top, bottom) = design.menubar_gradient();
        let border = design.border();
        iced::widget::Container::new(bar)
            .width(iced::Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Gradient(
                    iced::Gradient::Linear(
                        iced::gradient::Linear::new(0.0)
                            .add_stop(0.0, top)
                            .add_stop(1.0, bottom)
                    )
                )),
                border: iced::Border {
                    color: border,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .into()
    }
}

fn open_export_folder(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

fn menu_button<'a>(label: String, message: Message) -> Element<'a, Message> {
    iced::widget::button(
        fonts::body(label)
            .align_x(iced::alignment::Horizontal::Left)
            .width(iced::Length::Fill),
    )
    .on_press(message)
    .width(iced::Length::Fill)
    .style(|theme: &iced::Theme, status: iced::widget::button::Status| iced::widget::button::Style {
            background: if matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            ) {
                Some(theme.extended_palette().background.strong.color.into())
            } else {
                None
            },
            text_color: theme.extended_palette().background.base.text,
            ..iced::widget::button::Style::default()
        })
        .into()
}

pub fn run_app(config: Config) -> iced::Result {
    let size: iced::Size = config
        .window
        .size
        .unwrap_or([1100.0, 720.0])
        .into();

    let boot_config = Arc::new(config);
    let boot_config_for_boot = Arc::clone(&boot_config);

    iced::application(
        move || {
            let cfg = (*boot_config_for_boot).clone();
            (App::new(cfg.clone()), App::startup_task(&cfg))
        },
        App::update,
        App::view,
    )
    .title(|_: &App| "IMG Editor Plus".to_string())
    .theme(|state: &App| -> Option<Theme> { Some(state.theme()) })
    .subscription(App::subscription)
    .settings(iced::Settings {
        default_text_size: iced::Pixels(14.0),
        fonts: vec![
            crate::ui::fonts::INTER_FONT_BYTES.into(),
            crate::ui::fonts::BRICOLAGE_DISPLAY_FONT_BYTES.into(),
        ],
        ..iced::Settings::default()
    })
    .window(iced::window::Settings {
        icon: window_icon(),
        ..iced::window::Settings::default()
    })
    .default_font(crate::ui::fonts::INTER)
    .window_size(size)
    .resizable(true)
    .centered()
    .run()
}

fn window_icon() -> Option<iced::window::Icon> {
    let bytes = include_bytes!("../../asset/logo/IMGEditorLogo.png");
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?;
    let image = image.to_rgba8();
    let (width, height) = image.dimensions();
    iced::window::icon::from_rgba(image.into_raw(), width, height).ok()
}

#[allow(dead_code)]
fn _force_space_use(_: Space) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        App::new(Config::default())
    }

    #[test]
    fn search_changed_updates_text_immediately() {
        let mut app = test_app();
        let _ = app.update(Message::SearchChanged("player".to_string()));
        assert_eq!(app.search, "player");
        assert!(app.filter_pending);
    }

    #[test]
    fn search_changed_no_op_for_same_value() {
        let mut app = test_app();
        app.search = "player".to_string();
        app.filter_pending = false;
        let _ = app.update(Message::SearchChanged("player".to_string()));
        assert!(!app.filter_pending);
    }

    #[test]
    fn debounce_tick_applies_pending_filter() {
        let mut app = test_app();
        app.editor.new_archive();
        app.search = "player".to_string();
        app.filter_pending = true;

        let _ = app.update(Message::DebounceTick);

        assert!(!app.filter_pending);
    }

    #[test]
    fn rename_input_updates_buffer_without_committing() {
        let mut app = test_app();
        let _ = app.update(Message::RenameInputChanged("player".to_string()));
        assert_eq!(app.rename_buffer, "player");
    }

    #[test]
    fn commit_rename_uses_buffer_and_clears_it() {
        let mut app = test_app();
        app.rename_buffer = "renamed".to_string();

        let _ = app.update(Message::CommitRename);

        assert!(app.rename_buffer.is_empty());
    }
}
