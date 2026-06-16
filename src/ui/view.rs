use iced::widget::{
    Column, Container, Row, Scrollable, Space, button, column, container, mouse_area,
    pane_grid, progress_bar, rule, row, stack, text, text_input, tooltip,
};
use iced::{Alignment, Border, Color, Element, Length};
use iced_fonts::lucide;

use crate::archive::{SortColumn, SortDirection};

use crate::parser::ImgVersion;
use crate::ui::app::{App, EntryAction, Message, Pane, ABOUT_TEXT};

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
            button(text(name_label))
                .on_press(Message::SortBy(SortColumn::Name))
                .width(Length::FillPortion(6))
                .style(button::text),
            button(text(type_label))
                .on_press(Message::SortBy(SortColumn::Type))
                .width(Length::FillPortion(2))
                .style(button::text),
            button(text(size_label))
                .on_press(Message::SortBy(SortColumn::Size))
                .width(Length::FillPortion(2))
                .style(button::text),
        ]
        .spacing(8)
        .padding(6);

        let mut content = Column::new().spacing(0).width(Length::Fill);
        content = content.push(headers);

        if archive.selected_indices.is_empty() {
            return Container::new(
                column![
                    Space::new().height(Length::Fixed(8.0)),
                    text("No entries match the current filter."),
                ]
                .align_x(Alignment::Center),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
        }

        for &display_index in &archive.selected_indices {
            let Some(entry) = archive.entries.get(display_index) else {
                continue;
            };
            content = content.push(self.build_entry_row(display_index, entry));
        }

        let table = Scrollable::new(content).height(Length::Fill);
        mouse_area(table)
            .on_move(Message::CursorMoved)
            .into()
    }

    fn build_entry_row(
        &self,
        display_index: usize,
        entry: &crate::archive::EntryInfo,
    ) -> Element<'_, Message> {
        let is_renaming = entry.rename;
        let is_selected = entry.selected;
        let size_kb = entry.sector * 2;
        let version = entry.file_type.clone().to_string();
        let file_name = entry.file_name.clone().to_string();

        let name_widget: Element<'_, Message> = if is_renaming {
            text_input("", &self.rename_buffer)
                .on_input(Message::CommitRename)
                .on_submit(Message::CommitRename(self.rename_buffer.clone()))
                .width(Length::FillPortion(6))
                .into()
        } else {
            let label = if is_selected {
                format!("✓ {file_name}")
            } else {
                file_name
            };
            text(label).width(Length::FillPortion(6)).into()
        };

        let row_content: Element<'_, Message> = row![
            name_widget,
            text(version).width(Length::FillPortion(2)),
            text(format!("{size_kb} KB"))
                .width(Length::FillPortion(2))
        ]
        .spacing(8)
        .padding(6)
        .into();

        let cell: iced::widget::Container<'_, Message> = Container::new(row_content)
            .style(move |theme: &iced::Theme| {
                if is_selected {
                    iced::widget::container::Style {
                        background: Some(theme.extended_palette().primary.weak.color.into()),
                        ..Default::default()
                    }
                } else {
                    iced::widget::container::Style::default()
                }
            });
        let cell: Element<'_, Message> = cell.into();

        let cell = mouse_area(cell)
            .on_press(Message::EntryClicked(display_index))
            .on_double_click(Message::EntryDoubleClicked(display_index))
            .on_right_press(Message::EntryRightClicked(display_index));

        cell.into()
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
        let percent_text = format!("{:.0}%", progress * 100.0);

        let mut col = column![
            text(format!("Format: {version_text}")),
            text(format!("Entries: {total} (visible: {visible})")),
            rule::horizontal(1),
            text(format!("Progress: {percent_text}")),
            progress_bar(0.0..=1.0, progress),
        ]
        .spacing(6)
        .padding(8)
        .width(Length::Fixed(280.0));

        if in_use {
            col = col.push(button(text("Cancel")).on_press(Message::CancelActive));
        }

        if let Some(_folder) = archive.last_export_folder.as_ref() {
            if !in_use {
                col = col.push(
                    button(text("Open export folder"))
                        .on_press(Message::OpenLastExportFolder),
                );
            }
        }

        col = col.push(rule::horizontal(1));
        col = col.push(text("Logs:"));

        let logs: Vec<String> = archive.logs.iter().rev().take(50).cloned().collect();
        let log_widget =
            Scrollable::new(Column::with_children(
                logs.into_iter().map(|m| text(m).into()),
            ))
                .height(Length::Fixed(180.0));

        col = col.push(log_widget);

        col.into()
    }

    pub(crate) fn build_status_bar(&self) -> Element<'_, Message> {
        let status_text = self.toast.clone().unwrap_or_else(|| {
            format!("{} v{}", crate::ui::theme::APP_NAME, env!("CARGO_PKG_VERSION"))
        });
        let bar = Container::new(
            Row::new()
                .push(text(status_text))
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
            text("New"),
            tooltip::Position::Bottom,
        ),
        tooltip(
            toolbar_button(lucide::folder_open().size(18).into(), Message::OpenArchive),
            text("Open"),
            tooltip::Position::Bottom,
        ),
        tooltip(
            toolbar_button(lucide::save().size(18).into(), Message::SaveArchive),
            text("Save"),
            tooltip::Position::Bottom,
        ),
        rule::vertical(1),
        tooltip(
            toolbar_button(lucide::download().size(18).into(), Message::ImportFiles),
            text("Import"),
            tooltip::Position::Bottom,
        ),
        tooltip(
            toolbar_button(lucide::upload().size(18).into(), Message::ExportSelected),
            text("Export selected"),
            tooltip::Position::Bottom,
        ),
        rule::vertical(1),
        tooltip(
            toolbar_button(lucide::trash_two().size(18).into(), Message::DeleteSelected),
            text("Delete selected"),
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
            let tab = button(text(label))
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
                text("Open or create an archive to get started.").size(18),
                Space::new().height(Length::Fill),
            ]
            .align_x(Alignment::Center),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    } else {
        let search = row![
            text("Search:"),
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
        build_context_menu(app),
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
            text(ABOUT_TEXT).size(13),
            Space::new().height(Length::Fixed(8.0)),
            row![
                button(text("Visit repository"))
                    .on_press(Message::VisitRepository)
                    .style(button::primary),
                Space::new().width(Length::Fixed(8.0)),
                button(text("Close")).on_press(Message::HideAbout),
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
            text("Welcome to IMGEditor v2!").size(16),
            text("Open or create an archive to get started.").size(13),
            text("Supported formats: GTA III, VC, San Andreas, Bully SE.").size(13),
            Space::new().height(Length::Fixed(8.0)),
            button(text("Get started"))
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
            text("IMG format not supported.").size(14),
            text(format!("Path: {}", path.display())).size(12),
            text("Supported formats: GTA III, Vice City, San Andreas, Bully SE.").size(12),
            Space::new().height(Length::Fixed(8.0)),
            button(text("Close")).on_press(Message::HideUnsupported),
        ]
        .spacing(6),
    ))
}

fn build_update_status(app: &App) -> Option<Element<'_, Message>> {
    let msg = app.show_update_status.clone()?;
    Some(modal_box(
        "Update check",
        column![
            text(msg).size(13),
            Space::new().height(Length::Fixed(8.0)),
            row![
                button(text("Open releases"))
                    .on_press(Message::VisitRepository)
                    .style(button::primary),
                Space::new().width(Length::Fixed(8.0)),
                button(text("Close")).on_press(Message::HideUpdateStatus),
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
        column![text(title).size(16), content]
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

fn build_context_menu(app: &App) -> Option<Element<'_, Message>> {
    let (index, pos) = app.context_menu?;
    let Some(archive) = app
        .editor
        .archives()
        .get(app.editor.selected_archive().unwrap_or(0))
    else {
        return None;
    };
    let entry = archive.entries.get(index)?;

    let card = container(
        column![
            text(format!("{}", entry.file_name)),
            rule::horizontal(1),
            context_button("Export", Message::EntryContextAction(EntryAction::Export)),
            context_button("Rename", Message::EntryContextAction(EntryAction::Rename)),
            context_button("Copy name", Message::EntryContextAction(EntryAction::CopyName)),
            context_button("Delete", Message::EntryContextAction(EntryAction::Delete)),
        ]
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

    let menu = container(card)
        .padding(iced::Padding {
            top: pos.y,
            left: pos.x,
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

fn context_button(label: &str, message: Message) -> iced::widget::Button<'_, Message> {
    button(
        text(label)
            .align_x(iced::alignment::Horizontal::Left)
            .width(Length::Fill),
    )
    .on_press(message)
    .width(Length::Fill)
    .style(crate::ui::view::menu_button_style)
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
