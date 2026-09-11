use crate::archive::{ExportStatus, SortColumn};
use crate::sort::SortDirection;
use iced::widget::{
    Column, Container, Float, Row, Scrollable, Space, button, canvas, checkbox, column, container,
    image, mouse_area, opaque, pane_grid, progress_bar, responsive, row, stack, text_input,
    tooltip,
};
use iced::{Alignment, Border, Color, Element, Length, Rectangle, Vector};

use crate::inspector::scene3d::camera::BaseOrientation;
use crate::inspector::scene3d::pipeline::RenderFlags;
use crate::parser::{EntryInspection, ImgVersion};
use crate::tasks::FolderDuplicatePolicy;
use crate::ui::app::{ABOUT_TEXT, App, EntryAction, InspectorTab, Message, Pane, RippleTarget};
use crate::ui::design::Design;
use crate::ui::fonts;
use crate::ui::icons;
use crate::ui::interaction;
use crate::ui::loading_indicator::LoadingSpinner;
use crate::ui::viewer3d_widget::SceneOriginMode;
use crate::ui::widgets as w;

static LOGO_HANDLE: std::sync::LazyLock<image::Handle> = std::sync::LazyLock::new(|| {
    image::Handle::from_bytes(include_bytes!("../../asset/logo/IMGEditorLogo.png").to_vec())
});

fn logo_element() -> Element<'static, Message> {
    container(
        image(LOGO_HANDLE.clone())
            .width(Length::Fixed(96.0))
            .height(Length::Fixed(96.0))
            .content_fit(iced::ContentFit::Contain),
    )
    .width(Length::Shrink)
    .align_x(Alignment::Center)
    .into()
}

/// Height (px) of a single entry row. Must stay in sync with the `height(Length::Fixed(ROW_HEIGHT))`
/// applied in `build_entry_row`; virtualization math depends on it.
const ROW_HEIGHT: f32 = 32.0;
/// Maximum accent travel used by the selected-row pulse.
/// Peak colour travel of the selected-row pulse. Raised so the pulse
/// reads clearly against the resting selection tint.
const SELECTION_PULSE_BACKGROUND_MAX: f32 = 0.62;
/// Keep the success tint visible without moving a dark-theme status label into
/// the low-contrast middle of a light green background.
const TOAST_BACKGROUND_MAX: f32 = 0.35;
/// Height (px) of the fixed column-header row.
const HEADER_HEIGHT: f32 = 32.0;
/// Number of rows to keep rendered above and below the scroll viewport. 10 rows ≈ 320 px of
/// over-render — negligible cost, eliminates any chance of a blank band at the edges.
const OVERSCAN_ROWS: i32 = 10;

/// The original editor enabled ImGui's alternating table rows. Use the
/// design surface rather than a hard-coded color so the separation remains
/// restrained in both light and dark themes.
fn alternate_entry_row_background(design: &Design) -> Color {
    let base = design.page();
    let contrast = if design.is_dark {
        design.surface_subtle()
    } else {
        design.border()
    };
    let strength = if design.is_dark { 0.25 } else { 0.55 };

    iced::theme::palette::mix(base, contrast, strength)
}

impl App {
    pub(crate) fn build_entry_table(&self) -> Element<'_, Message> {
        let Some(archive_index) = self.editor.selected_archive() else {
            return Space::new().width(Length::Fill).height(Length::Fill).into();
        };
        let Some(archive) = self.editor.archives().get(archive_index) else {
            return Space::new().width(Length::Fill).height(Length::Fill).into();
        };

        let design = self.design();

        let name_label = sort_label(
            "Name",
            archive.sort.column == SortColumn::Name,
            archive.sort.direction,
        );
        let type_label = archive.sort.type_header_label.clone();
        let size_label = sort_label(
            "Size",
            archive.sort.column == SortColumn::Size,
            archive.sort.direction,
        );

        let headers = row![
            container(w::styled_tooltip(
                button(fonts::header(name_label))
                    .on_press(Message::SortBy(SortColumn::Name))
                    .width(Length::Fill)
                    .style(button::text),
                fonts::caption(sort_tooltip_text(
                    SortColumn::Name,
                    archive.sort.column == SortColumn::Name,
                    archive.sort.direction,
                    archive.primary_type_label(),
                )),
                tooltip::Position::Bottom,
            ))
            .width(Length::FillPortion(6)),
            container(w::styled_tooltip(
                button(fonts::header(type_label))
                    .on_press(Message::SortBy(SortColumn::Type))
                    .width(Length::Fill)
                    .style(button::text),
                fonts::caption(sort_tooltip_text(
                    SortColumn::Type,
                    archive.sort.column == SortColumn::Type,
                    archive.sort.direction,
                    archive.primary_type_label(),
                )),
                tooltip::Position::Bottom,
            ))
            .width(Length::FillPortion(2)),
            container(w::styled_tooltip(
                button(fonts::header(size_label))
                    .on_press(Message::SortBy(SortColumn::Size))
                    .width(Length::Fill)
                    .style(button::text),
                fonts::caption(sort_tooltip_text(
                    SortColumn::Size,
                    archive.sort.column == SortColumn::Size,
                    archive.sort.direction,
                    archive.primary_type_label(),
                )),
                tooltip::Position::Bottom,
            ))
            .width(Length::FillPortion(2)),
        ]
        .spacing(8)
        .padding(6)
        .height(Length::Fixed(HEADER_HEIGHT));

        let header_bg = design.surface();
        let headers = Container::new(headers)
            .width(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(header_bg)),
                ..Default::default()
            });

        if archive.selected_indices.is_empty() {
            return column![headers, w::hairline(design.divider()), empty_state()]
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        }

        let total = archive.selected_indices.len();
        let total_height = total as f32 * ROW_HEIGHT;
        let scroll_y = self.scroll_y.max(0.0);

        // Compatibility verdicts by entry index, built once per frame so
        // the row styling below is a map lookup, not a scan per row.
        // Skipped entirely when the user turned highlighting off.
        let compat_verdicts: std::collections::HashMap<usize, crate::compat::games::Verdict> =
            if self.compat_highlight_enabled {
                archive
                    .compat_report
                    .as_ref()
                    .map(|report| {
                        report
                            .entry_verdicts
                            .iter()
                            .map(|verdict| (verdict.entry_index, verdict.worst))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                std::collections::HashMap::new()
            };

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
            content =
                content.push(self.build_entry_row(
                    archive_index,
                    entry_index,
                    display_row,
                    entry,
                    compat_verdicts.get(&entry_index).copied(),
                ));
        }

        if bottom_pad_rows > 0 {
            content = content.push(Space::new().height(Length::Fixed(bottom_pad_height)));
        }

        let content = content.height(Length::Fixed(total_height));

        let scrollable = Scrollable::new(content)
            .id(iced::widget::Id::new("entry_table"))
            .height(Length::Fill)
            .auto_scroll(self.context_menu.is_none())
            .direction(iced::widget::scrollable::Direction::Vertical(
                iced::widget::scrollable::Scrollbar::new().scroller_width(16.0),
            ))
            .on_scroll(|viewport| Message::ScrollOffsetChanged {
                y: viewport.absolute_offset().y,
                max_y: (viewport.content_bounds().height - viewport.bounds().height).max(0.0),
            });

        // Context menu overlay sits above the scrollable but below the rest of
        // the UI. It is anchored to the right-clicked row's position within
        // the table pane (so we don't need the absolute cursor coordinates,
        // which Iced 0.14's MouseArea doesn't expose).
        let mut layers: Vec<Element<'_, Message>> = Vec::new();
        layers.push(scrollable.into());

        if let Some((entry_index, display_row)) = self.context_menu
            && let Some(overlay) = build_context_menu(
                archive,
                entry_index,
                display_row,
                scroll_y,
                design.divider(),
            )
        {
            layers.push(overlay);
        }

        // Keep this wrapper and its base Scrollable present regardless of
        // autoscroll state. Iced retains Scrollable offsets by widget-tree
        // position, so inserting a different root on MMB would reset it.
        let table_body = mouse_area(
            stack(layers)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_enter(Message::EntryTableHoverChanged(true))
        .on_exit(Message::EntryTableHoverChanged(false));

        column![headers, w::hairline(design.divider()), table_body]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn build_entry_row<'a>(
        &'a self,
        archive_index: usize,
        entry_index: usize,
        display_row: usize,
        entry: &'a crate::archive::EntryInfo,
        compat_verdict: Option<crate::compat::games::Verdict>,
    ) -> Element<'a, Message> {
        use std::borrow::Cow;

        let row_interactive = !self.autoscroll;
        let is_renaming = entry.rename && row_interactive;
        let is_selected = entry.selected;
        // Use the visible-row position so filtering and sorting preserve a
        // stable zebra pattern instead of making stripes appear to jump.
        let design = self.design();
        let alternate_background = (display_row % 2 == 1)
            .then(|| alternate_entry_row_background(&design));
        let literal_types = self.config.literal_file_types;

        // Render display strings on demand for the visible row only. Pre-caching
        // these for every filtered entry caused thousands of allocations each
        // time the filter or selection changed.
        let file_name: Cow<'_, str> = if is_selected {
            Cow::Owned(format!("✓ {}", entry.file_name))
        } else {
            Cow::Borrowed(entry.file_name.as_str())
        };
        let file_type = Cow::Borrowed(entry.display_file_type(literal_types).as_str());
        let size_kb = Cow::Owned(format!("{} KB", entry.sector * 2));

        let entry_key = (archive_index, entry_index);
        let selection_pulse = self.entry_selection_pulse(entry_key);
        let icon_nudge = self.entry_icon_nudge(entry_key);
        let text_nudge = self.entry_text_nudge(entry_key);
        // The pulse micro-motion renders the name and icon through a
        // `Float` overlay, which does NOT inherit the row container's
        // text color — without an explicit color the overlay text falls
        // back to the theme default and flashes white in themes whose
        // resting text isn't white. Compute the same color the row
        // style applies and pin it onto both widgets.
        let theme = self.theme();
        let extended = theme.extended_palette();
        let row_text_color = if is_selected {
            let peak_background = iced::theme::palette::mix(
                extended.primary.weak.color,
                extended.primary.strong.color,
                SELECTION_PULSE_BACKGROUND_MAX,
            );
            w::stable_readable_text_color(
                extended.primary.weak.color,
                peak_background,
                extended.primary.weak.text,
            )
        } else if let Some(background) = alternate_background {
            w::readable_text_color(background, extended.background.base.text)
        } else {
            extended.background.base.text
        };
        let icon_scale = if self.config.icon_micro_motion_enabled {
            1.0 + selection_pulse * 0.10
        } else {
            1.0
        };
        let file_type_icon =
            icons::file_type(&entry.file_name).style(text_color_fn(row_text_color));
        let file_icon: Element<'_, Message> = if icon_nudge.abs() > f32::EPSILON || icon_scale > 1.0
        {
            Float::new(file_type_icon.size(16))
                .scale(icon_scale)
                .translate(move |_, _| Vector::new(icon_nudge, 0.0))
                .into()
        } else {
            file_type_icon.size(16).into()
        };

        let name_widget: Element<'_, Message> = if is_renaming {
            text_input("", &self.rename_buffer)
                .id(iced::widget::Id::new("rename_input"))
                .on_input(Message::RenameInputChanged)
                .on_submit(Message::CommitRename)
                .width(Length::Fill)
                .into()
        } else {
            let label = if is_selected {
                fonts::strong(file_name)
            } else {
                fonts::body(file_name)
            };
            label
                .style(text_color_fn(row_text_color))
                .width(Length::Fill)
                .into()
        };

        let name_widget: Element<'_, Message> = if !is_renaming && text_nudge.abs() > f32::EPSILON {
            Float::new(name_widget)
                .translate(move |_, _| Vector::new(text_nudge, 0.0))
                .into()
        } else {
            name_widget
        };
        let name_cell = w::icon_label(file_icon, name_widget).width(Length::FillPortion(6));

        let row_content: Element<'_, Message> = row![
            name_cell,
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

        // Compatibility highlight for this row, when a per-target
        // validation has been run on this archive.
        let compat_row = compat_verdict;

        let cell = Container::new(row_content)
            .height(Length::Fixed(ROW_HEIGHT))
            .style(move |theme: &iced::Theme| {
                if is_selected {
                    let palette = theme.extended_palette();
                    let peak_background = iced::theme::palette::mix(
                        palette.primary.weak.color,
                        palette.primary.strong.color,
                        SELECTION_PULSE_BACKGROUND_MAX,
                    );
                    let background = iced::theme::palette::mix(
                        palette.primary.weak.color,
                        palette.primary.strong.color,
                        selection_pulse * SELECTION_PULSE_BACKGROUND_MAX,
                    );
                    iced::widget::container::Style {
                        background: Some(background.into()),
                        text_color: Some(w::stable_readable_text_color(
                            palette.primary.weak.color,
                            peak_background,
                            palette.primary.weak.text,
                        )),
                        ..Default::default()
                    }
                } else if let Some(verdict) = compat_row {
                    compat_row_style(verdict)
                } else if let Some(background) = alternate_background {
                    iced::widget::container::Style {
                        background: Some(background.into()),
                        text_color: Some(row_text_color),
                        ..Default::default()
                    }
                } else {
                    iced::widget::container::Style::default()
                }
            });

        let cell: Element<'_, Message> = if let Some(ripple) =
            self.ripple_visual(RippleTarget::Entry {
                archive_index,
                entry_index: entry_key.1,
            }) {
            stack(vec![
                cell.into(),
                interaction::ripple_overlay::<Message>(ripple)
                    .width(Length::Fill)
                    .height(Length::Fixed(ROW_HEIGHT))
                    .into(),
            ])
            .width(Length::Fill)
            .height(Length::Fixed(ROW_HEIGHT))
            .clip(true)
            .into()
        } else {
            cell.into()
        };

        if row_interactive {
            // Per-row mouse_area so the click is attributed to this exact row.
            // Iced 0.14's MouseArea only carries a Message (no position), so the
            // right-click absolute position is captured separately by a global
            // event subscription and read by the context menu.
            mouse_area(cell)
                .on_press(Message::EntryClicked(display_row))
                .on_double_click(Message::EntryDoubleClicked(display_row))
                .on_right_press(Message::EntryRightClicked(display_row))
                .into()
        } else {
            // Let the native Scrollable receive the stopping click before a
            // row can react, matching browser autoscroll behavior.
            cell
        }
    }

    pub(crate) fn build_info_panel(&self) -> Element<'_, Message> {
        // The pane grid owns the inspector width. Keeping this adaptive lets
        // the preview use the entire pane instead of leaving a fixed-width
        // column stranded on the left with a large blank region beside it.
        let width = Length::Fill;

        let export_tab = self.build_export_tab();
        let model_tab = self.build_model_tab();
        let texture_tab = self.build_texture_tab();

        let bold_text = iced::Font {
            family: iced::font::Family::default(),
            weight: iced::font::Weight::Bold,
            ..iced::Font::default()
        };
        let selected_tab = self.selected_inspector_tab;
        let tab_pulse = self.inspector_tab_selection_pulse(selected_tab);
        let tab_bar = iced_aw::widget::tab_bar::TabBar::new(Message::Viewer3dSelectTab)
            .push(
                InspectorTab::Export,
                iced_aw::TabLabel::Text("Export".to_string()),
            )
            .push(
                InspectorTab::Model3D,
                iced_aw::TabLabel::Text("3D view".to_string()),
            )
            .push(
                InspectorTab::Texture,
                iced_aw::TabLabel::Text("Texture".to_string()),
            )
            .set_active_tab(&selected_tab)
            .tab_width(Length::FillPortion(1))
            .style(move |theme, status| {
                let mut style = iced_aw::style::tab_bar::primary(theme, status);
                let mut background = match style.tab_label_background {
                    iced::Background::Color(color) => color,
                    _ => theme.extended_palette().background.base.color,
                };
                if status == iced_aw::style::Status::Active && tab_pulse > 0.0 {
                    background = iced::theme::palette::mix(
                        background,
                        theme.extended_palette().primary.strong.color,
                        tab_pulse * 0.38,
                    );
                    style.tab_label_background = background.into();
                }
                let foreground = w::readable_text_color(
                    background,
                    theme.extended_palette().background.base.text,
                );
                style.text_color = foreground;
                style.icon_color = foreground;
                style
            })
            .text_size(13.0)
            .text_font(bold_text)
            .height(Length::Fixed(32.0))
            .width(width);

        let tab_bar: Element<'_, Message> =
            if let Some(ripple) = self.ripple_visual(RippleTarget::InspectorTab(selected_tab)) {
                stack(vec![
                    tab_bar.into(),
                    interaction::ripple_overlay::<Message>(ripple)
                        .width(width)
                        .height(Length::Fixed(32.0))
                        .into(),
                ])
                .width(width)
                .height(Length::Fixed(32.0))
                .clip(true)
                .into()
            } else {
                tab_bar.into()
            };

        let tab_bar = Container::new(tab_bar)
            .width(width)
            .height(Length::Fixed(32.0))
            .style({
                let chrome = self.design().chrome();
                move |_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(chrome)),
                    ..Default::default()
                }
            });

        let active_content: Element<'_, Message> = match selected_tab {
            InspectorTab::Export => export_tab,
            InspectorTab::Model3D => model_tab,
            InspectorTab::Texture => texture_tab,
        };

        column![
            tab_bar,
            Container::new(active_content)
                .width(width)
                .height(Length::Fill),
        ]
        .width(width)
        .height(Length::Fill)
        .into()
    }

    fn build_export_tab(&self) -> Element<'_, Message> {
        let Some(archive) = self
            .editor
            .archives()
            .get(self.editor.selected_archive().unwrap_or(0))
        else {
            return Space::new().width(Length::Fill).height(Length::Fill).into();
        };

        let design = self.design();

        let version_text = version_label(archive.version);
        let total = archive.entries.len();
        let visible = archive.selected_indices.len();
        let raw_progress = archive.progress.percentage();
        let in_use = archive.progress.in_use();
        let progress = self
            .animator
            .get_or(crate::ui::app::ANIM_PROGRESS, raw_progress);
        let display_progress = if in_use { progress } else { raw_progress };
        let (progress_label, percent_text) = if in_use {
            ("Progress", format!("{:.0}%", display_progress * 100.0))
        } else {
            match archive.export_status {
                ExportStatus::Ready => ("Progress", "Ready to export".to_string()),
                ExportStatus::Done => ("Progress", "100%".to_string()),
                _ => ("Progress", format!("{:.0}%", progress * 100.0)),
            }
        };

        let progress_widget: Element<'_, Message> = if in_use && self.config.motion_enabled {
            stack(vec![
                progress_bar(0.0..=1.0, display_progress).into(),
                interaction::shimmer_overlay::<Message>(self.shimmer_phase, display_progress)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
            ])
            .width(Length::Fill)
            .into()
        } else {
            progress_bar(0.0..=1.0, display_progress).into()
        };

        let mut col = column![
            label_value_owned("Format", version_text.to_string()),
            label_value("Entries", format!("{total} (visible: {visible})")),
            w::hairline(design.divider()),
            label_value(progress_label, percent_text),
            progress_widget,
        ]
        .spacing(6)
        .padding(8)
        .width(Length::Fill);

        if in_use {
            col = col.push(
                button(w::icon_label(
                    icons::close().size(14),
                    fonts::body("Cancel"),
                ))
                .on_press(Message::CancelActive),
            );
        }

        if let Some(_folder) = archive.last_export_folder.as_ref()
            && !in_use
        {
            col = col.push(
                button(w::icon_label(
                    icons::open_archive().size(14),
                    fonts::body("Open export folder"),
                ))
                .on_press(Message::OpenLastExportFolder),
            );
        }

        col = col.push(w::hairline(design.divider()));

        if let Some((index, inspection)) = self.inspected_entry.as_ref()
            && archive.entries.get(*index).is_some()
        {
            col = col.push(row![
                fonts::header("Selected entry:"),
                Space::new().width(Length::Fill),
                copy_button("Copy", Message::CopySelectedEntryDetails),
            ]);
            col = col.push(Self::build_inspection_panel(
                inspection,
                self.config.literal_file_types,
            ));
            col = col.push(w::hairline(design.divider()));
        }

        col = col.push(row![
            fonts::header("Logs:"),
            Space::new().width(Length::Fill),
            copy_button("Copy", Message::CopyLogs),
        ]);

        let logs: Vec<String> = archive.logs.iter().rev().take(50).cloned().collect();
        let log_widget = Column::with_children(logs.into_iter().map(|m| fonts::caption(m).into()));
        let log_bg = design.page();
        let log_border = design.divider();
        col = col.push(
            Container::new(log_widget)
                .width(Length::Fill)
                .padding(6)
                .style(move |_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(log_bg)),
                    border: iced::Border {
                        color: log_border,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                }),
        );

        if !archive.recent_exports.is_empty() {
            col = col.push(w::hairline(design.divider()));
            col = col.push(fonts::header("Recent exports:"));
            let exports: Vec<String> = archive
                .recent_exports
                .iter()
                .rev()
                .take(8)
                .cloned()
                .collect();
            let exports_widget =
                Column::with_children(exports.into_iter().map(|m| fonts::caption(m).into()));
            col = col.push(exports_widget);
        }

        Scrollable::new(col)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn build_model_tab(&self) -> Element<'_, Message> {
        let Some(archive) = self
            .editor
            .archives()
            .get(self.editor.selected_archive().unwrap_or(0))
        else {
            return container(fonts::caption("No archive open."))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into();
        };
        let Some(entry_index) = self.editor.selected_entry() else {
            return container(fonts::caption(
                "Select a .nif or .dff entry to preview it in 3D.",
            ))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into();
        };
        let Some(entry) = archive.entries.get(entry_index) else {
            return container(fonts::caption("The selected entry is no longer available."))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into();
        };
        let entry_lower = entry.file_name.to_ascii_lowercase();
        let is_model = entry_lower.ends_with(".nif") || entry_lower.ends_with(".dff");
        let scene_matches = self.viewer_scene_matches_selection();
        let loading = self.viewer_load_matches_selection();
        let gpu_error = scene_matches
            .then(|| self.viewer3d_handle.with(|inner| inner.gpu_error.clone()))
            .flatten();

        let toolbar = self.build_viewer3d_toolbar(is_model, scene_matches, loading);
        let stats = self.build_viewer3d_stats(scene_matches);

        let body: Element<'_, Message> = if let Some(error) = gpu_error {
            container(
                column![
                    fonts::header("GPU viewer unavailable"),
                    fonts::caption(error),
                    fonts::caption("Try clearing the preview or selecting a smaller model."),
                    button(fonts::body("Clear viewer error")).on_press(Message::Viewer3dClear),
                ]
                .spacing(8)
                .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .padding(16)
            .into()
        } else if scene_matches {
            let widget =
                crate::ui::viewer3d_widget::Scene3dWidget::new(self.viewer3d_handle.clone());
            widget.into()
        } else if loading {
            let entry_name = self.viewer_loading_entry_name().unwrap_or("selected model");
            container(
                column![
                    canvas::Canvas::new(LoadingSpinner::new(self.viewer_load_phase))
                        .width(Length::Fixed(48.0))
                        .height(Length::Fixed(48.0)),
                    fonts::header("Preparing 3D preview"),
                    fonts::body(entry_name),
                    fonts::caption("Reading geometry and resolving textures…"),
                    fonts::caption("Future previews of this model will be instant."),
                ]
                .spacing(8)
                .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .padding(16)
            .into()
        } else if is_model {
            container(fonts::caption("Ready to preview this model in 3D."))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .padding(16)
                .into()
        } else {
            container(fonts::caption(format!(
                "The in-app viewer renders .nif and .dff entries. {} is not a supported model — use the right-click menu for another viewer.",
                entry_lower
            )))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into()
        };

        let prompt: Element<'_, Message> = if loading {
            Space::new().height(Length::Fixed(0.0)).into()
        } else if !scene_matches && is_model {
            fonts::caption("Use ‘Load selected’ above to preview this model.").into()
        } else if !scene_matches {
            fonts::caption("Select a .nif or .dff entry, then right-click → Open in 3D viewer.")
                .into()
        } else {
            Space::new().height(Length::Fixed(0.0)).into()
        };

        let mut col = column![toolbar]
            .spacing(4)
            .padding(4)
            .width(Length::Fill)
            .height(Length::Fill);
        col = col.push(body);
        col = col.push(stats);
        col = col.push(prompt);
        col.into()
    }

    fn build_viewer3d_stats(&self, scene_matches: bool) -> Element<'_, Message> {
        let (triangles, vertices, textures, has_scene, w, h, orientation, origin_mode) =
            self.viewer3d_handle.with(|i| {
                let w = i.camera.viewport.width.max(1);
                let h = i.camera.viewport.height.max(1);
                let orient = i
                    .scene
                    .as_ref()
                    .filter(|_| scene_matches)
                    .map(|s| s.base_orientation)
                    .unwrap_or(BaseOrientation::Yup);
                (
                    i.scene
                        .as_ref()
                        .filter(|_| scene_matches)
                        .map(|s| s.total_triangles())
                        .unwrap_or(0),
                    i.scene
                        .as_ref()
                        .filter(|_| scene_matches)
                        .map(|s| s.total_vertices())
                        .unwrap_or(0),
                    i.scene
                        .as_ref()
                        .filter(|_| scene_matches)
                        .map(|s| s.textured_mesh_count())
                        .unwrap_or(0),
                    scene_matches && i.scene.is_some(),
                    w,
                    h,
                    orient,
                    i.origin_mode,
                )
            });
        let orient_label = match orientation {
            BaseOrientation::Yup => "Y-up",
            BaseOrientation::Zup => "Z-up",
            BaseOrientation::Xup => "X-up",
        };
        let origin_label = match origin_mode {
            SceneOriginMode::Centered => "centered",
            SceneOriginMode::World => "world",
        };
        let line = if has_scene {
            format!(
                "{} vertices   {} triangles   {} textures   {}×{}   {}   {}",
                vertices, triangles, textures, w, h, orient_label, origin_label
            )
        } else if let Some(entry_name) = self.viewer_loading_entry_name() {
            format!("Preparing {entry_name}…")
        } else {
            "No scene loaded".to_string()
        };
        container(fonts::caption(line))
            .width(Length::Fill)
            .height(Length::Fixed(20.0))
            .align_x(Alignment::Center)
            .padding(2)
            .into()
    }
    fn build_texture_tab(&self) -> Element<'_, Message> {
        let Some(archive) = self
            .editor
            .archives()
            .get(self.editor.selected_archive().unwrap_or(0))
        else {
            return container(fonts::caption("No archive open."))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into();
        };
        let Some(entry_index) = self.editor.selected_entry() else {
            return container(fonts::caption(
                "Select a TXD, NFT, NIF, or DFF entry to preview textures.",
            ))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into();
        };
        let Some(entry) = archive.entries.get(entry_index) else {
            return container(fonts::caption(
                "Select a .txd, .nft, .nif, or .dff entry to preview textures.",
            ))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into();
        };
        let entry_name = entry.file_name.to_string();
        let lower = entry_name.to_ascii_lowercase();
        let is_txd = lower.ends_with(".txd");
        let is_nft = lower.ends_with(".nft");
        let is_nif = lower.ends_with(".nif");
        let is_dff = lower.ends_with(".dff");
        if !is_txd && !is_nft && !is_nif && !is_dff {
            return container(fonts::caption(format!(
                "{} is not a texture container. Preview is available for TXD, NFT, or rendered model entries.",
                entry_name
            )))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into();
        }
        let textures = archive.texture_cache.get(&entry_index);
        let Some(textures) = textures else {
            if is_nif || is_dff {
                return column![
                    fonts::caption("Load the selected model to resolve its textures."),
                    button(w::icon_label(
                        icons::model().size(14),
                        fonts::body("Load selected model"),
                    ))
                    .on_press(Message::Viewer3dLoadSelected),
                ]
                .spacing(6)
                .align_x(Alignment::Center)
                .padding(8)
                .into();
            }
            let kind = if is_nft { "NFT" } else { "TXD" };
            return column![
                fonts::caption(format!("{kind} {entry_name} is not yet decoded.")),
                button(w::icon_label(
                    icons::texture().size(14),
                    fonts::body(format!("Load selected {kind} textures")),
                ))
                .on_press(Message::TextureDecodeRequested),
            ]
            .spacing(4)
            .align_x(Alignment::Center)
            .padding(8)
            .into();
        };
        if textures.is_empty() {
            return container(fonts::caption("No decodable textures in this container."))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into();
        }
        let tex_idx = self.selected_texture.min(textures.len() - 1);
        let tex = &textures[tex_idx];
        let mut col = Column::new()
            .spacing(6)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8);
        let mut action_row = Row::new()
            .spacing(6)
            .width(Length::Fill)
            .align_y(Alignment::Center);
        if (is_nif || is_dff) && !self.viewer_scene_matches_selection() {
            action_row = action_row.push(
                button(w::icon_label(
                    icons::model().size(14),
                    fonts::body("Load selected model"),
                ))
                .on_press(Message::Viewer3dLoadSelected),
            );
        }
        action_row = action_row.push(
            button(w::icon_label(
                icons::export().size(14),
                fonts::body(format!("Export textures ({})", textures.len())),
            ))
            .on_press(Message::TextureExport),
        );
        col = col.push(action_row);
        if textures.len() > 1 {
            let mut sel_row = Row::new()
                .spacing(4)
                .width(Length::Shrink)
                .height(Length::Fixed(32.0))
                .align_y(Alignment::Center);
            for (i, texture) in textures.iter().enumerate() {
                let label = if i == tex_idx {
                    format!("● {}", i + 1)
                } else {
                    format!("○ {}", i + 1)
                };
                let slot_button = button(
                    fonts::caption(label)
                        .width(Length::Fill)
                        .align_x(Alignment::Center),
                )
                // Keep the slot label in one line even for slots 10+.
                // The default button padding leaves too little content
                // width inside a compact fixed-width button.
                .width(Length::Fixed(42.0))
                .height(Length::Fixed(32.0))
                .padding([4.0, 5.0])
                .on_press(Message::TextureSelect(i))
                .style(button::text);
                sel_row = sel_row.push(w::styled_tooltip(
                    slot_button,
                    fonts::body(texture.name.clone()),
                    tooltip::Position::Bottom,
                ));
            }
            let slot_rail = Scrollable::new(sel_row)
                .width(Length::Fill)
                .height(Length::Fixed(38.0))
                .direction(iced::widget::scrollable::Direction::Horizontal(
                    iced::widget::scrollable::Scrollbar::new().scroller_width(10.0),
                ));
            col = col.push(
                row![
                    fonts::caption(format!("Texture {}/{}", tex_idx + 1, textures.len())),
                    slot_rail,
                ]
                .spacing(6)
                .align_y(Alignment::Center)
                .width(Length::Fill),
            );
        }
        let texture_meta = row![
            row![fonts::header("Name:"), fonts::body(tex.name.clone())]
                .spacing(3)
                .align_y(Alignment::Center),
            row![
                fonts::header("Format:"),
                fonts::body(format!(
                    "{} ({}×{})",
                    tex.format_name, tex.width, tex.height
                )),
            ]
            .spacing(3)
            .align_y(Alignment::Center),
            row![
                fonts::header("Alpha:"),
                fonts::body(if tex.has_alpha { "Yes" } else { "No" }),
            ]
            .spacing(3)
            .align_y(Alignment::Center),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .wrap();
        col = col.push(texture_meta);

        let scene_matches = self.viewer_scene_matches_selection();
        let texture_only_preview = !scene_matches && (is_txd || is_nft);
        let uv_triangles = self.viewer3d_handle.with(|inner| {
            inner
                .scene
                .as_deref()
                .filter(|_| scene_matches)
                .map(|scene| crate::ui::texture_preview::uv_triangles_for_texture(scene, &tex.name))
                .unwrap_or_default()
        });
        let uv_tooltip = if texture_only_preview {
            "Standalone texture preview. Select a matching DFF or NIF model to enable UV mapping."
        } else if uv_triangles.is_empty() {
            "UV mapping is available after matching model geometry is loaded."
        } else {
            "Show the UV triangles from the matching model geometry."
        };
        let uv_toggle = w::styled_tooltip(
            checkbox(self.show_texture_uv && !uv_triangles.is_empty())
                .label("Show UV map")
                .on_toggle_maybe((!uv_triangles.is_empty()).then_some(Message::TextureUvToggled)),
            fonts::caption(uv_tooltip),
            tooltip::Position::Top,
        );
        let uv_status = if texture_only_preview {
            fonts::caption("Texture-only preview · UV map needs matching model geometry")
        } else if uv_triangles.is_empty() {
            fonts::caption("Load matching model geometry to enable UVs")
        } else {
            fonts::caption(format!("{} triangles", uv_triangles.len()))
        };
        let texture_only_notice: Element<'_, Message> = if texture_only_preview {
            // Keep the notice semantic without hard-coding a blue surface:
            // the active theme's primary weak color gives Everforest and
            // custom themes their own accent while the base surface keeps it
            // legible in both light and dark palettes.
            let theme = self.theme();
            let palette = theme.extended_palette();
            let background = iced::theme::palette::mix(
                palette.background.base.color,
                palette.primary.weak.color,
                0.32,
            );
            w::badge(
                "Texture-only preview".to_string(),
                background,
                w::readable_text_color(background, palette.background.base.text),
            )
            .into()
        } else {
            Space::new().into()
        };
        let grid_toggle = w::styled_tooltip(
            checkbox(self.show_texture_grid)
                .label("Grid")
                .on_toggle(Message::ViewTextureGridToggled),
            fonts::caption("Show a reference grid over the texture preview."),
            tooltip::Position::Top,
        );
        let mut grid_size_row = Row::new().spacing(3).align_y(Alignment::Center);
        for divisions in crate::config::ALLOWED_GRID_DIVISIONS {
            let size_button = button(fonts::caption(format!("{divisions}×{divisions}")))
                .on_press(Message::ViewTextureGridSize(divisions))
                .padding([3.0, 6.0]);
            let size_button = if divisions == self.texture_grid_divisions {
                size_button.style(button::primary)
            } else {
                size_button.style(button::text)
            };
            grid_size_row = grid_size_row.push(w::styled_tooltip(
                size_button,
                fonts::caption(format!(
                    "Use a {divisions}×{divisions} reference grid for the texture."
                )),
                tooltip::Position::Top,
            ));
        }
        col = col.push(
            row![
                texture_only_notice,
                uv_toggle,
                uv_status,
                grid_toggle,
                fonts::body("Size:"),
                grid_size_row
            ]
            .spacing(6)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .wrap(),
        );

        // Lazily build the Iced image handle once per texture and cache it on
        // the decoded texture. This avoids cloning the full RGBA buffer on every
        // frame while the texture tab is open.
        let handle = tex
            .handle
            .get_or_init(|| image::Handle::from_rgba(tex.width, tex.height, tex.rgba.clone()))
            .clone();
        let image_viewport = crate::ui::texture_preview::TextureViewport {
            handle,
            image_width: tex.width,
            image_height: tex.height,
            render_image: true,
            show_grid: self.show_texture_grid,
            grid_divisions: self.texture_grid_divisions,
            show_uv: self.show_texture_uv && !uv_triangles.is_empty(),
            uv_triangles,
        };
        let overlay_viewport = crate::ui::texture_preview::TextureViewport {
            render_image: false,
            ..image_viewport.clone()
        };
        // Both layers must occupy the exact same panel bounds. `Canvas` uses
        // a fixed intrinsic size by default, so constraining only the parent
        // stack would leave the image and overlay in a small corner.
        let image_layer = canvas::Canvas::new(image_viewport)
            .width(Length::Fill)
            .height(Length::Fill);
        let overlay_layer = canvas::Canvas::new(overlay_viewport)
            .width(Length::Fill)
            .height(Length::Fill);
        let preview: Element<'_, Message> = stack(vec![image_layer.into(), overlay_layer.into()])
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
        col = col.push(preview);
        col.into()
    }

    fn build_viewer3d_toolbar(
        &self,
        is_model: bool,
        scene_matches: bool,
        loading: bool,
    ) -> Element<'_, Message> {
        if !is_model {
            return Space::new().height(Length::Fixed(28.0)).into();
        }
        if !scene_matches {
            if loading {
                return row![
                    w::icon_label(icons::model().size(14), fonts::caption("3D:")),
                    fonts::caption("Preparing selected model…"),
                ]
                .spacing(4)
                .padding(2)
                .width(Length::Fill)
                .into();
            }
            return row![
                w::icon_label(icons::model().size(14), fonts::caption("3D:")),
                w::styled_tooltip(
                    button(w::icon_label(
                        icons::refresh().size(14),
                        fonts::caption("Load selected"),
                    ))
                    .on_press(Message::Viewer3dLoadSelected)
                    .height(Length::Fixed(28.0)),
                    fonts::caption("Load the selected model into the 3D viewer."),
                    tooltip::Position::Bottom,
                ),
            ]
            .spacing(4)
            .padding(2)
            .width(Length::Fill)
            .into();
        }
        let (flags, origin_mode, has_textures) = self.viewer3d_handle.with(|i| {
            (
                i.flags,
                i.origin_mode,
                i.scene
                    .as_deref()
                    .is_some_and(|scene| scene.textured_mesh_count() > 0),
            )
        });
        let button_height = Length::Fixed(28.0);
        let mut row = Row::new().spacing(4).padding(2).width(Length::Fill);
        row = row.push(w::icon_label(
            icons::model().size(14),
            fonts::caption("3D:"),
        ));
        row = row.push(w::styled_tooltip(
            button(w::icon_label(
                icons::refresh().size(14),
                fonts::caption("Reset view"),
            ))
            .on_press(Message::Viewer3dReset)
            .height(button_height),
            fonts::caption("Re-fit the camera to the model. Shortcut: R"),
            tooltip::Position::Right,
        ));
        row = row.push(w::styled_tooltip(
            button(w::icon_label(
                icons::close().size(14),
                fonts::caption("Clear"),
            ))
            .on_press(Message::Viewer3dClear)
            .height(button_height),
            fonts::caption("Drop the loaded scene"),
            tooltip::Position::Right,
        ));
        row = row.push(w::styled_tooltip(
            checkbox(flags.contains(RenderFlags::WIREFRAME))
                .label("Wire overlay")
                .on_toggle(|_| Message::Viewer3dToggleWireframe),
            fonts::caption("Show triangle edges over the shaded model."),
            tooltip::Position::Bottom,
        ));
        row = row.push(w::styled_tooltip(
            checkbox(flags.contains(RenderFlags::CULL_BACK))
                .label("Cull backfaces")
                .on_toggle(|_| Message::Viewer3dToggleCullBackfaces),
            fonts::caption("Hide back-facing triangles to inspect surface winding."),
            tooltip::Position::Bottom,
        ));
        row = row.push(w::styled_tooltip(
            checkbox(flags.contains(RenderFlags::HAS_TEXTURE))
                .label("Textured")
                .on_toggle(|_| Message::Viewer3dToggleTextured),
            fonts::caption("Use the model's decoded textures instead of a neutral material."),
            tooltip::Position::Bottom,
        ));
        let alpha_available = flags.contains(RenderFlags::HAS_TEXTURE) && has_textures;
        let alpha_checked = alpha_available && flags.contains(RenderFlags::ALPHA_BLEND);
        let alpha_hint = if alpha_available {
            "Respect texture alpha for cutouts and transparent materials."
        } else {
            "Enable Textured on a model with textures to use alpha blending."
        };
        row = row.push(w::styled_tooltip(
            checkbox(alpha_checked)
                .label("Alpha blend")
                .on_toggle_maybe(alpha_available.then_some(|_| Message::Viewer3dToggleAlphaBlend)),
            fonts::caption(alpha_hint),
            tooltip::Position::Bottom,
        ));
        row = row.push(w::styled_tooltip(
            checkbox(origin_mode == SceneOriginMode::Centered)
                .label("Center origin")
                .on_toggle(|_| Message::Viewer3dToggleCenterOrigin),
            fonts::caption(
                "Recenter the model for inspection; disable to preserve world coordinates.",
            ),
            tooltip::Position::Bottom,
        ));
        row = row.push(w::styled_tooltip(
            checkbox(flags.contains(RenderFlags::SHOW_GRID))
                .label("Grid floor")
                .on_toggle(|_| Message::Viewer3dToggleGrid),
            fonts::caption("Show the world reference grid and XYZ axes."),
            tooltip::Position::Bottom,
        ));
        row.wrap().vertical_spacing(4).into()
    }

    fn build_inspection_panel(
        inspection: &EntryInspection,
        literal_types: bool,
    ) -> Element<'_, Message> {
        let mut panel = Column::new().spacing(4);

        panel = panel.push(label_value_owned("Name", inspection.file_name.to_string()));
        let type_label = if literal_types {
            literal_type_label(&inspection.file_name)
        } else {
            inspection.file_type.to_string()
        };
        panel = panel.push(label_value_owned("Type", type_label));

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
            panel = panel.push(
                Space::new()
                    .width(Length::Fixed(0.0))
                    .height(Length::Fixed(6.0)),
            );
            for (key, value) in &inspection.summary {
                panel = panel.push(label_value_owned(key, value.to_string()));
            }
        }

        if let Some(preview) = &inspection.preview_hex {
            panel = panel.push(
                Space::new()
                    .width(Length::Fixed(0.0))
                    .height(Length::Fixed(6.0)),
            );
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
        let design = self.design();

        // Build the status text: left side.
        let selected_count = self.editor.selected_archive().map_or(0, |idx| {
            self.editor
                .archives()
                .get(idx)
                .map_or(0, |a| a.entries.iter().filter(|e| e.selected).count())
        });

        let left_text = if self.toast.is_some() {
            self.toast.clone().unwrap_or_default()
        } else if selected_count > 0 {
            format!("Selected: {selected_count}")
        } else {
            format!(
                "{} v{}",
                crate::ui::theme::APP_NAME,
                env!("CARGO_PKG_VERSION")
            )
        };

        // Animate a smooth transition between the normal surface color
        // and a success-green tint when a toast is active.
        let normal_bg = design.chrome();
        let toast_bg = design.success_gradient().0;
        let mix = self
            .animator
            .get(crate::ui::app::ANIM_TOAST_OPACITY)
            .clamp(0.0, 1.0);
        let toast_mix = mix * TOAST_BACKGROUND_MAX;
        let peak_bg = Color {
            r: normal_bg.r + (toast_bg.r - normal_bg.r) * TOAST_BACKGROUND_MAX,
            g: normal_bg.g + (toast_bg.g - normal_bg.g) * TOAST_BACKGROUND_MAX,
            b: normal_bg.b + (toast_bg.b - normal_bg.b) * TOAST_BACKGROUND_MAX,
            a: 1.0,
        };
        let bg = Color {
            r: normal_bg.r + (toast_bg.r - normal_bg.r) * toast_mix,
            g: normal_bg.g + (toast_bg.g - normal_bg.g) * toast_mix,
            b: normal_bg.b + (toast_bg.b - normal_bg.b) * toast_mix,
            a: 1.0,
        };
        let preferred_status_text = self.theme().extended_palette().background.base.text;
        let status_text_target =
            w::stable_readable_text_color(normal_bg, peak_bg, preferred_status_text);
        let status_text = w::smooth_color_mix(preferred_status_text, status_text_target, mix);

        let bar = Container::new(
            Row::new()
                .push(fonts::caption(left_text).color(status_text))
                .push(Space::new().width(Length::Fill))
                .align_y(Alignment::Center)
                .padding(6),
        )
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: Some(status_text),
            ..Default::default()
        });
        bar.into()
    }
}

fn toolbar_button(
    icon: Element<'static, Message>,
    msg: Message,
) -> iced::widget::Button<'static, Message> {
    // Pin the icon to a fixed 22x22 box centered on both axes inside
    // the 34x34 button; relies on nothing but layout, so every glyph
    // gets the same geometric treatment.
    button(
        container(icon)
            .width(Length::Fixed(22.0))
            .height(Length::Fixed(22.0))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .on_press(msg)
    .padding(6)
    .width(Length::Fixed(34.0))
    .height(Length::Fixed(34.0))
}

fn build_toolbar(accent: Color, bg: Color, divider: Color) -> Element<'static, Message> {
    let toolbar = row![
        w::styled_tooltip(
            toolbar_button(icons::new_archive().size(18).into(), Message::NewArchive),
            fonts::body("New"),
            tooltip::Position::Bottom,
        ),
        w::styled_tooltip(
            toolbar_button(icons::open_archive().size(18).into(), Message::OpenArchive),
            fonts::body("Open"),
            tooltip::Position::Bottom,
        ),
        w::styled_tooltip(
            toolbar_button(icons::save().size(18).into(), Message::SaveArchive),
            fonts::body("Save"),
            tooltip::Position::Bottom,
        ),
        w::styled_tooltip(
            toolbar_button(icons::pack().size(18).into(), Message::PackArchive),
            fonts::body("Pack archive"),
            tooltip::Position::Bottom,
        ),
        w::vhairline(divider),
        w::styled_tooltip(
            toolbar_button(icons::import().size(18).into(), Message::ImportFiles),
            fonts::body("Import"),
            tooltip::Position::Bottom,
        ),
        w::styled_tooltip(
            toolbar_button(icons::open_archive().size(18).into(), Message::ImportFolder),
            fonts::body("Import folder"),
            tooltip::Position::Bottom,
        ),
        w::styled_tooltip(
            toolbar_button(icons::export().size(18).into(), Message::ExportSelected),
            fonts::body("Export selected"),
            tooltip::Position::Bottom,
        ),
        w::vhairline(divider),
        w::styled_tooltip(
            toolbar_button(icons::delete().size(18).into(), Message::DeleteSelected),
            fonts::body("Delete selected"),
            tooltip::Position::Bottom,
        ),
        w::vhairline(divider),
        w::styled_tooltip(
            toolbar_button(icons::shield_check().size(18).into(), Message::OpenValidatorPopup),
            fonts::body("Validate textures"),
            tooltip::Position::Bottom,
        ),
    ]
    .spacing(4)
    .padding(4)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    Container::new(
        Row::new()
            .push(w::accent_bar(accent, 42.0))
            .push(toolbar)
            .width(Length::Fill)
            .align_y(Alignment::Center),
    )
    .height(Length::Fixed(42.0))
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(bg)),
        ..Default::default()
    })
    .into()
}

/// Tab label for an archive. When another open archive shares the same
/// file name (III, VC and SA all ship `gta3.img`), the last two parent
/// folders are appended so the tabs stay distinguishable; the tooltip
/// carries the full path.
fn archive_tab_label(
    archive: &crate::archive::ArchiveInfo,
    archives: &[crate::archive::ArchiveInfo],
) -> String {
    let duplicated = archives
        .iter()
        .filter(|other| other.file_name == archive.file_name)
        .count()
        > 1;
    if !duplicated {
        return archive.file_name.clone();
    }
    let mut parts: Vec<String> = Vec::new();
    let mut cursor = archive.path.as_ref().and_then(|path| path.parent());
    while let Some(dir) = cursor {
        if parts.len() == 2 {
            break;
        }
        if let Some(name) = dir.file_name() {
            parts.push(name.to_string_lossy().to_string());
        }
        cursor = dir.parent();
    }
    if parts.is_empty() {
        return archive.file_name.clone();
    }
    parts.reverse();
    format!("{} · {}", archive.file_name, parts.join("\\"))
}

/// Cut a label to fit a tab width, appending an ellipsis. Iced's Text has
/// no ellipsis support in 0.14, so the budget is estimated from the tab
/// font's average glyph width.
fn ellipsize(label: &str, width: f32) -> String {
    const GLYPH_WIDTH: f32 = 7.2;
    const HORIZONTAL_PADDING: f32 = 20.0;
    let budget = (((width - HORIZONTAL_PADDING) / GLYPH_WIDTH).floor() as usize).max(4);
    if label.chars().count() <= budget {
        return label.to_string();
    }
    let mut out: String = label.chars().take(budget - 1).collect();
    out.push('…');
    out
}

/// Divider after the tab strip: drag horizontally to resize the tabs.
fn tab_resize_grip() -> Element<'static, Message> {
    let bar = Container::new(
        Space::new()
            .width(Length::Fixed(3.0))
            .height(Length::Fixed(24.0)),
    )
    .style(|theme: &iced::Theme| iced::widget::container::Style {
        background: Some(theme.extended_palette().background.strong.color.into()),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 2.0.into(),
        },
        ..Default::default()
    });
    let padded = container(bar).padding(iced::Padding {
        top: 0.0,
        right: 4.0,
        bottom: 0.0,
        left: 4.0,
    });
    mouse_area(padded)
        .on_press(Message::TabResizeStarted)
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .into()
}

pub fn build(app: &App) -> Element<'_, Message> {
    let design = app.design();
    let tab_surface = design.chrome();
    let page_bg = design.page();
    let empty_state_accent = design.accent();
    let menubar = app.menubar();
    let toolbar = build_toolbar(design.accent(), design.chrome(), design.divider());

    let tab_bar: Element<'_, Message> = if app.editor.archives().is_empty() {
        Space::new().height(Length::Fixed(0.0)).into()
    } else {
        let selected = app.editor.selected_archive().unwrap_or(0);
        let tab_width = app.archive_tab_width.clamp(
            crate::config::ARCHIVE_TAB_WIDTH_MIN,
            crate::config::ARCHIVE_TAB_WIDTH_MAX,
        );
        let mut tab_rows = Vec::new();
        for (index, archive) in app.editor.archives().iter().enumerate() {
            let is_selected = index == selected;
            let display_name = archive_tab_label(archive, app.editor.archives());
            let label = if archive.dirty {
                format!("● {display_name}")
            } else {
                display_name
            };
            let label = ellipsize(&label, tab_width);
            let tab_pulse = app.archive_tab_selection_pulse(index);
            let tab = button(fonts::body(label))
                .on_press(Message::SelectArchiveTab(index))
                .width(Length::Fixed(tab_width))
                .style(move |theme, status| {
                    let mut style = if is_selected {
                        button::primary(theme, status)
                    } else {
                        button::secondary(theme, status)
                    };
                    if is_selected
                        && tab_pulse > 0.0
                        && let Some(iced::Background::Color(background)) = style.background
                    {
                        let background = iced::theme::palette::mix(
                            background,
                            theme.extended_palette().primary.strong.color,
                            tab_pulse * 0.38,
                        );
                        style.background = Some(background.into());
                        style.text_color = w::readable_text_color(background, style.text_color);
                    }
                    style
                });
            // Accent bar on the left of the active tab
            let tab: Element<'_, Message> = if is_selected {
                Row::new()
                    .push(w::accent_bar(design.accent(), 32.0))
                    .push(tab)
                    .align_y(Alignment::Center)
                    .into()
            } else {
                tab.into()
            };
            // The tooltip carries the full path so the folder suffix in
            // the label never has to be decoded by hovering guesses.
            let tab: Element<'_, Message> = match archive.path.as_ref() {
                Some(path) => w::styled_tooltip(
                    tab,
                    fonts::caption(path.display().to_string()),
                    tooltip::Position::Bottom,
                )
                .into(),
                None => tab,
            };
            tab_rows.push(
                mouse_area(tab)
                    .on_middle_press(Message::CloseArchiveTab(index))
                    .into(),
            );
        }
        let row = Row::with_children(tab_rows).spacing(4).padding(4);
        let row: Element<'_, Message> =
            if let Some(ripple) = app.ripple_visual(RippleTarget::ArchiveTab(selected)) {
                stack(vec![
                    row.into(),
                    interaction::ripple_overlay::<Message>(ripple)
                        .width(Length::Fill)
                        .height(Length::Fixed(40.0))
                        .into(),
                ])
                .width(Length::Fill)
                .height(Length::Fixed(40.0))
                .clip(true)
                .into()
            } else {
                row.into()
            };
        Container::new(
            Row::new()
                .push(row)
                .push(Space::new().width(Length::Fill))
                .push(tab_resize_grip())
                .align_y(Alignment::Center)
                .height(Length::Fixed(40.0)),
        )
        .width(Length::Fill)
        .height(Length::Fixed(40.0))
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(tab_surface)),
            ..Default::default()
        })
        .into()
    };

    let body: Element<'_, Message> = if app.editor.archives().is_empty() {
        // Idle "breathing" on the hero icon: a slow scale + bob driven by the
        // animation ticker. Float renders through Iced's window-level overlay
        // layer, which paints above the modal stack — so whenever a dialog is
        // on screen the icon must fall back to a plain inline widget or it
        // would draw on top of the dialog.
        let modal_open = app.modal_open();
        let hero_icon: Element<'_, Message> = if app.config.motion_enabled && !modal_open {
            let phase = app.empty_state_phase * std::f32::consts::TAU;
            // Gentle idle drift: scale and bob are a quarter-turn out of
            // phase (circular motion) with a low amplitude, so the icon
            // floats instead of throbbing.
            Float::new(icons::archive().size(42).color(empty_state_accent))
                .scale(1.0 + phase.cos() * 0.018)
                .translate(move |_, _| Vector::new(0.0, phase.sin() * 1.4))
                .into()
        } else {
            icons::archive().size(42).color(empty_state_accent).into()
        };
        Container::new(
            column![
                Space::new().height(Length::Fill),
                hero_icon,
                Space::new().height(Length::Fixed(8.0)),
                fonts::display("Open or create an archive to get started."),
                Space::new().height(Length::Fixed(8.0)),
                fonts::caption("Or drag and drop an .img file here to open it."),
                Space::new().height(Length::Fill),
            ]
            .align_x(Alignment::Center),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    } else {
        let show_search = app.config.show_search_bar;
        let search_strip: Option<Element<'_, Message>> = show_search.then(|| {
            // The label column is pinned so the text input's x position
            // is deterministic: SEARCH_DROPDOWN_X anchors the floating
            // dropdown under the input, not under the strip's left edge.
            let label = container(
                row![
                    icons::search().size(15),
                    fonts::header("Search:"),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .width(Length::Fixed(SEARCH_LABEL_WIDTH))
            .align_y(Alignment::Center);
            let search_input = text_input("", &app.search)
                .id(iced::widget::Id::new("search_input"))
                .on_input(Message::SearchChanged)
                .width(Length::Fill);
            let mut search = row![mouse_area(label).on_press(Message::FocusSearchInput), search_input]
                .spacing(8)
                .padding([6, 8])
                .height(Length::Fill)
                .align_y(Alignment::Center);
            // Trailing clear button: wipes the query and refocuses the
            // input. Only rendered when there is something to clear.
            if !app.search.is_empty() {
                let clear_icon = container(icons::close().size(14))
                    .width(Length::Fixed(20.0))
                    .height(Length::Fixed(20.0))
                    .center_x(Length::Fill)
                    .center_y(Length::Fill);
                let hover_bg = with_alpha(design.accent(), 0.22);
                search = search.push(
                    button(clear_icon)
                        .on_press(Message::ClearSearch)
                        .padding(0)
                        .width(Length::Fixed(22.0))
                        .height(Length::Fixed(22.0))
                        .style(move |theme, status| {
                            let highlighted = matches!(
                                status,
                                button::Status::Hovered | button::Status::Pressed
                            );
                            let palette = theme.extended_palette();
                            iced::widget::button::Style {
                                background: highlighted
                                    .then_some(iced::Background::Color(hover_bg)),
                                text_color: palette.background.base.text,
                                border: Border {
                                    color: Color::TRANSPARENT,
                                    width: 0.0,
                                    radius: 4.0.into(),
                                },
                                ..Default::default()
                            }
                        }),
                );
            }

            let search_bg = design.chrome();
            Container::new(search)
                .width(Length::Fill)
                .height(Length::Fixed(SEARCH_STRIP_HEIGHT))
                .style(move |_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(search_bg)),
                    ..Default::default()
                })
                .into()
        });

        let pane_surface = design.surface();
        let split_divider = design.divider();
        let split_accent = design.accent();
        let main_row = pane_grid(&app.panes, move |_pane, state, _is_maximized| {
            let content: Element<'_, Message> = match state {
                Pane::Table => app.build_entry_table(),
                Pane::Info => app.build_info_panel(),
            };
            pane_grid::Content::new(
                Container::new(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(move |_| iced::widget::container::Style {
                        background: Some(iced::Background::Color(pane_surface)),
                        ..Default::default()
                    }),
            )
        })
        .on_resize(10, Message::PaneResized)
        .spacing(3)
        .style(move |_| pane_grid::Style {
            hovered_region: pane_grid::Highlight {
                background: iced::Background::Color(Color::TRANSPARENT),
                border: iced::Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 0.0.into(),
                },
            },
            hovered_split: pane_grid::Line {
                color: split_divider,
                width: 3.0,
            },
            picked_split: pane_grid::Line {
                color: split_accent,
                width: 3.0,
            },
        })
        .height(Length::Fill);

        match search_strip {
            Some(strip) => {
                let open = app.predictions_open();
                let (accent, surface, divider) =
                    (design.accent(), design.surface(), design.divider());
                // The Float is an overlay child of the workspace stack:
                // it renders above the pane grid at the translated
                // position and still receives clicks there, while the
                // grid below keeps its exact layout. The stack + Float
                // are ALWAYS present (the card is an empty placeholder
                // when closed) so toggling predictions never reshapes
                // the widget tree — reshaping would drop the text
                // input's focus state.
                let dropdown =
                    Float::new(responsive(move |size| {
                        if open {
                            search_prediction_dropdown(
                                app,
                                size.width - SEARCH_DROPDOWN_X,
                                accent,
                                surface,
                                divider,
                            )
                        } else {
                            container(column![]).into()
                        }
                    }))
                    .translate(move |bounds, viewport| {
                        if !open {
                            return Vector::ZERO;
                        }
                        // Anchor just below the search strip and inside
                        // the window on short viewports.
                        let y = if bounds.height + SEARCH_STRIP_HEIGHT > viewport.height {
                            (viewport.height - bounds.height).max(0.0)
                        } else {
                            SEARCH_STRIP_HEIGHT
                        };
                        Vector::new(SEARCH_DROPDOWN_X, y)
                    });
                stack(vec![column![strip, main_row].into(), dropdown.into()]).into()
            }
            None => main_row.into(),
        }
    };

    let status = app.build_status_bar();
    let base = column![
        menubar,
        w::hairline(design.divider()),
        toolbar,
        w::hairline(design.divider()),
        tab_bar,
        w::hairline(design.divider()),
        body,
        w::hairline(design.divider()),
        status,
    ]
    .spacing(0)
    .width(Length::Fill)
    .height(Length::Fill);

    let overlays: Vec<Element<'_, Message>> = vec![
        build_about(app),
        build_welcome(app),
        build_unsupported(app),
        build_folder_import(app),
        build_import_preflight(app),
        build_save_report(app),
        build_unsaved_dialog(app),
        build_update_status(app),
        build_sort_manager(app),
        build_toast_overlay(app),
        build_validator_popup(app),
        build_quit_fade(app),
    ]
    .into_iter()
    .flatten()
    .collect();

    // Keep the root widget shape stable while overlays appear and disappear.
    // Changing from `Container(base)` to `Stack(base, overlay...)` makes Iced
    // recreate the whole state tree because their tags differ. That silently
    // resets every nested Scrollable and also destroys the native
    // Scrollable::AutoScrolling interaction whenever a toast is shown or
    // dismissed. The base layer must therefore always occupy stack slot 0;
    // transient notifications can be appended without moving it.
    let mut layers: Vec<Element<'_, Message>> = vec![
        Container::new(base)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(page_bg)),
                ..Default::default()
            })
            .into(),
    ];
    layers.extend(overlays);
    stack(layers)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn build_about(app: &App) -> Option<Element<'_, Message>> {
    if !app.show_about {
        return None;
    }

    let about_content = column![
        logo_element(),
        Space::new().height(Length::Fixed(8.0)),
        fonts::body(ABOUT_TEXT)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
        Space::new().height(Length::Fixed(8.0)),
        row![
            button(w::icon_label(
                icons::external_viewer().size(14),
                fonts::body("Visit repository"),
            ))
            .on_press(Message::VisitRepository)
            .style(button::primary),
            button(w::icon_label(icons::close().size(14), fonts::body("Close")))
                .on_press(Message::HideAbout),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ]
    .spacing(6)
    .width(Length::Fill)
    .align_x(Alignment::Center);

    Some(modal_box(
        "About",
        container(about_content)
            .width(Length::Fixed(400.0))
            .align_x(iced::alignment::Horizontal::Center),
    ))
}

fn build_welcome(app: &App) -> Option<Element<'_, Message>> {
    if !app.show_welcome {
        return None;
    }
    Some(modal_box(
        "Welcome",
        column![
            container(logo_element())
                .width(Length::Fixed(350.0))
                .align_x(iced::alignment::Horizontal::Center),
            Space::new().height(Length::Fixed(8.0)),
            fonts::display(format!(
                "Welcome to {} v{}",
                crate::ui::theme::APP_NAME,
                env!("CARGO_PKG_VERSION")
            )),
            fonts::body("A GTA archive editor for III, VC, San Andreas, Bully SE."),
            Space::new().height(Length::Fixed(8.0)),
            checkbox(app.welcome_persist)
                .label("Don't show this message again")
                .on_toggle(Message::ToggleWelcomePersist),
            checkbox(!app.config.update_check_enabled)
                .label("Disable update checking")
                .on_toggle(Message::ToggleUpdateDisabled),
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

/// Pre-save report: what the archive contains relative to its target,
/// shown only when something needs a decision. The save itself stays
/// verbatim - this dialog blocks nothing but the write.
fn build_save_report(app: &App) -> Option<Element<'_, Message>> {
    let pending = app.pending_save.as_ref()?;
    let issue = pending.issue.as_ref()?;
    let target = app
        .editor
        .archives()
        .get(pending.index)
        .and_then(|archive| archive.target_game)
        .and_then(crate::compat::games::profile_by_id)
        .map(|game| game.display)
        .unwrap_or("the target");

    let mut body = Column::new().spacing(6).width(Length::Fill);
    body = body.push(fonts::body(format!(
        "{} textures, {} entries - checked against {target}.",
        issue.textures, issue.entry_count
    )));
    body = body.push(fonts::caption(format!(
        "{} native/supported · {} convertible (lossless) · {} incompatible · {} unknown",
        issue.fine, issue.convertible, issue.incompatible, issue.unknown
    )));

    if let Some(note) = &issue.container_note {
        body = body.push(
            fonts::body(format!("Container: {note}"))
                .color(compat_verdict_accent(crate::compat::games::Verdict::Unsupported)),
        );
    }

    if !issue.anomalies.is_empty() {
        let mut list = Column::new().spacing(3).width(Length::Fill);
        for (code, count, example) in issue.anomalies.iter().take(8) {
            list = list.push(fonts::caption(format!("{code}: {count} (e.g. {example})")));
        }
        body = body.push(Space::new().height(Length::Fixed(4.0)));
        body = body.push(fonts::strong("Broken headers (fixable without re-encoding):"));
        body = body.push(list);
    }
    if issue.warnings > 0 {
        body = body.push(fonts::caption(format!(
            "{} warning-level anomalies (reported, not blocking).",
            issue.warnings
        )));
    }

    body = body.push(Space::new().height(Length::Fixed(4.0)));
    body = body.push(fonts::caption(
        "Saving writes every entry verbatim; no texture is re-encoded or converted.",
    ));
    body = body.push(Space::new().height(Length::Fixed(8.0)));
    body = body.push(
        row![
            button(fonts::body("Save anyway"))
                .on_press(Message::SaveCheckConfirmed)
                .style(button::primary),
            button(fonts::body("Cancel")).on_press(Message::SaveCheckCancelled),
        ]
        .spacing(8),
    );

    Some(modal_box(
        "Save check",
        container(body).width(Length::Fixed(480.0)),
    ))
}

/// Unsaved-changes guard: shown when closing a dirty archive or quitting
/// with dirty archives open. The files on disk are untouched either way;
/// the dialog only protects the in-memory edits.
fn build_unsaved_dialog(app: &App) -> Option<Element<'_, Message>> {
    let pending = app.pending_close?;
    match pending {
        crate::ui::app::PendingClose::Archive(index) => {
            let archive = app.editor.archives().get(index)?;
            let body = column![
                fonts::body(format!("'{}' has unsaved changes.", archive.file_name)),
                fonts::caption(
                    "Closing without saving discards them; the archive file on disk is untouched."
                ),
                Space::new().height(Length::Fixed(8.0)),
                row![
                    button(fonts::strong("Save"))
                        .on_press(Message::CloseGuardSave)
                        .style(button::primary),
                    button(fonts::body("Discard")).on_press(Message::CloseGuardDiscard),
                    button(fonts::body("Cancel")).on_press(Message::CloseGuardCancel),
                ]
                .spacing(8),
            ]
            .spacing(6)
            .width(Length::Fill);
            Some(modal_box(
                "Unsaved changes",
                container(body).width(Length::Fixed(440.0)),
            ))
        }
        crate::ui::app::PendingClose::Window(_) => {
            let dirty: Vec<String> = app
                .editor
                .archives()
                .iter()
                .filter(|archive| archive.dirty)
                .map(|archive| archive.file_name.clone())
                .collect();
            let body = column![
                fonts::body(format!(
                    "{} archive(s) have unsaved changes: {}",
                    dirty.len(),
                    dirty.join(", ")
                )),
                fonts::caption("Quitting now discards them; the files on disk are untouched."),
                Space::new().height(Length::Fixed(8.0)),
                row![
                    button(fonts::strong("Discard changes and quit"))
                        .on_press(Message::CloseGuardDiscard)
                        .style(button::primary),
                    button(fonts::body("Cancel")).on_press(Message::CloseGuardCancel),
                ]
                .spacing(8),
            ]
            .spacing(6)
            .width(Length::Fill);
            Some(modal_box(
                "Unsaved changes",
                container(body).width(Length::Fixed(440.0)),
            ))
        }
    }
}

/// Pre-flight import dialog: lists the files whose formats the target
/// engine cannot consume and lets the user import anyway or cancel.
fn build_import_preflight(app: &App) -> Option<Element<'_, Message>> {
    let pending = app.pending_import.as_ref()?;
    let target = app
        .editor
        .archives()
        .get(pending.index)
        .and_then(|archive| archive.target_game)
        .and_then(crate::compat::games::profile_by_id)
        .map(|game| game.display)
        .unwrap_or("the selected target");

    let flagged = pending.flagged();
    let total_files = pending.paths.len();
    let incompatible: usize = flagged.iter().map(|check| check.incompatible()).sum();
    let unknown: usize = flagged.iter().map(|check| check.unknown()).sum();
    let mut lines = Column::new().spacing(4).width(Length::Fill);
    for check in flagged.iter().take(12) {
        let detail = check
            .offenders
            .first()
            .map(|(name, verdict, note)| {
                if note.is_empty() {
                    format!("{name}: {}", verdict.label())
                } else {
                    format!("{name}: {} - {note}", verdict.label())
                }
            })
            .unwrap_or_default();
        lines = lines.push(column![
            fonts::strong(check.file_name.clone()),
            fonts::caption(format!(
                "{} texture(s): {}",
                check.textures,
                detail
            )),
        ]
        .spacing(1));
    }
    if flagged.len() > 12 {
        lines = lines.push(fonts::caption(format!(
            "...and {} more flagged file(s).",
            flagged.len() - 12
        )));
    }

    let body = column![
        fonts::body(format!(
            "{} of {} file(s) need a decision for {target}: {incompatible} incompatible texture(s), {unknown} unknown.",
            flagged.len(),
            total_files
        )),
        Space::new().height(Length::Fixed(4.0)),
        lines,
        Space::new().height(Length::Fixed(4.0)),
        fonts::caption(
            "Imports are verbatim either way - the format only matters if the game must load these textures."
        ),
        Space::new().height(Length::Fixed(8.0)),
        row![
            button(fonts::body("Import anyway")
                .color(compat_verdict_accent(crate::compat::games::Verdict::Unsupported)))
            .on_press(Message::ImportCheckConfirmed)
            .style(button::primary),
            button(fonts::body("Cancel import")).on_press(Message::ImportCheckCancelled),
        ]
        .spacing(8),
    ]
    .spacing(4)
    .width(Length::Fill);

    Some(modal_box(
        "Import check",
        container(body).width(Length::Fixed(460.0)),
    ))
}

fn build_folder_import(app: &App) -> Option<Element<'_, Message>> {
    let (_, plan) = app.pending_folder_import.as_ref()?;
    let duplicate_text = if plan.duplicate_count == 0 {
        "No duplicate names detected.".to_string()
    } else {
        format!(
            "{} duplicate name(s) detected. Choose how to handle them.",
            plan.duplicate_count
        )
    };
    let scan_text = if plan.scan_skipped.is_empty() {
        None
    } else {
        Some(format!(
            "{} item(s) could not be inspected and will be skipped.",
            plan.scan_skipped.len()
        ))
    };

    let mut actions = Row::new().spacing(8).push(
        button(fonts::body(if plan.duplicate_count == 0 {
            "Import files"
        } else {
            "Import (skip duplicates)"
        }))
        .on_press(Message::ConfirmFolderImport(FolderDuplicatePolicy::Skip))
        .style(button::primary),
    );
    if plan.duplicate_count > 0 {
        actions = actions.push(
            button(fonts::body("Replace duplicates"))
                .on_press(Message::ConfirmFolderImport(FolderDuplicatePolicy::Replace)),
        );
    }
    actions = actions.push(button(fonts::body("Cancel")).on_press(Message::CancelFolderImport));

    let mut content = column![
        fonts::body(format!("Folder: {}", plan.folder.display())),
        fonts::body(format!(
            "{} regular file(s) • {}",
            plan.files.len(),
            crate::ui::app::format_byte_count(plan.total_bytes)
        )),
        fonts::caption(
            "Only files directly inside this folder are included; subfolders are not scanned."
        ),
        fonts::caption(duplicate_text),
    ]
    .spacing(6);
    if let Some(scan_text) = scan_text {
        content = content.push(fonts::caption(scan_text));
    }
    content = content
        .push(Space::new().height(Length::Fixed(8.0)))
        .push(actions);

    Some(modal_box("Import folder", content))
}

fn build_update_status(app: &App) -> Option<Element<'_, Message>> {
    let msg = app.show_update_status.clone()?;
    Some(modal_box(
        "Update check",
        column![
            fonts::body(msg),
            Space::new().height(Length::Fixed(8.0)),
            checkbox(app.config.update_notify_disabled)
                .label("Do not show this message again")
                .on_toggle(Message::ToggleUpdateNotifyDisabled),
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

/// `static` empty maps for the IDE/COL fallback. The dialog only presents
/// in-memory preview data, so IDE/COL labels fall back to their normal name
/// comparison when no catalog is loaded here.
static EMPTY_IDE_MAP: std::sync::LazyLock<
    std::collections::HashMap<compact_str::CompactString, compact_str::CompactString>,
> = std::sync::LazyLock::new(std::collections::HashMap::new);
static EMPTY_COL_MAP: std::sync::LazyLock<
    std::collections::HashMap<compact_str::CompactString, compact_str::CompactString>,
> = std::sync::LazyLock::new(std::collections::HashMap::new);

fn build_sort_manager(app: &App) -> Option<Element<'_, Message>> {
    if !app.show_sort_manager {
        return None;
    }
    let draft = app.sort_draft.as_ref()?;

    let archive = app
        .editor
        .selected_archive()
        .and_then(|idx| app.editor.archives().get(idx));
    let archive_name = archive.map(|archive| archive.file_name.as_str());
    let preview_entries = archive.map_or(&[][..], |archive| archive.entries.as_slice());
    let design = app.design();

    let dialog = crate::ui::sort_manager::build(
        archive_name,
        draft,
        crate::ui::sort_manager::SortPreview {
            entries: preview_entries,
            primary_type: None, // populated for the table view, not the dialog
            literal_types: app.config.literal_file_types,
            ide_labels: &EMPTY_IDE_MAP,
            col_labels: &EMPTY_COL_MAP,
        },
        &design,
    );

    let surface = design.surface();
    let text_color = design.text();
    let border = design.border();
    let shadow = design.iced_shadow(&design.tokens.elevation.modal);
    let card = Container::new(dialog)
        .width(Length::Fill)
        .max_width(880.0)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(surface)),
            text_color: Some(text_color),
            border: Border {
                color: border,
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow,
            ..Default::default()
        });

    // The full-window opaque layer keeps the entry table inert while the
    // draft is being edited. The card itself has a capped width, so it stays
    // centered and never reflows into the file list like an inline panel.
    Some(
        opaque(
            Container::new(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(24)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(Color::from_rgba(
                        0.0, 0.0, 0.0, 0.42,
                    ))),
                    ..Default::default()
                }),
        ),
    )
}

/// Opaque black layer that fades in just before the window closes.
/// Teardown can briefly composite whatever the driver/DWM leaves in the
/// swapchain; ending on black keeps that invisible (and avoids flashing
/// a light buffer at low-light users). Defaults to opaque once the fade
/// animation is done or reaped, so the layer never lifts early.
fn build_quit_fade(app: &App) -> Option<Element<'_, Message>> {
    if app.quitting.is_none() {
        return None;
    }
    let alpha = app
        .animator
        .get_or(crate::ui::app::ANIM_QUIT_FADE, 1.0)
        .clamp(0.0, 1.0);
    Some(
        container(Space::new().width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(Color::from_rgba(
                    0.0, 0.0, 0.0, alpha,
                ))),
                ..Default::default()
            })
            .into(),
    )
}

/// Centered modal card. `content` must establish its own width (a
/// fixed-width container, like the other dialogs): the card column is
/// `Length::Shrink`, and Fill-width content inside it collapses to an
/// empty rounded box with no buttons.
fn modal_box<'a>(title: &'a str, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    let content: Element<'a, Message> = content.into();
    let content = column![
        fonts::display(title)
            .align_x(iced::alignment::Horizontal::Center)
            .width(Length::Fill),
        content,
    ]
    .spacing(8)
    .padding(16)
    .max_width(480)
    .width(Length::Shrink)
    .align_x(Alignment::Center);
    // Build a floating card with the design-system colors.
    // We use static defaults here because modal_box is called from a
    // non-App context (Element builder). The design system colors tied
    // to a live App would need App::design() passed in.
    let card =
        Container::new(content).style(move |theme: &iced::Theme| iced::widget::container::Style {
            background: Some(theme.extended_palette().background.base.color.into()),
            border: Border {
                color: theme.extended_palette().background.strong.color,
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 16.0,
            },
            ..Default::default()
        });

    let card_element: Element<'_, Message> = card.into();
    Container::new(card_element)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

fn build_validator_popup(app: &App) -> Option<Element<'_, Message>> {
    if !app.validator_popup_open {
        return None;
    }
    let archive_index = app.editor.selected_archive()?;
    let archive = app.editor.archives().get(archive_index)?;
    // Explicit target first; the last run's target is only a fallback for
    // archives validated before the target became persistent.
    let current_target = archive
        .target_game
        .or_else(|| archive.compat_report.as_ref().and_then(|report| report.target));
    let highlight_enabled = app.compat_highlight_enabled;

    let design = app.design();
    let surface = design.surface();
    let text_color = design.text();
    let border_color = design.border();
    let shadow = design.iced_shadow(&design.tokens.elevation.modal);
    let divider = design.divider();

    // `responsive` bounds the card to the window: the game list scrolls
    // when it does not fit, and the header (with the X) and footer (with
    // Close) stay reachable at any window size.
    Some(opaque(responsive(move |size| {
        let max_height = (size.height - 96.0).max(260.0);

        // Right padding keeps the cards (and their Validate buttons) a
        // clear step away from the scrollbar rail.
        let mut cards = Column::new()
            .spacing(10)
            .width(Length::Fill)
            .padding(iced::Padding { right: 16.0, ..Default::default() });
        for game in crate::compat::games::ALL_GAMES {
            let mut native_lines = Column::new().spacing(2).width(Length::Fill);
            let mut unknown_lines = Column::new().spacing(2).width(Length::Fill);
            let mut has_unknown = false;
            for info in crate::compat::games::format_catalog(game) {
                let line = row![
                    fonts::caption(info.class).width(Length::Fixed(210.0)),
                    fonts::caption(info.note).width(Length::Fill),
                ]
                .spacing(8)
                .width(Length::Fill);
                if info.verdict == crate::compat::games::Verdict::Native {
                    native_lines = native_lines.push(line);
                } else {
                    has_unknown = true;
                    unknown_lines = unknown_lines.push(line);
                }
            }

            let last_run = archive
                .compat_report
                .as_ref()
                .filter(|report| report.target == Some(game.id))
                .map(|report| {
                    let counts = report
                        .verdicts
                        .get(game.id)
                        .map(|counts| {
                            counts
                                .iter()
                                .map(|(verdict, count)| format!("{verdict} {count}"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_else(|| "no textures".to_string());
                    format!("Last run: {counts}")
                });

            let mut card_body = Column::new().spacing(6).width(Length::Fill);
            let mut header = row![fonts::strong(game.display).width(Length::Fill)];
            if current_target == Some(game.id) {
                header = header.push(fonts::caption("current target"));
            }
            header = header.push(
                button(fonts::body(format!("Validate for {}", game.display)))
                    .on_press(Message::ValidateArchiveFor(game.id))
                    .style(button::primary),
            );
            card_body = card_body.push(header.spacing(8).align_y(Alignment::Center));
            if let Some(last_run) = last_run {
                card_body = card_body.push(fonts::caption(last_run));
            }
            card_body = card_body.push(fonts::caption("Native (retail-verified):"));
            card_body = card_body.push(native_lines);
            if has_unknown {
                card_body = card_body.push(fonts::caption("Unknown / not game-native:"));
                card_body = card_body.push(unknown_lines);
            }

            cards = cards.push(
                Container::new(card_body)
                    .width(Length::Fill)
                    .padding(10)
                    .style(move |_| iced::widget::container::Style {
                        background: Some(iced::Background::Color(surface)),
                        text_color: Some(text_color),
                        border: Border {
                            color: border_color,
                            width: 1.0,
                            radius: 8.0.into(),
                        },
                        ..Default::default()
                    }),
            );
        }

        let title_row = row![
            fonts::display("Validate textures").width(Length::Fill),
            button(icons::close().size(16))
                .on_press(Message::CloseValidatorPopup)
                .style(button::text),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let introduction = fonts::body(
            "Pick the game this archive targets. The validator flags every texture \
             outside that engine's retail-accepted formats, and lists the unknowns \
             that could plausibly load but are not game-native.",
        )
        .width(Length::Fill);

        // Advisory content hint: a suggestion with its evidence, never
        // applied automatically.
        let hint_row: Option<Element<'static, Message>> = archive.target_hint.as_ref().map(|hint| {
            let display = crate::compat::games::profile_by_id(hint.game_id)
                .map(|game| game.display)
                .unwrap_or(hint.game_id);
            let confidence = match hint.confidence {
                crate::compat::hint::HintConfidence::High => "looks like",
                crate::compat::hint::HintConfidence::Medium => "possibly",
            };
            let mut body = column![
                fonts::body(format!("Content {confidence} {display}"))
                    .color(compat_verdict_accent(crate::compat::games::Verdict::Supported)),
                fonts::caption(hint.reasons.join(" · ")),
            ]
            .spacing(2);
            if archive.target_game != Some(hint.game_id) {
                if let Some(current) = archive.target_game {
                    let current_display = crate::compat::games::profile_by_id(current)
                        .map(|game| game.display)
                        .unwrap_or(current);
                    body = body.push(fonts::caption(format!("current target: {current_display}")));
                }
                body = body.push(
                    button(fonts::body(format!("Use {display} as target")))
                        .on_press(Message::ValidateArchiveFor(hint.game_id))
                        .style(button::primary),
                );
            } else {
                body = body.push(fonts::caption("(already the target)"));
            }
            Container::new(body)
                .width(Length::Fill)
                .padding(8)
                .style(|_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(Color::from_rgba(
                        0.30, 0.60, 0.95, 0.10,
                    ))),
                    border: Border {
                        color: Color::from_rgba(0.30, 0.60, 0.95, 0.30),
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                })
                .into()
        });

        let footer = column![
            compat_legend(size.width),
            row![
                Space::new().width(Length::Fill),
                checkbox(highlight_enabled)
                    .label("Highlight rows")
                    .on_toggle(Message::SetCompatHighlight),
                button(fonts::body("Close")).on_press(Message::CloseValidatorPopup),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(8)
        .width(Length::Fill);

        let mut content = column![
            title_row,
            w::hairline(divider),
            introduction,
        ]
        .spacing(10)
        .width(Length::Fill)
        .align_x(Alignment::Start);
        if let Some(hint_row) = hint_row {
            content = content.push(hint_row);
        }
        let content = content
            .push(
                Scrollable::new(cards)
                    .direction(iced::widget::scrollable::Direction::Vertical(
                        iced::widget::scrollable::Scrollbar::new()
                            .scroller_width(10.0)
                            .margin(3.0),
                    ))
                    .style(validator_scrollbar_style)
                    .height(Length::Fill),
            )
            .push(w::hairline(divider))
            .push(footer);

        let card = Container::new(content)
            .width(Length::Fill)
            .max_width(720.0)
            .max_height(max_height)
            .height(Length::Fill)
            .padding(16)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(surface)),
                text_color: Some(text_color),
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: 12.0.into(),
                },
                shadow,
                ..Default::default()
            });

        Container::new(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(24)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_| iced::widget::container::Style {
                background: Some(iced::Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.42))),
                ..Default::default()
            })
            .into()
    })))
}

/// Floating, accent-tinted scrollbar for the validator popup: a round
/// scroller on a faint rail, no hard borders. Built on Iced's
/// `scrollable::default` so the auto-scroll overlay and hit-testing
/// geometry stay correct.
fn validator_scrollbar_style(
    theme: &iced::Theme,
    status: iced::widget::scrollable::Status,
) -> iced::widget::scrollable::Style {
    let palette = theme.extended_palette();
    let mut style = iced::widget::scrollable::default(theme, status);
    let hovering = !matches!(status, iced::widget::scrollable::Status::Active { .. });
    let scroller_color = if hovering {
        palette.primary.strong.color
    } else {
        palette.primary.weak.color
    };
    let rounded = |radius: f32| Border {
        color: Color::TRANSPARENT,
        width: 0.0,
        radius: radius.into(),
    };
    style.vertical_rail = iced::widget::scrollable::Rail {
        background: Some(
            palette
                .background
                .base
                .color
                .scale_alpha(0.30)
                .into(),
        ),
        border: rounded(6.0),
        scroller: iced::widget::scrollable::Scroller {
            background: scroller_color.into(),
            border: rounded(6.0),
        },
    };
    style
}

/// Accent color for a validator verdict. The row tint and the legend
/// chips both read this, so they can never drift apart.
fn compat_verdict_accent(verdict: crate::compat::games::Verdict) -> Color {
    use crate::compat::games::Verdict;
    match verdict {
        Verdict::Native => Color::from_rgba(0.24, 0.78, 0.44, 1.0),
        Verdict::Supported | Verdict::ConvertibleLossless => {
            Color::from_rgba(0.30, 0.60, 0.95, 1.0)
        }
        Verdict::LossyConvertible => Color::from_rgba(0.95, 0.60, 0.20, 1.0),
        Verdict::Unsupported => Color::from_rgba(0.92, 0.30, 0.30, 1.0),
        Verdict::Untested => Color::from_rgba(0.95, 0.78, 0.25, 1.0),
    }
}

/// Colored icon + label legend for the validator result colors, with a
/// tooltip per entry. Wraps into multiple rows when the card is narrow.
fn compat_legend(max_width: f32) -> Element<'static, Message> {
    use crate::compat::games::Verdict;

    // Icon Text is not Clone in Iced 0.14, so the lookup builds a fresh
    // glyph per chip instead of storing widgets in the entries list.
    let icon_for = |verdict: Verdict| -> iced::widget::Text<'static> {
        match verdict {
            Verdict::Native => icons::verdict_native().size(15),
            Verdict::Supported | Verdict::ConvertibleLossless => icons::verdict_convert().size(15),
            Verdict::LossyConvertible => icons::verdict_lossy().size(15),
            Verdict::Untested => icons::verdict_unknown().size(15),
            Verdict::Unsupported => icons::verdict_incompatible().size(15),
        }
    };

    let entries: [(Verdict, &'static str, &'static str); 5] = [
        (
            Verdict::Native,
            "native",
            "Authored by this engine - no action needed.",
        ),
        (
            Verdict::Supported,
            "supported / convertible",
            "Loads, but is not the game's data dialect; a lossless rewrite may be offered.",
        ),
        (
            Verdict::LossyConvertible,
            "lossy convert",
            "Can be used only after a pixel-changing conversion (compression or quantization).",
        ),
        (
            Verdict::Untested,
            "unknown",
            "No evidence either way - not known to be incompatible. Treat with care.",
        ),
        (
            Verdict::Unsupported,
            "incompatible",
            "The selected engine cannot consume this format.",
        ),
    ];

    // One row at full width; otherwise two-per-row so the footer never
    // clips the last entries.
    let per_row = if max_width >= 640.0 { 5 } else { 2 };
    let mut rows = Column::new().spacing(6).width(Length::Fill);
    for chunk in entries.chunks(per_row) {
        let mut line = Row::new().spacing(14);
        for (verdict, label, tip) in chunk {
            let accent = compat_verdict_accent(*verdict);
            let chip: Element<'static, Message> = w::styled_tooltip(
                row![icon_for(*verdict).color(accent), fonts::caption(*label)]
                    .spacing(4)
                    .align_y(Alignment::Center),
                fonts::caption(*tip),
                tooltip::Position::Top,
            )
            .into();
            line = line.push(chip);
        }
        rows = rows.push(line);
    }
    rows.into()
}

/// Row tint for a validator verdict: green = native, blue = supported /
/// losslessly convertible, amber = unknown, red = incompatible. The
/// low-alpha background plus a matching border reads as a subtle glow
/// without disturbing the list layout (no shadow bleed between rows).
fn compat_row_style(
    verdict: crate::compat::games::Verdict,
) -> iced::widget::container::Style {
    let accent = compat_verdict_accent(verdict);
    let background = Color {
        a: 0.13,
        ..accent
    };
    let border = Color {
        a: 0.38,
        ..accent
    };
    iced::widget::container::Style {
        background: Some(iced::Background::Color(background)),
        border: Border {
            color: border,
            width: 1.0,
            radius: 3.0.into(),
        },
        shadow: iced::Shadow {
            color: Color {
                a: accent.a * 0.13,
                ..accent
            },
            offset: iced::Vector::new(0.0, 0.0),
            blur_radius: 8.0,
        },
        ..Default::default()
    }
}

fn build_context_menu(
    archive: &crate::archive::ArchiveInfo,
    entry_index: usize,
    display_row: usize,
    scroll_y: f32,
    divider: Color,
) -> Option<Element<'_, Message>> {
    let entry = archive.entries.get(entry_index)?;

    // Header: the right-clicked entry's name, plus a "+N more" badge
    // when the right-click accumulated a multi-selection.
    let selected_count = archive.entries.iter().filter(|e| e.selected).count();
    let mut header = Column::new().spacing(2);
    header = header.push(fonts::strong(entry.file_name.to_string()));
    if selected_count > 1 {
        header = header.push(fonts::caption(format!(
            "+{} more selected",
            selected_count - 1
        )));
    }

    let mut items: Vec<Element<'_, Message>> = vec![header.into(), w::hairline(divider)];

    let lower = entry.file_name.to_lowercase();
    if lower.ends_with(".nif") || lower.ends_with(".dff") {
        items.push(
            context_button(
                "Open in 3D viewer",
                Message::EntryContextAction(EntryAction::Render),
            )
            .into(),
        );
        items.push(
            context_button(
                "Open in external viewer",
                Message::EntryContextAction(EntryAction::RenderExternal),
            )
            .into(),
        );
    } else if lower.ends_with(".col") {
        items.push(
            context_button(
                "Open in external viewer",
                Message::EntryContextAction(EntryAction::RenderExternal),
            )
            .into(),
        );
    }

    if lower.ends_with(".txd")
        || lower.ends_with(".nft")
        || lower.ends_with(".nif")
        || lower.ends_with(".dff")
    {
        items.push(
            context_button(
                "View textures",
                Message::EntryContextAction(EntryAction::ViewTextures),
            )
            .into(),
        );
    }

    if lower.ends_with(".nif") {
        // A NIF's textures live in its companion NFT; the action
        // resolves the basename and exports the NFT's contents.
        items.push(
            context_button(
                "Export companion NFT textures",
                Message::EntryContextAction(EntryAction::ExportEmbeddedTextures),
            )
            .into(),
        );
    } else if lower.ends_with(".nft") {
        // An NFT is itself a texture library; the action walks its
        // NiPixelData blocks directly.
        items.push(
            context_button(
                "Export Embedded Textures",
                Message::EntryContextAction(EntryAction::ExportEmbeddedTextures),
            )
            .into(),
        );
    }

    items.push(context_button("Export", Message::EntryContextAction(EntryAction::Export)).into());
    items.push(context_button("Rename", Message::EntryContextAction(EntryAction::Rename)).into());
    items.push(
        context_button(
            "Copy name",
            Message::EntryContextAction(EntryAction::CopyName),
        )
        .into(),
    );
    items.push(context_button("Delete", Message::EntryContextAction(EntryAction::Delete)).into());

    let card = container(
        iced::widget::Column::with_children(items)
            .spacing(4)
            .padding(8)
            .width(Length::Fill),
    )
    // Keep the outer card compact while giving its Fill labels a real width to use.
    .width(Length::Fixed(CONTEXT_MENU_WIDTH))
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

    // The row's y in the table pane equals the fixed header height plus the
    // row's position within the scrollable viewport.
    let row_y = HEADER_HEIGHT + (display_row as f32 * ROW_HEIGHT - scroll_y).max(0.0);

    // Float lays the card out at its natural size before translating it. This
    // makes the edge checks use the actual menu height instead of an estimate.
    let menu = Float::new(card)
        .translate(move |bounds, viewport| context_menu_translation(bounds, viewport, row_y));

    let backdrop = mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
        .on_press(Message::HideContextMenu);

    Some(stack(vec![backdrop.into(), menu.into()]).into())
}

const CONTEXT_MENU_EDGE_GAP: f32 = 8.0;
const CONTEXT_MENU_LEFT_OFFSET: f32 = 12.0;
const CONTEXT_MENU_WIDTH: f32 = 260.0;

fn context_menu_translation(bounds: Rectangle, viewport: Rectangle, row_y: f32) -> Vector {
    let anchor_x = bounds.x + CONTEXT_MENU_LEFT_OFFSET;
    let anchor_y = bounds.y + row_y;

    let min_x = viewport.x + CONTEXT_MENU_EDGE_GAP;
    let max_x = (viewport.x + viewport.width - bounds.width - CONTEXT_MENU_EDGE_GAP).max(min_x);
    let x = anchor_x.clamp(min_x, max_x);

    let min_y = viewport.y + CONTEXT_MENU_EDGE_GAP;
    let max_y = (viewport.y + viewport.height - bounds.height - CONTEXT_MENU_EDGE_GAP).max(min_y);
    let below_fits =
        anchor_y + bounds.height <= viewport.y + viewport.height - CONTEXT_MENU_EDGE_GAP;
    let above_y = anchor_y + ROW_HEIGHT - bounds.height;
    let preferred_y = if below_fits || above_y < min_y {
        anchor_y
    } else {
        above_y
    };
    let y = preferred_y.clamp(min_y, max_y);

    Vector::new(x - bounds.x, y - bounds.y)
}

/// A fixed text-color style closure; reused across widgets that must
/// keep the same foreground inside and outside `Float` overlays.
fn text_color_fn(
    color: Color,
) -> impl for<'a> Fn(&'a iced::Theme) -> iced::widget::text::Style {
    move |_| iced::widget::text::Style {
        color: Some(color),
    }
}

fn with_alpha(color: Color, factor: f32) -> Color {
    Color {
        a: color.a * factor,
        ..color
    }
}

/// Height (px) of a single prediction row in the search dropdown.
const PREDICTION_ROW_HEIGHT: f32 = 26.0;
/// Height (px) of the pinned search strip. The floating prediction
/// dropdown anchors at this offset below the workspace top.
const SEARCH_STRIP_HEIGHT: f32 = 34.0;
/// Width (px) of the pinned "Search:" label column inside the strip.
const SEARCH_LABEL_WIDTH: f32 = 88.0;
/// X offset of the floating dropdown: strip padding + label column +
/// row spacing. Keeps the card under the text input, not the whole
/// strip.
const SEARCH_DROPDOWN_X: f32 = 104.0;

/// Raw extension in capitals for the literal type display mode
/// (`DFF`, `NIF`), or `FILE` for extension-less names.
fn literal_type_label(file_name: &str) -> String {
    std::path::Path::new(file_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_uppercase())
        .unwrap_or_else(|| "FILE".to_string())
}

/// The fuzzy-search prediction dropdown, rendered between the search
/// strip and the pane grid while the search input is focused. Lists the
/// best subsequence matches; when the query matched nothing, shows the
/// single "Did you mean …" typo suggestion instead.
fn search_prediction_dropdown(
    app: &App,
    width: f32,
    accent: Color,
    surface: Color,
    divider: Color,
) -> Element<'_, Message> {
    let prediction_button = |name: String, message: Message, active: bool, hint: Option<&'static str>| {
        let label = if let Some(hint) = hint {
            row![fonts::caption(hint), fonts::body(name)]
                .spacing(6)
                .align_y(Alignment::Center)
        } else {
            row![fonts::body(name)].align_y(Alignment::Center)
        };
        let hover_bg = with_alpha(accent, 0.16);
        let active_bg = with_alpha(accent, 0.28);
        button(
            container(label)
                .width(Length::Fill)
                .align_x(Alignment::Start)
                .padding([2, 8]),
        )
        .height(Length::Fixed(PREDICTION_ROW_HEIGHT))
        .width(Length::Fill)
        .style(move |theme, status| {
            let highlighted = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let background = if highlighted {
                Some(iced::Background::Color(hover_bg))
            } else if active {
                Some(iced::Background::Color(active_bg))
            } else {
                None
            };
            let palette = theme.extended_palette();
            iced::widget::button::Style {
                background,
                text_color: w::readable_text_color(surface, palette.background.base.text),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 3.0.into(),
                },
                ..Default::default()
            }
        })
        .on_press(message)
    };

    let mut list = Column::new().spacing(2);
    let match_count = app.search_predictions.len();
    for (index, (_, name)) in app.search_predictions.iter().enumerate() {
        list = list.push(prediction_button(
            name.clone(),
            Message::SearchPredictPick(index),
            app.prediction_index == Some(index),
            None,
        ));
    }
    if let Some((_, name)) = &app.did_you_mean {
        list = list.push(prediction_button(
            name.clone(),
            Message::SearchPickDidYouMean,
            app.prediction_index.is_some_and(|i| i >= match_count),
            Some("Did you mean"),
        ));
    }

    container(container(list).padding([4, 4]))
        .width(Length::Fixed(width.max(0.0)))
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(surface)),
            border: Border {
                color: divider,
                width: 1.0,
                radius: 4.0.into(),
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                offset: iced::Vector::new(0.0, 3.0),
                blur_radius: 8.0,
            },
            ..Default::default()
        })
        .into()
}

/// Floating toast snackbar pinned to the bottom-right corner. Slides up and
/// fades in when a toast appears; fades back out through `reveal` after the
/// toast is dismissed. All colors are alpha-scaled by `reveal` (Iced has no
/// opacity widget), so the whole card animates as one surface.
fn build_toast_overlay(app: &App) -> Option<Element<'_, Message>> {
    let (text, reveal) = app.toast_overlay()?;
    let design = app.design();
    let surface = with_alpha(design.surface(), reveal);
    let border = with_alpha(design.border(), reveal);
    let text_color = with_alpha(design.text(), reveal);
    let accent = with_alpha(design.accent(), reveal);
    let shadow_alpha = 0.35 * reveal;

    let card = Container::new(
        row![
            w::accent_bar(accent, 20.0),
            fonts::body(text).color(text_color),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding(10)
    .max_width(480.0)
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(surface)),
        border: Border {
            color: border,
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, shadow_alpha),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 14.0,
        },
        ..Default::default()
    });

    Some(
        Float::new(card)
            .translate(move |bounds, viewport| {
                let margin = 16.0;
                let hidden_offset = (1.0 - reveal) * (bounds.height + margin + 8.0);
                let x = viewport.x + viewport.width - bounds.width - margin;
                let y = viewport.y + viewport.height - bounds.height - margin + hidden_offset;
                Vector::new(x - bounds.x, y - bounds.y)
            })
            .into(),
    )
}

fn context_button(label: &str, message: Message) -> iced::widget::Button<'_, Message> {
    button(w::icon_label(
        context_icon(&message),
        fonts::body(label)
            .align_x(iced::alignment::Horizontal::Left)
            .width(Length::Fill),
    ))
    .on_press(message)
    .width(Length::Fill)
    .style(crate::ui::view::menu_button_style)
}

fn context_icon(message: &Message) -> Element<'static, Message> {
    let icon = match message {
        Message::EntryContextAction(action) => match action {
            EntryAction::CopyName => icons::copy(),
            EntryAction::Rename => icons::rename(),
            EntryAction::Delete => icons::delete(),
            EntryAction::Export => icons::export(),
            EntryAction::Render => icons::model(),
            EntryAction::RenderExternal => icons::external_viewer(),
            EntryAction::ViewTextures => icons::texture(),
            EntryAction::ExportEmbeddedTextures => icons::export(),
        },
        _ => icons::generic_file(),
    };
    icon.size(16).into()
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
    button(w::icon_label(
        icons::copy().size(13),
        fonts::caption(label).align_x(iced::alignment::Horizontal::Center),
    ))
    .on_press(message)
    .width(Length::Shrink)
    .style(menu_button_style)
    .into()
}

pub fn version_label(version: ImgVersion) -> &'static str {
    match version {
        ImgVersion::One => "PC v1",
        ImgVersion::Two => "PC v2",
        ImgVersion::Xbox360 => "Xbox 360 IMG v1",
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

/// Short, state-aware hint for a table header sort button, matching the
/// behaviour in the `SortBy` handler (Size starts largest-first; Type
/// cycles the primary type).
fn sort_tooltip_text(
    column: SortColumn,
    active: bool,
    direction: SortDirection,
    primary_type: Option<&str>,
) -> String {
    match column {
        SortColumn::Name => match (active, direction) {
            (true, SortDirection::Ascending) => "Sorted by file name (A → Z).",
            (true, SortDirection::Descending) => "Sorted by file name (Z → A).",
            (false, _) => "Sort by file name (A → Z).",
        }
        .to_string(),
        SortColumn::Type => match (active, primary_type) {
            (true, Some(primary)) => format!("Sorted by file type, {primary} first."),
            (true, None) => "Sorted by file type alphabetically.".to_string(),
            (false, _) => "Sort by file type (alphabetical).".to_string(),
        },
        SortColumn::Size => match (active, direction) {
            (true, SortDirection::Descending) => "Sorted by size (largest first).",
            (true, SortDirection::Ascending) => "Sorted by size (smallest first).",
            (false, _) => "Sort by size (largest first).",
        }
        .to_string(),
    }
}

pub fn menu_button_style(theme: &iced::Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let highlighted = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let highlight = palette.background.strong.color;
    button::Style {
        background: if highlighted {
            Some(highlight.into())
        } else {
            None
        },
        text_color: if highlighted {
            w::readable_text_color(highlight, palette.background.base.text)
        } else {
            palette.background.base.text
        },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_labels_disambiguate_duplicate_names() {
        use crate::parser::ImgVersion;

        let mut iii = crate::archive::ArchiveInfo::new("gta3", false, ImgVersion::One);
        iii.path = Some(std::path::PathBuf::from("C:/games/III/gta3.img"));
        let mut sa = crate::archive::ArchiveInfo::new("gta3", false, ImgVersion::Two);
        sa.path = Some(std::path::PathBuf::from("C:/games/SA/gta3.img"));
        let mut player = crate::archive::ArchiveInfo::new("player", false, ImgVersion::Two);
        player.path = Some(std::path::PathBuf::from("C:/games/SA/player.img"));

        let archives = vec![iii, sa, player];
        assert_eq!(
            archive_tab_label(&archives[0], &archives),
            "gta3 · games\\III",
            "two parent layers disambiguate models folders"
        );
        assert_eq!(archive_tab_label(&archives[1], &archives), "gta3 · games\\SA");
        assert_eq!(
            archive_tab_label(&archives[2], &archives),
            "player",
            "unique names stay bare"
        );
    }

    #[test]
    fn tab_labels_ellipsize_to_the_tab_width() {
        let long = "gta3.img · GTA San Andreas\\models";
        let short = "player.img";

        // Wide tabs keep the whole label; narrow tabs cut it with an
        // ellipsis and never exceed the glyph budget.
        assert_eq!(ellipsize(short, 150.0), short);
        let cut = ellipsize(long, 100.0);
        assert!(cut.ends_with('…'));
        assert!(cut.chars().count() < long.chars().count());
        assert!(ellipsize(long, 40.0).chars().count() >= 4);

        // The minimum width still leaves room for a readable stub.
        let minimal = ellipsize(long, crate::config::ARCHIVE_TAB_WIDTH_MIN);
        assert!(minimal.chars().count() >= 6, "got {minimal:?}");
    }

    #[test]
    fn context_menu_stays_below_when_the_viewport_has_room() {
        let bounds = Rectangle {
            x: 100.0,
            y: 20.0,
            width: 140.0,
            height: 100.0,
        };
        let viewport = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 500.0,
            height: 400.0,
        };

        let translation = context_menu_translation(bounds, viewport, 80.0);

        assert_eq!(bounds.x + translation.x, 112.0);
        assert_eq!(bounds.y + translation.y, 100.0);
    }

    #[test]
    fn context_menu_flips_above_a_bottom_edge() {
        let bounds = Rectangle {
            x: 100.0,
            y: 20.0,
            width: 140.0,
            height: 170.0,
        };
        let viewport = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 500.0,
            height: 260.0,
        };

        let translation = context_menu_translation(bounds, viewport, 190.0);
        let top = bounds.y + translation.y;

        assert_eq!(top, 72.0);
        assert!(top + bounds.height <= viewport.y + viewport.height - CONTEXT_MENU_EDGE_GAP);
    }

    #[test]
    fn context_menu_clamps_to_viewport_edges() {
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 140.0,
            height: 100.0,
        };
        let viewport = Rectangle {
            x: 10.0,
            y: 30.0,
            width: 180.0,
            height: 130.0,
        };

        let translation = context_menu_translation(bounds, viewport, 0.0);
        let left = bounds.x + translation.x;
        let top = bounds.y + translation.y;

        assert_eq!(left, viewport.x + CONTEXT_MENU_EDGE_GAP);
        assert_eq!(top, viewport.y + CONTEXT_MENU_EDGE_GAP);
    }

    #[test]
    fn alternate_entry_rows_are_subtle_in_light_and_dark_designs() {
        for design in [Design::light(), Design::dark()] {
            let base = design.page();
            let alternate = alternate_entry_row_background(&design);

            assert_ne!(alternate, base);
            if design.is_dark {
                assert!(alternate.relative_luminance() > base.relative_luminance());
            } else {
                assert!(alternate.relative_luminance() < base.relative_luminance());
            }
            assert!(alternate.relative_contrast(base) < 1.4);
        }
    }
}
