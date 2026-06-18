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

use crate::archive::{ArchiveInfo, EntryInfo, ExportStatus, SortColumn, SortDirection};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{Config, ThemeMode};
use crate::editor::Editor;
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

#[derive(Debug, Clone)]
pub enum Message {
    Noop,
    ShortcutPressed(Shortcut),

    NewArchive,
    OpenArchive,
    OpenArchiveResult(Option<PathBuf>),
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
    CommitRename(String),
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryAction {
    CopyName,
    Rename,
    Delete,
    Export,
    Render,
    ViewTextures,
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
    pub last_export_selected_only: bool,
    pub fast_export: bool,
    pub panes: pane_grid::State<Pane>,
    pub context_menu: Option<(usize, usize)>,
    pub inspected_entry: Option<(usize, EntryInspection)>,
    /// Index into the decoded TXD textures currently being viewed.
    pub txd_selected_texture: usize,
    pub scroll_y: f32,
    pub search_pending: Option<String>,
    pub autoscroll: Option<AutoScroll>,
    pub modifiers: Modifiers,
    pub entry_table_id: iced::widget::Id,
    viewer_rxs: Vec<tokio::sync::mpsc::UnboundedReceiver<ViewerEvent>>,
    pub animator: Animator,
    prev_tick: Option<std::time::Instant>,
    toast_pulses_remaining: u32,
    toast_pulse_target: f32,
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
            last_export_selected_only: false,
            fast_export,
            panes,
            context_menu: None,
            inspected_entry: None,
            txd_selected_texture: 0,
            scroll_y: 0.0,
            search_pending: None,
            autoscroll: None,
            modifiers: Modifiers::default(),
            entry_table_id: iced::widget::Id::unique(),
            viewer_rxs: Vec::new(),
            animator: Animator::new(),
            prev_tick: None,
            toast_pulses_remaining: 0,
            toast_pulse_target: 0.0,
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
            match entry {
                Some(entry) => Some(Miss {
                    entry,
                    archive_path,
                    mmap,
                    archive_file_name,
                }),
                None => None,
            }
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
                if let Err(err) = self.editor.open_archive(&path) {
                    if matches!(err, crate::editor::OpenArchiveError::UnsupportedFormat) {
                        self.show_unsupported = Some(path);
                    } else {
                        self.toast = Some(format!("Failed to open archive: {err}"));
                    }
                }
                Task::none()
            }
            Message::OpenArchiveResult(None) => Task::none(),

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
                    {
                        if let Some(entry) = archive.entries.get(index) {
                                    self.rename_buffer = entry.file_name.to_string();
                        }
                    }
                    if let Some(archive) = self.editor.selected_archive_mut() {
                        if let Some(entry) = archive.entries.get_mut(index) {
                            entry.rename = true;
                        }
                    }
                }
                Task::none()
            }
            Message::CommitRename(new_name) => {
                self.editor.rename_selected(&new_name);
                self.rename_buffer.clear();
                Task::none()
            }
            Message::CancelRename => {
                if let Some(archive) = self.editor.selected_archive_mut() {
                    for entry in &mut archive.entries {
                        entry.rename = false;
                    }
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
                self.search_pending = Some(value);
                Task::none()
            }
            Message::DebounceTick => {
                if let Some(query) = self.search_pending.take() {
                    if query != self.search {
                        self.search = query;
                        return self.run_refresh_filter();
                    }
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
                        if let Some(entry) = archive.entries.get_mut(entry_index) {
                            entry.rename = true;
                            self.rename_buffer = entry.file_name.to_string();
                        }
                        archive.rebuild_row_cache();
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
                    if let Some(archive_index) = self.editor.selected_archive() {
                        if let Some(entry_index) = self.editor.selected_entry() {
                            if let Some(archive) = self.editor.archives().get(archive_index) {
                                if let Some(entry) = archive.entries.get(entry_index) {
                                    let name = entry.file_name.to_string();
                                    self.toast = Some(format!("Copied name: {}", name));
                                    return iced::clipboard::write::<Message>(name);
                                }
                            }
                        }
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
                EntryAction::Render => {
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
                        archive.add_log(format!("Opening 3D viewer for {name}"));
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
                    self.entry_table_id.clone(),
                    AbsoluteOffset { x: None, y: Some(new_y) },
                ))
            }
            Message::AutoScrollEnded => {
                self.autoscroll = None;
                Task::none()
            }
            Message::OpenLastExportFolder => {
                if let Some(index) = self.editor.selected_archive() {
                    if let Some(archive) = self.editor.archives().get(index) {
                        if let Some(folder) = archive.last_export_folder.clone() {
                            open_export_folder(&folder);
                        }
                    }
                }
                Task::none()
            }
            Message::SortBy(column) => {
                if let Some(archive) = self.editor.selected_archive_mut() {
                    let unique_types = archive.unique_file_types();
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
        }
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

        let anim_tick = iced::time::every(Duration::from_millis(16)).map(Message::AnimationTick);

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
        let file_menu = Menu::new(vec![
            Item::new(menu_button(
                format!("New ({})", shortcut_display(Shortcut::New)),
                Message::NewArchive,
            )),
            Item::new(menu_button(
                format!("Open… ({})", shortcut_display(Shortcut::Open)),
                Message::OpenArchive,
            )),
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
