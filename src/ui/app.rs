use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iced::keyboard::Event as KeyboardEvent;
use iced::widget::{Space, container, pane_grid};
use iced::{Element, Subscription, Task, Theme};
use iced_fonts::LUCIDE_FONT_BYTES;
use iced_aw::menu::{Item, Menu, MenuBar};

use crate::archive::ArchiveInfo;
use crate::config::{Config, ThemeMode};
use crate::editor::Editor;
use crate::parser::ImgVersion;
use crate::tasks::{ExportMode, ExportTask, SaveTask};
use crate::ui::dialogs::{self, SaveArchiveChoice};
use crate::ui::keymap::{Shortcut, detect_pressed, shortcut_display};
use crate::ui::theme::resolve_theme;
use crate::updater::{UpdateResult, UpdateState, check_updates_future};

const REPO_URL: &str = "https://github.com/CloudyTabzy/IMGEditor-rs";
const UPDATER_REPO: &str = "CloudyTabzy/IMGEditor-rs";

pub const ABOUT_TEXT: &str = concat!(
    "Grinch_'s IMG Editor v",
    env!("CARGO_PKG_VERSION"),
    "\n\nA pure Rust desktop editor for GTA IMG archives.\n\n",
    "Supported formats:\n",
    "- GTA III\n",
    "- GTA Vice City\n",
    "- GTA San Andreas\n",
    "- Bully Scholarship Edition"
);

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
        result: Result<usize, String>,
    },

    SelectAll,
    InvertSelection,
    DeleteSelected,
    StartRename,
    CommitRename(String),
    CancelRename,
    CancelActive,

    SearchChanged(String),
    RefreshFilter,

    EntryClicked(usize),
    EntryDoubleClicked(usize),
    EntryRightClicked(usize),
    EntryContextAction(EntryAction),

    ShowAbout,
    HideAbout,
    ShowWelcome,
    HideWelcome,
    ShowUnsupported(PathBuf),
    HideUnsupported,
    VisitRepository,
    HideUpdateStatus,

    CheckUpdatesManual,
    UpdateResultReceived(UpdateResult),

    SetTheme(ThemeMode),
    TickProgress,
    PaneResized(pane_grid::ResizeEvent),

    FilesDropped(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryAction {
    CopyName,
    Rename,
    Delete,
    Export,
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
    pub show_unsupported: Option<PathBuf>,
    pub show_update_status: Option<String>,
    pub update_state: UpdateState,
    pub update_check_manual: bool,
    pub toast: Option<String>,
    pub last_export_selected_only: bool,
    pub panes: pane_grid::State<Pane>,
}

impl Default for App {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

impl App {
    pub fn new(config: Config) -> Self {
        let show_welcome = !config.first_run_complete;
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
            show_unsupported: None,
            show_update_status: None,
            update_state: UpdateState::Idle,
            update_check_manual: false,
            toast: None,
            last_export_selected_only: false,
            panes,
        }
    }

    pub fn theme(&self) -> Theme {
        resolve_theme(self.config.theme)
    }

    pub fn startup_task() -> Task<Message> {
        Task::batch(vec![
            iced::font::load(LUCIDE_FONT_BYTES).map(|_| Message::Noop),
            Task::perform(
                check_updates_future(
                    UPDATER_REPO.to_string(),
                    env!("CARGO_PKG_VERSION").to_string(),
                ),
                Message::UpdateResultReceived,
            ),
        ])
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
                Task::none()
            }
            Message::CloseArchiveTab(index) => {
                self.editor.close_archive(index);
                Task::none()
            }
            Message::SelectArchiveTab(index) => {
                self.editor.select_archive(index);
                Task::none()
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
                let task = ExportTask::new(archive, folder, mode);
                Task::perform(
                    async move { task.run().await.map_err(|e| e.to_string()) },
                    move |result| Message::ExportCompleted { index, result },
                )
            }
            Message::ExportFolderResult(None) => Task::none(),

            Message::ExportCompleted { index, result } => {
                self.editor
                    .add_log_to(index, format!("Exported {} entries", result.as_ref().unwrap_or(&0)));
                match result {
                    Ok(count) => {
                        self.toast = Some(format!("Exported {count} entries."));
                    }
                    Err(err) => {
                        self.toast = Some(format!("Export failed: {err}"));
                    }
                }
                Task::none()
            }

            Message::SelectAll => {
                self.editor.select_all(true);
                Task::none()
            }
            Message::InvertSelection => {
                self.editor.invert_selection();
                Task::none()
            }
            Message::DeleteSelected => {
                self.editor.delete_selected();
                Task::none()
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
                self.search = value;
                self.editor.update_filtered_list(&self.search);
                Task::none()
            }
            Message::RefreshFilter => {
                self.editor.update_filtered_list(&self.search);
                Task::none()
            }

            Message::EntryClicked(index) => {
                self.editor.select_entry(index, false, false);
                Task::none()
            }
            Message::EntryDoubleClicked(index) => {
                self.editor.set_selected_entry(Some(index));
                self.editor.select_entry(index, false, false);
                if let Some(archive) = self.editor.selected_archive_mut() {
                    if let Some(entry) = archive.entries.get_mut(index) {
                        entry.rename = true;
                        self.rename_buffer = entry.file_name.to_string();
                    }
                }
                Task::none()
            }
            Message::EntryRightClicked(index) => {
                self.editor.set_selected_entry(Some(index));
                Task::none()
            }
            Message::EntryContextAction(action) => match action {
                EntryAction::CopyName => {
                    if let Some(archive_index) = self.editor.selected_archive() {
                        if let Some(entry_index) = self.editor.selected_entry() {
                            if let Some(archive) = self.editor.archives().get(archive_index) {
                                if let Some(entry) = archive.entries.get(entry_index) {
                                    self.toast = Some(format!("Copied name: {}", entry.file_name));
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
            },

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
                self.config.first_run_complete = true;
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
                self.update_check_manual = false;
                match result {
                    UpdateResult::Available { version, url } => {
                        self.update_state = UpdateState::Available {
                            version: version.clone(),
                            url,
                        };
                        self.show_update_status = Some(format!("Update available: {version}"));
                    }
                    UpdateResult::UpToDate => {
                        self.update_state = UpdateState::UpToDate;
                        self.show_update_status = Some("You are using the latest version.".into());
                    }
                    UpdateResult::Error(err) => {
                        self.update_state = UpdateState::Error(err.clone());
                        self.show_update_status = Some(format!("Update check failed: {err}"));
                    }
                }
                Task::none()
            }

            Message::SetTheme(theme) => {
                self.config.theme = theme;
                self.save_config();
                Task::none()
            }
            Message::TickProgress => Task::none(),
            Message::PaneResized(event) => {
                self.panes.resize(event.split, event.ratio);
                Task::none()
            }

            Message::FilesDropped(path) => {
                if self.editor.selected_archive().is_some() {
                    self.toast = Some(format!("Imported {} dropped files.", 1));
                    if let Some((_index, _archive)) = self.editor.clone_selected_archive() {
                        self.editor.append_import(_index, vec![path], false);
                    }
                } else {
                    self.toast = Some("Open an archive first to drop files into it.".into());
                }
                Task::none()
            }
        }
    }

    fn start_export(&mut self, mode: ExportMode) -> Task<Message> {
        self.last_export_selected_only = matches!(mode, ExportMode::Selected);
        self.toast = None;
        dialogs::save_folder().map(Message::ExportFolderResult)
    }
}

impl App {
    pub fn subscription(&self) -> Subscription<Message> {
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

        let tick = if self.has_active_progress() {
            iced::time::every(Duration::from_millis(120)).map(|_| Message::TickProgress)
        } else {
            Subscription::none()
        };

        let window = iced::window::events().map(|(_id, event)| match event {
            iced::window::Event::FileDropped(path) => Message::FilesDropped(path),
            _ => Message::Noop,
        });

        Subscription::batch([key, tick, window])
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
            container(iced::widget::text(label))
                .padding([4, 12])
                .into()
        }

        let bar = MenuBar::new(vec![
            Item::with_menu(menu_label("File"), file_menu),
            Item::with_menu(menu_label("Edit"), edit_menu),
            Item::with_menu(menu_label("Selection"), selection_menu),
            Item::with_menu(menu_label("Option"), option_menu),
            Item::with_menu(menu_label("Help"), help_menu),
        ]);

        iced::widget::Container::new(bar)
            .width(iced::Length::Fill)
            .style(|theme| iced::widget::container::Style {
                background: Some(theme.extended_palette().background.weak.color.into()),
                ..Default::default()
            })
            .into()
    }
}

fn menu_button<'a>(label: String, message: Message) -> Element<'a, Message> {
    iced::widget::button(
        iced::widget::text(label)
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
            (App::new(cfg), App::startup_task())
        },
        App::update,
        App::view,
    )
    .theme(|state: &App| -> Option<Theme> { Some(state.theme()) })
    .subscription(App::subscription)
    .window_size(size)
    .resizable(true)
    .centered()
    .run()
}

#[allow(dead_code)]
fn _force_space_use(_: Space) {}
