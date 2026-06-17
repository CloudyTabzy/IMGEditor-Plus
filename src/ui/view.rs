use iced::widget::{
    Column, Container, Row, Scrollable, Space, button, column, container, image, mouse_area,
    pane_grid, progress_bar, rule, row, stack, text_input, tooltip,
};
use iced::{Alignment, Border, Color, Element, Length};
use iced_fonts::lucide;

use crate::archive::{ExportStatus, RowDisplay, SortColumn, SortDirection};

use crate::parser::{EntryInspection, ImgVersion};
use crate::ui::app::{App, EntryAction, Message, Pane, ABOUT_TEXT};
use crate::ui::fonts;

/// Height (px) of a single entry row. Must stay in sync with the `height(Length::Fixed(ROW_HEIGHT))`
/// applied in `build_entry_row`; virtualization math depends on it.
const ROW_HEIGHT: f32 = 32.0;
/// Height (px) of the fixed column-header row.
const HEADER_HEIGHT: f32 = 32.0;
/// Number of rows to keep rendered above and below the scroll viewport. 10 rows ≈ 320 px of
/// over-render — negligible cost, eliminates any chance of a blank band at the edges.
const OVERSCAN_ROWS: i32 = 10;

impl App {
    pub(crate) fn build_entry_table(&self) -> Element<'_, Message> {
        let Some(archive) = self
            .editor
            .archives()
            .get(self.editor.selected_archive().unwrap_or(0))
        else {
            return Space::new().width(Length::Fill).height(Length::Fill).into();
        };

        let name_label = sort_label("Name", archive.sort.column == SortColumn::Name, archive.sort.direction);
        let type_label = if archive.sort.column == SortColumn::Type {
            let unique_types = archive.unique_file_types();
            let primary = unique_types
                .get(archive.sort.type_index % unique_types.len().max(1))
                .map(|s| s.to_string())
                .unwrap_or_default();
            format!("Type ↑ {}", primary)
        } else {
            "Type".to_string()
        };
        let size_label = sort_label("Size", archive.sort.column == SortColumn::Size, archive.sort.direction);

        let headers = row![
            button(fonts::header(name_label))
                .on_press(Message::SortBy(SortColumn::Name))
                .width(Length::FillPortion(6))
                .style(button::text),
            button(fonts::header(type_label))
                .on_press(Message::SortBy(SortColumn::Type))
                .width(Length::FillPortion(2))
                .style(button::text),
            button(fonts::header(size_label))
                .on_press(Message::SortBy(SortColumn::Size))
                .width(Length::FillPortion(2))
                .style(button::text),
        ]
        .spacing(8)
        .padding(6)
        .height(Length::Fixed(HEADER_HEIGHT));

        if archive.selected_indices.is_empty() {
            return column![headers, empty_state()]
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        }

        let total = archive.selected_indices.len();
        let total_height = total as f32 * ROW_HEIGHT;
        let scroll_y = self.scroll_y.max(0.0);

        // Window of visible rows, with an overscan to cover any tall viewport.
        let raw_first = ((scroll_y / ROW_HEIGHT) as i32) - OVERSCAN_ROWS;
        let last_inclusive = ((scroll_y / ROW_HEIGHT) as i32) + 64;
        let mut first = raw_first.max(0) as usize;
        let mut last = (last_inclusive as usize + 1).min(total);

        // Always render the renaming row so its text_input never disappears.
        if let Some(rename_row) = renaming_display_row(archive) {
            if rename_row < first {
                first = rename_row;
            } else if rename_row >= last {
                last = (rename_row + 1).min(total);
            }
        }

        let top_pad_rows = first;
        let bottom_pad_rows = total - last;
        let top_pad_height = top_pad_rows as f32 * ROW_HEIGHT;
        let bottom_pad_height = bottom_pad_rows as f32 * ROW_HEIGHT;

        let mut content = Column::new().spacing(0).width(Length::Fill);
        if top_pad_rows > 0 {
            content = content.push(Space::new().height(Length::Fixed(top_pad_height)));
        }

        for display_row in first..last {
            let Some(entry_index) = archive.selected_indices.get(display_row).copied() else {
                continue;
            };
            let Some(entry) = archive.entries.get(entry_index) else {
                continue;
            };
            let row_display = archive.row_cache.get(display_row);
            content = content.push(self.build_entry_row(display_row, entry, row_display));
        }

        if bottom_pad_rows > 0 {
            content = content.push(Space::new().height(Length::Fixed(bottom_pad_height)));
        }

        let content = content.height(Length::Fixed(total_height));

        let scrollable = Scrollable::new(content)
            .id(self.entry_table_id.clone())
            .height(Length::Fill)
            .direction(iced::widget::scrollable::Direction::Vertical(
                iced::widget::scrollable::Scrollbar::new().scroller_width(16.0),
            ))
            .on_scroll(|viewport| Message::ScrollOffsetChanged(viewport.absolute_offset().y));

        // Context menu overlay sits above the scrollable but below the rest of
        // the UI. It is anchored to the right-clicked row's position within
        // the table pane (so we don't need the absolute cursor coordinates,
        // which Iced 0.14's MouseArea doesn't expose).
        let mut layers: Vec<Element<'_, Message>> = Vec::new();
        layers.push(scrollable.into());

        if let Some((entry_index, display_row)) = self.context_menu {
            if let Some(overlay) = build_context_menu(archive, entry_index, display_row, scroll_y) {
                layers.push(overlay);
            }
        }

        let table_body: Element<'_, Message> = stack(layers).into();

        // Middle-click anywhere in the table body to start autoscroll mode.
        let table_body = mouse_area(table_body)
            .on_middle_press(Message::AutoScrollStarted)
            .into();

        // Autoscroll indicator overlay.
        let table_body: Element<'_, Message> = if self.autoscroll.is_some() {
            stack(vec![table_body, build_autoscroll_indicator()]).into()
        } else {
            table_body
        };

        column![headers, table_body]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn build_entry_row(
        &self,
        display_row: usize,
        entry: &crate::archive::EntryInfo,
        row_display: Option<&RowDisplay>,
    ) -> Element<'_, Message> {
        let is_renaming = entry.rename;
        let is_selected = entry.selected;

        let (file_name, file_type, size_kb) = match row_display {
            Some(rd) => (rd.name.clone(), rd.file_type.clone(), rd.size_kb.clone()),
            None => {
                let name = if is_selected {
                    format!("✓ {}", entry.file_name)
                } else {
                    entry.file_name.to_string()
                };
                (
                    name,
                    entry.file_type.to_string(),
                    format!("{} KB", entry.sector * 2),
                )
            }
        };

        let name_widget: Element<'_, Message> = if is_renaming {
            text_input("", &self.rename_buffer)
                .on_input(Message::CommitRename)
                .on_submit(Message::CommitRename(self.rename_buffer.clone()))
                .width(Length::FillPortion(6))
                .into()
        } else {
            let label = if is_selected {
                fonts::strong(file_name)
            } else {
                fonts::body(file_name)
            };
            label.width(Length::FillPortion(6)).into()
        };

        let row_content: Element<'_, Message> = row![
            name_widget,
            if is_selected {
                fonts::strong(file_type).width(Length::FillPortion(2))
            } else {
                fonts::body(file_type).width(Length::FillPortion(2))
            },
            if is_selected {
                fonts::strong(size_kb).width(Length::FillPortion(2))
            } else {
                fonts::body(size_kb).width(Length::FillPortion(2))
            },
        ]
        .spacing(8)
        .padding(6)
        .into();

        let cell = Container::new(row_content)
            .height(Length::Fixed(ROW_HEIGHT))
            .style(move |theme: &iced::Theme| {
                if is_selected {
                    let palette = theme.extended_palette();
                    iced::widget::container::Style {
                        background: Some(palette.primary.weak.color.into()),
                        text_color: Some(palette.primary.weak.text),
                        ..Default::default()
                    }
                } else {
                    iced::widget::container::Style::default()
                }
            });

        // Per-row mouse_area so the click is attributed to this exact row.
        // Iced 0.14's MouseArea only carries a Message (no position), so the
        // right-click absolute position is captured separately by a global
        // event subscription and read by the context menu.
        mouse_area(cell)
            .on_press(Message::EntryClicked(display_row))
            .on_double_click(Message::EntryDoubleClicked(display_row))
            .on_right_press(Message::EntryRightClicked(display_row))
            .on_middle_press(Message::AutoScrollStartedAtRow(display_row))
            .into()
    }

    pub(crate) fn build_info_panel(&self) -> Element<'_, Message> {
        let Some(archive) = self
            .editor
            .archives()
            .get(self.editor.selected_archive().unwrap_or(0))
        else {
            return Space::new().width(Length::Fixed(0.0)).into();
        };

        let version_text = version_label(archive.version);
        let total = archive.entries.len();
        let visible = archive.selected_indices.len();
        let progress = archive.progress.percentage();
        let in_use = archive.progress.in_use();
        let (progress_label, percent_text) = if in_use {
            ("Progress", format!("{:.0}%", progress * 100.0))
        } else {
            match archive.export_status {
                ExportStatus::Ready => ("Progress", "Ready to export".to_string()),
                ExportStatus::Done => ("Progress", "100%".to_string()),
                _ => ("Progress", format!("{:.0}%", progress * 100.0)),
            }
        };

        let mut col = column![
            label_value_owned("Format", version_text.to_string()),
            label_value("Entries", format!("{total} (visible: {visible})")),
            rule::horizontal(1),
            label_value(progress_label, percent_text),
            progress_bar(0.0..=1.0, progress),
        ]
        .spacing(6)
        .padding(8)
        .width(Length::Fixed(280.0));

        if in_use {
            col = col.push(button(fonts::body("Cancel")).on_press(Message::CancelActive));
        }

        if let Some(_folder) = archive.last_export_folder.as_ref() {
            if !in_use {
                col = col.push(
                    button(fonts::body("Open export folder"))
                        .on_press(Message::OpenLastExportFolder),
                );
            }
        }

        col = col.push(rule::horizontal(1));

        if let Some((index, inspection)) = self.inspected_entry.as_ref() {
            if archive.entries.get(*index).is_some() {
                col = col.push(row![
                    fonts::header("Selected entry:"),
                    Space::new().width(Length::Fill),
                    copy_button("Copy", Message::CopySelectedEntryDetails),
                ]);
                col = col.push(Self::build_inspection_panel(inspection));
                col = col.push(rule::horizontal(1));

                // TXD texture preview (if decoded).
                let is_txd = inspection
                    .file_name
                    .as_str()
                    .to_ascii_lowercase()
                    .ends_with(".txd");
                if is_txd {
                    let textures = archive.txd_cache.get(index);
                    if let Some(textures) = textures {
                        if !textures.is_empty() {
                            let tex_idx = self.txd_selected_texture.min(textures.len() - 1);
                            let tex = &textures[tex_idx];

                            col = col.push(
                                button(fonts::body(format!(
                                    "Export textures ({})",
                                    textures.len()
                                )))
                                .on_press(Message::TxdExportTextures),
                            );

                            // Texture selector row (prev / next).
                            if textures.len() > 1 {
                                let mut sel_row = Row::new().spacing(4);
                                sel_row = sel_row.push(fonts::caption("Texture:"));
                                for (i, _) in textures.iter().enumerate() {
                                    let label = if i == tex_idx {
                                        format!("● {}", i + 1)
                                    } else {
                                        format!("○ {}", i + 1)
                                    };
                                    sel_row = sel_row.push(
                                        button(fonts::caption(label))
                                            .on_press(Message::TxdSelectTexture(i))
                                            .style(button::text),
                                    );
                                }
                                col = col.push(sel_row);
                            }

                            col = col.push(label_value_owned("Name", tex.name.clone()));
                            col = col.push(label_value_owned(
                                "Format",
                                format!("{} ({}×{})", tex.format_name, tex.width, tex.height),
                            ));
                            col = col.push(label_value_owned(
                                "Alpha",
                                if tex.has_alpha { "Yes" } else { "No" }.to_string(),
                            ));

                            // Texture image preview.
                            let handle = image::Handle::from_rgba(
                                tex.width,
                                tex.height,
                                tex.rgba.clone(),
                            );
                            let preview = image::Viewer::new(handle)
                                .width(Length::Fill)
                                .height(Length::Fixed(200.0));
                            col = col.push(preview);
                        }
                    } else {
                        // TXD not yet decoded — offer a decode button.
                        col = col.push(
                            button(fonts::body("Decode textures"))
                                .on_press(Message::TxdDecodeRequested),
                        );
                    }
                }
            }
        }

        col = col.push(row![
            fonts::header("Logs:"),
            Space::new().width(Length::Fill),
            copy_button("Copy", Message::CopyLogs),
        ]);

        let logs: Vec<String> = archive.logs.iter().rev().take(50).cloned().collect();
        let log_widget = Column::with_children(
            logs.into_iter().map(|m| fonts::caption(m).into()),
        );

        col = col.push(log_widget);

        if !archive.recent_exports.is_empty() {
            col = col.push(rule::horizontal(1));
            col = col.push(fonts::header("Recent exports:"));
            let exports: Vec<String> = archive.recent_exports.iter().rev().take(8).cloned().collect();
            let exports_widget = Column::with_children(
                exports.into_iter().map(|m| fonts::caption(m).into()),
            );
            col = col.push(exports_widget);
        }

        Scrollable::new(col)
            .width(Length::Fixed(280.0))
            .height(Length::Fill)
            .into()
    }

    fn build_inspection_panel(inspection: &EntryInspection) -> Element<'_, Message> {
        let mut panel = Column::new().spacing(4);

        panel = panel.push(label_value_owned("Name", inspection.file_name.to_string()));
        panel = panel.push(label_value_owned("Type", inspection.file_type.to_string()));

        let size_text = if inspection.size_bytes >= 1024 * 1024 {
            format!(
                "{:.2} MB ({} bytes, {} sectors)",
                inspection.size_bytes as f64 / (1024.0 * 1024.0),
                inspection.size_bytes,
                inspection.size_sectors
            )
        } else if inspection.size_bytes >= 1024 {
            format!(
                "{:.2} KB ({} bytes, {} sectors)",
                inspection.size_bytes as f64 / 1024.0,
                inspection.size_bytes,
                inspection.size_sectors
            )
        } else {
            format!(
                "{} bytes ({} sectors)",
                inspection.size_bytes, inspection.size_sectors
            )
        };
        panel = panel.push(label_value_owned("Size", size_text));
        let offset_text = format!(
            "sector {} (byte {})",
            inspection.offset_bytes / 2048,
            inspection.offset_bytes
        );
        panel = panel.push(label_value_owned("Offset", offset_text));
        panel = panel.push(label_value_owned("Source", inspection.source.to_string()));

        if !inspection.summary.is_empty() {
            panel = panel.push(Space::new().width(Length::Fixed(0.0)).height(Length::Fixed(6.0)));
            for (key, value) in &inspection.summary {
                panel = panel.push(label_value_owned(key, value.to_string()));
            }
        }

        if let Some(preview) = &inspection.preview_hex {
            panel = panel.push(Space::new().width(Length::Fixed(0.0)).height(Length::Fixed(6.0)));
            panel = panel.push(fonts::body("Preview (hex):"));
            panel = panel.push(
                Scrollable::new(fonts::body_monospace(preview.clone()))
                    .direction(iced::widget::scrollable::Direction::Horizontal(
                        iced::widget::scrollable::Scrollbar::new(),
                    ))
                    .height(Length::Fixed(40.0)),
            );
        }

        panel.into()
    }

    pub(crate) fn build_status_bar(&self) -> Element<'_, Message> {
        let status_text = self.toast.clone().unwrap_or_else(|| {
            format!("{} v{}", crate::ui::theme::APP_NAME, env!("CARGO_PKG_VERSION"))
        });
        let bar = Container::new(
            Row::new()
                .push(fonts::caption(status_text))
                .push(Space::new().width(Length::Fill))
                .align_y(Alignment::Center)
                .padding(6),
        )
        .style(|theme: &iced::Theme| iced::widget::container::Style {
            background: Some(theme.extended_palette().background.weak.color.into()),
            ..Default::default()
        });
        bar.into()
    }
}

fn toolbar_button(
    icon: Element<'static, Message>,
    msg: Message,
) -> iced::widget::Button<'static, Message> {
    button(icon)
        .on_press(msg)
        .padding(6)
        .width(Length::Fixed(34.0))
        .height(Length::Fixed(34.0))
}

fn build_toolbar() -> Element<'static, Message> {
    let toolbar = row![
        tooltip(
            toolbar_button(lucide::file_plus().size(18).into(), Message::NewArchive),
            fonts::body("New"),
            tooltip::Position::Bottom,
        ),
        tooltip(
            toolbar_button(lucide::folder_open().size(18).into(), Message::OpenArchive),
            fonts::body("Open"),
            tooltip::Position::Bottom,
        ),
        tooltip(
            toolbar_button(lucide::save().size(18).into(), Message::SaveArchive),
            fonts::body("Save"),
            tooltip::Position::Bottom,
        ),
        rule::vertical(1),
        tooltip(
            toolbar_button(lucide::download().size(18).into(), Message::ImportFiles),
            fonts::body("Import"),
            tooltip::Position::Bottom,
        ),
        tooltip(
            toolbar_button(lucide::upload().size(18).into(), Message::ExportSelected),
            fonts::body("Export selected"),
            tooltip::Position::Bottom,
        ),
        rule::vertical(1),
        tooltip(
            toolbar_button(lucide::trash_two().size(18).into(), Message::DeleteSelected),
            fonts::body("Delete selected"),
            tooltip::Position::Bottom,
        ),
    ]
    .spacing(4)
    .padding(4)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    Container::new(toolbar)
        .width(Length::Fill)
        .height(Length::Fixed(42.0))
        .style(|theme: &iced::Theme| iced::widget::container::Style {
            background: Some(theme.extended_palette().background.weak.color.into()),
            ..Default::default()
        })
        .into()
}

pub fn build(app: &App) -> Element<'_, Message> {
    let menubar = app.menubar();
    let toolbar = build_toolbar();

    let tab_bar: Element<'_, Message> = if app.editor.archives().is_empty() {
        Space::new().height(Length::Fixed(0.0)).into()
    } else {
        let selected = app.editor.selected_archive().unwrap_or(0);
        let mut tabs_row = Row::new().spacing(4).padding(4);
        for (index, archive) in app.editor.archives().iter().enumerate() {
            let is_selected = index == selected;
            let label = if archive.dirty {
                format!("● {}", archive.file_name)
            } else {
                archive.file_name.clone()
            };
            let tab = button(fonts::body(label))
                .on_press(Message::SelectArchiveTab(index))
                .style(if is_selected {
                    button::primary
                } else {
                    button::secondary
                });
            tabs_row = tabs_row.push(tab);
        }
        Container::new(tabs_row).into()
    };

    let body: Element<'_, Message> = if app.editor.archives().is_empty() {
        Container::new(
            column![
                Space::new().height(Length::Fill),
                fonts::display("Open or create an archive to get started."),
                Space::new().height(Length::Fill),
            ]
            .align_x(Alignment::Center),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    } else {
        let search = row![
            fonts::header("Search:"),
            text_input("", &app.search)
                .on_input(Message::SearchChanged)
                .width(Length::Fill),
        ]
        .spacing(8)
        .padding(8);

        let main_row = pane_grid(&app.panes, |_pane, state, _is_maximized| {
            pane_grid::Content::new(match state {
                Pane::Table => app.build_entry_table(),
                Pane::Info => app.build_info_panel(),
            })
        })
        .on_resize(10, Message::PaneResized)
        .height(Length::Fill);

        column![search, main_row].into()
    };

    let status = app.build_status_bar();
    let base = column![menubar, toolbar, tab_bar, body, status]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill);

    let overlays: Vec<Element<'_, Message>> = vec![
        build_about(app),
        build_welcome(app),
        build_unsupported(app),
        build_update_status(app),
    ]
    .into_iter()
    .flatten()
    .collect();

    if overlays.is_empty() {
        return Container::new(base)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
    }

    let mut layers: Vec<Element<'_, Message>> =
        vec![Container::new(base).width(Length::Fill).height(Length::Fill).into()];
    layers.extend(overlays);
    stack(layers).into()
}

fn build_about(app: &App) -> Option<Element<'_, Message>> {
    if !app.show_about {
        return None;
    }
    Some(modal_box(
        "About",
        column![
            fonts::body(ABOUT_TEXT),
            Space::new().height(Length::Fixed(8.0)),
            row![
                button(fonts::body("Visit repository"))
                    .on_press(Message::VisitRepository)
                    .style(button::primary),
                Space::new().width(Length::Fixed(8.0)),
                button(fonts::body("Close")).on_press(Message::HideAbout),
            ]
        ]
        .spacing(6),
    ))
}

fn build_welcome(app: &App) -> Option<Element<'_, Message>> {
    if !app.show_welcome {
        return None;
    }
    Some(modal_box(
        "Welcome",
        column![
            fonts::display("Welcome to IMGEditor v2!"),
            fonts::body("Open or create an archive to get started."),
            fonts::body("Supported formats: GTA III, VC, San Andreas, Bully SE."),
            Space::new().height(Length::Fixed(8.0)),
            button(fonts::strong("Get started"))
                .on_press(Message::HideWelcome)
                .style(button::primary),
        ]
        .spacing(6),
    ))
}

fn build_unsupported(app: &App) -> Option<Element<'_, Message>> {
    let path = app.show_unsupported.clone()?;
    Some(modal_box(
        "Unsupported format",
        column![
            fonts::body("IMG format not supported."),
            fonts::caption(format!("Path: {}", path.display())),
            fonts::caption("Supported formats: GTA III, Vice City, San Andreas, Bully SE."),
            Space::new().height(Length::Fixed(8.0)),
            button(fonts::body("Close")).on_press(Message::HideUnsupported),
        ]
        .spacing(6),
    ))
}

fn build_update_status(app: &App) -> Option<Element<'_, Message>> {
    let msg = app.show_update_status.clone()?;
    Some(modal_box(
        "Update check",
        column![
            fonts::body(msg),
            Space::new().height(Length::Fixed(8.0)),
            row![
                button(fonts::body("Open releases"))
                    .on_press(Message::VisitRepository)
                    .style(button::primary),
                Space::new().width(Length::Fixed(8.0)),
                button(fonts::body("Close")).on_press(Message::HideUpdateStatus),
            ]
        ]
        .spacing(6),
    ))
}

fn modal_box<'a>(
    title: &'a str,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let content: Element<'a, Message> = content.into();
    let card: iced::widget::Container<'_, Message> = Container::new(
        column![fonts::display(title), content]
            .spacing(8)
            .padding(16)
            .max_width(480),
    )
    .style(|theme: &iced::Theme| iced::widget::container::Style {
        background: Some(theme.extended_palette().background.base.color.into()),
        border: Border {
            color: theme.extended_palette().background.strong.color,
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        ..Default::default()
    });

    let card_element: Element<'_, Message> = card.into();
    Container::new(card_element)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

fn build_context_menu(
    archive: &crate::archive::ArchiveInfo,
    entry_index: usize,
    display_row: usize,
    scroll_y: f32,
) -> Option<Element<'_, Message>> {
    let entry = archive.entries.get(entry_index)?;

    let mut items: Vec<Element<'_, Message>> = vec![
        fonts::strong(entry.file_name.to_string()).into(),
        rule::horizontal(1).into(),
    ];

    if entry.file_name.to_lowercase().ends_with(".nif") {
        items.push(
            context_button("Render", Message::EntryContextAction(EntryAction::Render)).into(),
        );
    }

    if entry.file_name.to_lowercase().ends_with(".txd") {
        items.push(
            context_button("View textures",
                Message::EntryContextAction(EntryAction::ViewTextures)).into(),
        );
    }

    items.push(
        context_button("Export", Message::EntryContextAction(EntryAction::Export)).into(),
    );
    items.push(
        context_button("Rename", Message::EntryContextAction(EntryAction::Rename)).into(),
    );
    items.push(
        context_button("Copy name", Message::EntryContextAction(EntryAction::CopyName)).into(),
    );
    items.push(
        context_button("Delete", Message::EntryContextAction(EntryAction::Delete)).into(),
    );

    let card = container(
        iced::widget::Column::with_children(items)
            .spacing(4)
            .padding(8)
            .width(Length::Shrink),
    )
    .style(|theme: &iced::Theme| iced::widget::container::Style {
        background: Some(theme.extended_palette().background.base.color.into()),
        border: Border {
            color: theme.extended_palette().background.strong.color,
            width: 1.0,
            radius: 6.0.into(),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 6.0,
        },
        ..Default::default()
    });

    // Position the menu at the right-clicked row. The row's y in the table pane
    // equals the fixed header height plus the row's position within the
    // scrollable viewport (its content position minus the current scroll).
    let row_y = HEADER_HEIGHT + (display_row as f32 * ROW_HEIGHT - scroll_y).max(0.0);

    let menu = container(card)
        .padding(iced::Padding {
            top: row_y,
            left: 12.0,
            right: 0.0,
            bottom: 0.0,
        })
        .align_x(iced::alignment::Horizontal::Left)
        .align_y(iced::alignment::Vertical::Top)
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

    let backdrop = mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
        .on_press(Message::HideContextMenu);

    Some(stack(vec![backdrop.into(), menu]).into())
}

fn build_autoscroll_indicator() -> Element<'static, Message> {
    let dot = container(
        Space::new()
            .width(Length::Fixed(8.0))
            .height(Length::Fixed(8.0)),
    )
    .style(|theme: &iced::Theme| iced::widget::container::Style {
        background: Some(theme.extended_palette().primary.strong.color.into()),
        border: Border {
            color: theme.extended_palette().background.base.color,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    });

    container(dot)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

fn context_button(label: &str, message: Message) -> iced::widget::Button<'_, Message> {
    button(
        fonts::body(label)
            .align_x(iced::alignment::Horizontal::Left)
            .width(Length::Fill),
    )
    .on_press(message)
    .width(Length::Fill)
    .style(crate::ui::view::menu_button_style)
}

fn label_value(label: &str, value: String) -> Element<'_, Message> {
    row![
        fonts::header(format!("{label}:")),
        Space::new().width(Length::Fixed(4.0)),
        fonts::body(value),
    ]
    .into()
}

fn label_value_owned(label: &str, value: String) -> Element<'_, Message> {
    row![
        fonts::header(format!("{label}:")),
        Space::new().width(Length::Fixed(4.0)),
        fonts::body(value),
    ]
    .into()
}

fn copy_button(label: &str, message: Message) -> Element<'_, Message> {
    button(fonts::caption(label).align_x(iced::alignment::Horizontal::Center))
        .on_press(message)
        .width(Length::Shrink)
        .style(menu_button_style)
        .into()
}

pub fn version_label(version: ImgVersion) -> &'static str {
    match version {
        ImgVersion::One => "PC v1",
        ImgVersion::Two => "PC v2",
        ImgVersion::Unknown => "Unknown",
    }
}

fn sort_label(name: &str, active: bool, direction: SortDirection) -> String {
    if !active {
        return name.to_string();
    }
    let arrow = match direction {
        SortDirection::Ascending => "▲",
        SortDirection::Descending => "▼",
    };
    format!("{name} {arrow}")
}

pub fn menu_button_style(theme: &iced::Theme, status: button::Status) -> button::Style {
    button::Style {
        background: if matches!(
            status,
            button::Status::Hovered | button::Status::Pressed
        ) {
            Some(theme.extended_palette().background.strong.color.into())
        } else {
            None
        },
        text_color: theme.extended_palette().background.base.text,
        ..button::Style::default()
    }
}

fn renaming_display_row(archive: &crate::archive::ArchiveInfo) -> Option<usize> {
    let renaming_entry = archive.entries.iter().position(|e| e.rename)?;
    archive
        .selected_indices
        .iter()
        .position(|&i| i == renaming_entry)
}

fn empty_state() -> Element<'static, Message> {
    Container::new(
        column![
            Space::new().height(Length::Fixed(8.0)),
            fonts::body("No entries match the current filter."),
        ]
        .align_x(Alignment::Center),
    )
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}
