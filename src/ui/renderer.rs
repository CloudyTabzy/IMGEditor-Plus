pub use eframe::egui;

use crate::config::{Config, Theme};
use crate::editor::{Editor, TaskMessage};
use crate::hotkeys::Hotkey;
use crate::parser::{ImgParser, ImgVersion};
#[cfg(feature = "native-dialogs")]
use crate::ui::dialogs;

const ABOUT_TEXT: &str = concat!(
    "Grinch_'s IMG Editor\n",
    "Version 0.1.0\n",
    "\n",
    "Supported formats:\n",
    "- GTA III\n",
    "- GTA Vice City\n",
    "- GTA San Andreas\n",
    "- Bully Scholarship Edition"
);

pub struct MainWindow {
    editor: Editor,
    config: Config,
    search_filter: String,
    rename_buffer: String,
    show_about: bool,
    show_welcome: bool,
    show_unsupported: bool,
    unsupported_path: String,
    completion_receiver: async_channel::Receiver<TaskMessage>,
    completion_sender: async_channel::Sender<TaskMessage>,
    last_applied_theme: Option<Theme>,
    window_rect: Option<egui::Rect>,
}

impl Default for MainWindow {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

impl MainWindow {
    pub fn new(config: Config) -> Self {
        let (sender, receiver) = async_channel::bounded(64);
        let mut editor = Editor::new();
        editor.set_task_sender(sender.clone());
        let show_welcome = !config.first_run_complete;
        Self {
            editor,
            config,
            search_filter: String::new(),
            rename_buffer: String::new(),
            show_about: false,
            show_welcome,
            show_unsupported: false,
            unsupported_path: String::new(),
            completion_receiver: receiver,
            completion_sender: sender,
            last_applied_theme: None,
            window_rect: None,
        }
    }
}

impl eframe::App for MainWindow {
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        if let Some(rect) = self.window_rect {
            self.config.window_size = Some([rect.width(), rect.height()]);
            self.config.window_position = Some([rect.min.x, rect.min.y]);
        }
        let _ = self.config.save();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_completion_messages();

        if self.last_applied_theme != Some(self.config.theme) {
            self.config.theme.apply(ctx);
            self.last_applied_theme = Some(self.config.theme);
        }

        self.handle_dropped_files(ctx);
        self.handle_global_hotkeys(ctx);

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                self.file_menu(ui);
                self.edit_menu(ui);
                self.selection_menu(ui);
                self.option_menu(ui);
                self.help_menu(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.archive_tabs(ui);
        });

        self.modals(ctx);

        if self.has_active_progress() {
            ctx.request_repaint();
        }

        self.window_rect = ctx.input(|i| i.viewport().outer_rect);
    }
}

impl MainWindow {
    fn file_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("File", |ui| {
            let has_archive = self.editor.selected_archive().is_some();
            let has_path = self
                .editor
                .selected_archive()
                .map(|index| self.editor.archives()[index].path.is_some())
                .unwrap_or(false);

            if ui.button("New (Ctrl + N)").clicked() {
                self.editor.new_archive();
                ui.close_menu();
            }
            if ui.button("Open... (Ctrl + O)").clicked() {
                self.open_archive_dialog();
                ui.close_menu();
            }
            ui.add_enabled_ui(has_path, |ui| {
                if ui.button("Save (Ctrl + S)").clicked() {
                    let _ = self.editor.save_archive_in_place();
                    ui.close_menu();
                }
            });
            ui.add_enabled_ui(has_archive, |ui| {
                if ui.button("Save as... (Shift + S)").clicked() {
                    self.save_as_dialog();
                    ui.close_menu();
                }
            });
            ui.add_enabled_ui(has_archive, |ui| {
                if ui.button("Close (Shift + X)").clicked() {
                    self.editor.close_selected_archive();
                    ui.close_menu();
                }
            });
        });
    }

    fn edit_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Edit", |ui| {
            let has_archive = self.editor.selected_archive().is_some();
            let in_progress = self.has_active_progress();

            ui.add_enabled_ui(has_archive, |ui| {
                if ui.button("Import (Ctrl + I)").clicked() {
                    self.import_dialog(false);
                    ui.close_menu();
                }
            });
            ui.add_enabled_ui(has_archive, |ui| {
                if ui.button("Import & replace (Shift + I)").clicked() {
                    self.import_dialog(true);
                    ui.close_menu();
                }
            });
            ui.add_enabled_ui(has_archive && !in_progress, |ui| {
                if ui.button("Export all (Ctrl + E)").clicked() {
                    self.export_dialog(false);
                    ui.close_menu();
                }
            });
            ui.add_enabled_ui(has_archive && !in_progress, |ui| {
                if ui.button("Export selected (Shift + E)").clicked() {
                    self.export_dialog(true);
                    ui.close_menu();
                }
            });
        });
    }

    fn selection_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Selection", |ui| {
            let has_archive = self.editor.selected_archive().is_some();

            ui.add_enabled_ui(has_archive, |ui| {
                if ui.button("Select all (Ctrl + A)").clicked() {
                    self.editor.select_all(true);
                    ui.close_menu();
                }
            });
            ui.add_enabled_ui(has_archive, |ui| {
                if ui.button("Invert selection (Shift + A)").clicked() {
                    self.editor.invert_selection();
                    ui.close_menu();
                }
            });
        });
    }

    fn option_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Option", |ui| {
            ui.label("Theme:");
            for theme in [Theme::Light, Theme::Dark, Theme::System] {
                let selected = self.config.theme == theme;
                if ui.selectable_label(selected, theme.as_str()).clicked() {
                    self.config.theme = theme;
                    ui.close_menu();
                }
            }
        });
    }

    fn help_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Help", |ui| {
            if ui.button("About").clicked() {
                self.show_about = true;
                ui.close_menu();
            }
        });
    }

    fn archive_tabs(&mut self, ui: &mut egui::Ui) {
        let selected = self.editor.selected_archive().unwrap_or(0);
        let tab_ids: Vec<String> = self
            .editor
            .archives()
            .iter()
            .map(|archive| archive.file_name.clone())
            .collect();

        let mut new_selected = selected;
        let mut close_index: Option<usize> = None;

        ui.horizontal_wrapped(|ui| {
            for (index, name) in tab_ids.iter().enumerate() {
                let is_selected = selected == index;
                let response = ui.selectable_label(is_selected, name);
                if response.clicked() {
                    new_selected = index;
                }
                if response.middle_clicked() || response.clicked_by(egui::PointerButton::Secondary)
                {
                    close_index = Some(index);
                }
            }
        });

        if new_selected != selected {
            self.editor.select_archive(new_selected);
        }
        if let Some(index) = close_index {
            self.editor.close_archive(index);
        }

        ui.separator();

        if let Some(index) = self.editor.selected_archive() {
            self.archive_view(ui, index);
        } else {
            ui.with_layout(
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    ui.label("Open or create an archive to get started.");
                },
            );
        }
    }

    fn archive_view(&mut self, ui: &mut egui::Ui, index: usize) {
        if self.editor.archives()[index].update_search {
            let filter = self.search_filter.clone();
            self.editor.update_filtered_list(&filter);
        }

        ui.horizontal(|ui| {
            ui.label("Search:");
            let response = ui.text_edit_singleline(&mut self.search_filter);
            if response.changed() {
                let filter = self.search_filter.clone();
                self.editor.update_filtered_list(&filter);
            }
        });

        ui.columns(2, |columns| {
            self.entry_table(&mut columns[0], index);
            self.info_panel(&mut columns[1], index);
        });
    }

    fn entry_table(&mut self, ui: &mut egui::Ui, archive_index: usize) {
        let row_height = ui.text_style_height(&egui::TextStyle::Body);

        let filtered: Vec<usize> = {
            let archive = &self.editor.archives()[archive_index];
            archive.selected_indices.clone()
        };
        let total_entries = filtered.len();

        egui::ScrollArea::vertical()
            .id_source("entry_scroll")
            .auto_shrink([false; 2])
            .show_rows(ui, row_height, total_entries, |ui, row_range| {
                let width = ui.available_width();
                ui.set_width(width);

                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label("Size");
                        ui.label("Type");
                    });
                });
                ui.separator();

                for display_index in row_range {
                    let entry_index = filtered[display_index];
                    self.entry_row(ui, archive_index, entry_index);
                }
            });
    }

    fn entry_row(&mut self, ui: &mut egui::Ui, archive_index: usize, entry_index: usize) {
        let entry = &self.editor.archives()[archive_index].entries[entry_index];
        let is_selected = entry.selected;
        let is_renaming = entry.rename;
        let file_name = entry.file_name.clone();
        let file_type = entry.file_type.clone();
        let size_kb = entry.sector * 2;

        if is_renaming {
            ui.horizontal(|ui| {
                let response = ui.text_edit_singleline(&mut self.rename_buffer);
                if response.lost_focus() {
                    if ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                        let name = self.rename_buffer.clone();
                        self.editor.rename_selected(&name);
                    }
                    self.editor.archives_mut()[archive_index].entries[entry_index].rename = false;
                }
                response.request_focus();
            });
            return;
        }

        let response = ui
            .horizontal(|ui| {
                ui.selectable_label(
                    is_selected,
                    format!("{file_name}    {file_type}    {size_kb} kb"),
                )
            })
            .inner;

        if response.clicked() {
            let modifiers = ui.input(|input| input.modifiers);
            self.editor.select_entry(
                entry_index,
                modifiers.shift,
                modifiers.ctrl || modifiers.mac_cmd,
            );
        }

        let mut rename_requested = false;
        let mut delete_requested = false;
        let mut export_requested = false;

        response.context_menu(|ui| {
            if ui.button("Copy name").clicked() {
                ui.ctx().output_mut(|output| {
                    output.copied_text = file_name.clone();
                });
                ui.close_menu();
            }
            if ui.button("Rename").clicked() {
                rename_requested = true;
                ui.close_menu();
            }
            if ui.button("Delete").clicked() {
                delete_requested = true;
                ui.close_menu();
            }
            if ui.button("Export").clicked() {
                export_requested = true;
                ui.close_menu();
            }
        });

        if rename_requested {
            self.rename_buffer = file_name.clone();
            self.editor.set_selected_entry(Some(entry_index));
            self.editor.archives_mut()[archive_index].entries[entry_index].rename = true;
        }

        if delete_requested {
            self.editor.archives_mut()[archive_index].entries[entry_index].selected = true;
            self.editor.delete_selected();
        }

        if export_requested {
            self.editor.archives_mut()[archive_index].entries[entry_index].selected = true;
            self.export_dialog(true);
        }

        if response.double_clicked() {
            self.rename_buffer = file_name;
            self.editor.set_selected_entry(Some(entry_index));
            self.editor.archives_mut()[archive_index].entries[entry_index].rename = true;
        }
    }

    fn info_panel(&mut self, ui: &mut egui::Ui, index: usize) {
        let version = self.editor.archives()[index].version;
        let total_entries = self.editor.archives()[index].entries.len();
        let progress = self.editor.archives()[index].progress.percentage();
        let in_progress = self.editor.archives()[index].progress.in_use();
        let logs: Vec<String> = self.editor.archives()[index]
            .logs
            .iter()
            .rev()
            .take(50)
            .cloned()
            .collect();

        ui.vertical(|ui| {
            ui.label(format!("Format: {}", version_text(version)));
            ui.label(format!("Total entries: {total_entries}"));
            ui.separator();

            ui.add(
                egui::ProgressBar::new(progress)
                    .show_percentage()
                    .animate(in_progress),
            );

            if in_progress {
                if ui.button("Cancel").clicked() {
                    self.editor.archives_mut()[index].progress.request_cancel();
                }
            } else {
                ui.add_space(ui.spacing().interact_size.y);
            }

            ui.separator();

            ui.label("Logs:");
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    for message in logs {
                        ui.label(message);
                    }
                });
        });
    }

    fn has_active_progress(&self) -> bool {
        self.editor
            .archives()
            .iter()
            .any(|archive| archive.progress.in_use())
    }

    fn open_archive_dialog(&mut self) {
        #[cfg(feature = "native-dialogs")]
        if let Ok(Some(path)) = dialogs::open_file() {
            if let Err(err) = self.editor.open_archive(&path) {
                if let crate::editor::OpenArchiveError::UnsupportedFormat = err {
                    self.unsupported_path = path.to_string_lossy().to_string();
                    self.show_unsupported = true;
                }
            }
        }
    }

    fn save_as_dialog(&mut self) {
        let Some(index) = self.editor.selected_archive() else {
            return;
        };
        let _default_name = format!("{}.img", self.editor.archives()[index].file_name);
        let _version = self.editor.archives()[index].version;

        #[cfg(feature = "native-dialogs")]
        if let Ok(Some(choice)) = dialogs::save_archive(_default_name, _version) {
            let _ = self.editor.save_archive(&choice.path, choice.version);
        }
    }

    fn import_dialog(&mut self, _replace: bool) {
        #[cfg(feature = "native-dialogs")]
        if let Ok(paths) = dialogs::import_files() {
            self.editor.import_files(&paths, _replace);
        }
    }

    fn export_dialog(&mut self, _selected_only: bool) {
        #[cfg(feature = "native-dialogs")]
        if let Ok(Some(folder)) = dialogs::save_folder() {
            if _selected_only {
                self.editor.export_selected(&folder);
            } else {
                self.editor.export_all(&folder);
            }
        }
    }

    fn modals(&mut self, ctx: &egui::Context) {
        if self.show_welcome {
            egui::Window::new("Welcome")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Welcome to IMGEditor!");
                    ui.label("Open or create an archive to get started.");
                    ui.label("Supported formats: GTA III, Vice City, San Andreas, Bully SE.");
                    if ui.button("Get started").clicked() {
                        self.show_welcome = false;
                        self.config.first_run_complete = true;
                        let _ = self.config.save();
                    }
                });
        }

        if self.show_about {
            egui::Window::new("About")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(ABOUT_TEXT);
                    if ui.button("Close").clicked() {
                        self.show_about = false;
                    }
                });
        }

        if self.show_unsupported {
            egui::Window::new("Unsupported format")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("IMG format not supported!");
                    ui.label(format!("Path: {}", self.unsupported_path));
                    ui.label("Supported formats:");
                    ui.label("1. GTA III");
                    ui.label("2. GTA Vice City");
                    ui.label("3. GTA San Andreas");
                    ui.label("4. Bully Scholarship Edition");
                    if ui.button("Close").clicked() {
                        self.show_unsupported = false;
                    }
                });
        }
    }

    fn handle_global_hotkeys(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }

        let input = ctx.input(|i| i.clone());

        if Hotkey::new(egui::Key::N, None).pressed(&input, egui::Modifiers::CTRL) {
            self.editor.new_archive();
        } else if Hotkey::new(egui::Key::O, None).pressed(&input, egui::Modifiers::CTRL) {
            self.open_archive_dialog();
        } else if Hotkey::new(egui::Key::S, None).pressed(&input, egui::Modifiers::CTRL) {
            let _ = self.editor.save_archive_in_place();
        } else if Hotkey::new(egui::Key::S, None).pressed(&input, egui::Modifiers::SHIFT) {
            self.save_as_dialog();
        } else if Hotkey::new(egui::Key::I, None).pressed(&input, egui::Modifiers::CTRL) {
            self.import_dialog(false);
        } else if Hotkey::new(egui::Key::I, None).pressed(&input, egui::Modifiers::SHIFT) {
            self.import_dialog(true);
        } else if Hotkey::new(egui::Key::E, None).pressed(&input, egui::Modifiers::CTRL) {
            self.export_dialog(false);
        } else if Hotkey::new(egui::Key::E, None).pressed(&input, egui::Modifiers::SHIFT) {
            self.export_dialog(true);
        } else if Hotkey::new(egui::Key::A, None).pressed(&input, egui::Modifiers::CTRL) {
            self.editor.select_all(true);
        } else if Hotkey::new(egui::Key::A, None).pressed(&input, egui::Modifiers::SHIFT) {
            self.editor.invert_selection();
        } else if Hotkey::new(egui::Key::X, None).pressed(&input, egui::Modifiers::SHIFT) {
            self.editor.close_selected_archive();
        } else if Hotkey::new(egui::Key::Delete, None).pressed(&input, egui::Modifiers::NONE) {
            self.editor.delete_selected();
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped_files: Vec<std::path::PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect()
        });

        for path in dropped_files {
            if let Err(err) = self.editor.open_archive(&path) {
                if let crate::editor::OpenArchiveError::UnsupportedFormat = err {
                    self.unsupported_path = path.to_string_lossy().to_string();
                    self.show_unsupported = true;
                }
            }
        }
    }

    fn poll_completion_messages(&mut self) {
        while let Ok(message) = self.completion_receiver.try_recv() {
            match message {
                TaskMessage::SaveCompleted { index, archive } => {
                    if index < self.editor.archives().len() {
                        self.editor.archives_mut()[index] = archive;
                    }
                }
                TaskMessage::ExportCompleted { index } => {
                    if let Some(archive) = self.editor.archives_mut().get_mut(index) {
                        archive.add_log("Exported entries".to_string());
                    }
                }
            }
        }
    }
}

fn version_text(version: ImgVersion) -> &'static str {
    match version {
        ImgVersion::One => crate::parser::PcV1Parser.version_text(),
        ImgVersion::Two => crate::parser::PcV2Parser.version_text(),
        ImgVersion::Unknown => "Unknown",
    }
}
