use iced::widget::{
    Column, Container, Row, Scrollable, Space, button, column, mouse_area, progress_bar, rule,
    row, stack, text, text_input,
};
use iced::{Alignment, Border, Color, Element, Length};

use crate::parser::ImgVersion;
use crate::ui::app::{App, Message, ABOUT_TEXT};

impl App {
    pub(crate) fn build_entry_table(&self) -> Element<'_, Message> {
        let Some(archive) = self
            .editor
            .archives()
            .get(self.editor.selected_archive().unwrap_or(0))
        else {
            return Space::new().width(Length::Fill).height(Length::Fill).into();
        };

        let headers = row![
            text("Name").width(Length::FillPortion(6)),
            text("Type").width(Length::FillPortion(2)),
            text("Size").width(Length::FillPortion(2)),
        ]
        .spacing(8)
        .padding(6);

        let mut content = Column::new().spacing(0).width(Length::Fill);
        content = content.push(headers);

        if archive.selected_indices.is_empty() {
            return Container::new(
                column![
                    Space::new().height(Length::Fixed(8.0)),
                    text("No entries match the current filter.").size(13),
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

        Scrollable::new(content).height(Length::Fill).into()
    }

    fn build_entry_row(
        &self,
        display_index: usize,
        entry: &crate::archive::EntryInfo,
    ) -> Element<'_, Message> {
        let is_renaming = entry.rename;
        let is_selected = entry.selected;
        let size_kb = entry.sector * 2;
        let version = entry.file_type.clone();
        let file_name = entry.file_name.clone();

        let name_widget: Element<'_, Message> = if is_renaming {
            text_input("", &self.rename_buffer)
                .on_input(Message::CommitRename)
                .on_submit(Message::CommitRename(self.rename_buffer.clone()))
                .size(13)
                .into()
        } else {
            let label = if is_selected {
                format!("✓ {file_name}")
            } else {
                file_name
            };
            text(label).size(13).into()
        };

        let row_content: Element<'_, Message> = row![
            name_widget,
            text(version).size(13).width(Length::FillPortion(2)),
            text(format!("{size_kb} KB"))
                .size(13)
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

        let mut col = column![
            text(format!("Format: {version_text}")).size(13),
            text(format!("Entries: {total} (visible: {visible})")).size(13),
            rule::horizontal(1),
            progress_bar(0.0..=1.0, progress),
        ]
        .spacing(6)
        .padding(8)
        .width(Length::Fixed(280.0));

        if in_use {
            col = col.push(button(text("Cancel")).on_press(Message::CancelActive));
        }

        col = col.push(rule::horizontal(1));
        col = col.push(text("Logs:").size(13));

        let logs: Vec<String> = archive.logs.iter().rev().take(50).cloned().collect();
        let log_widget =
            Scrollable::new(Column::with_children(
                logs.into_iter().map(|m| text(m).size(12).into()),
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
                .push(text(status_text).size(12))
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

pub fn build(app: &App) -> Element<'_, Message> {
    let menubar = app.menubar();

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

        let table = app.build_entry_table();
        let info = app.build_info_panel();

        let main_row = row![table, rule::vertical(1), info]
            .spacing(4)
            .padding(4)
            .height(Length::Fill);

        column![search, main_row].into()
    };

    let status = app.build_status_bar();
    let base = column![menubar, tab_bar, body, status];

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

pub fn version_label(version: ImgVersion) -> &'static str {
    match version {
        ImgVersion::One => "PC v1",
        ImgVersion::Two => "PC v2",
        ImgVersion::Unknown => "Unknown",
    }
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
