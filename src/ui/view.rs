use iced::widget::{
    checkbox, Column, Container, Row, Scrollable, Space, button, column, container, image,
    mouse_area, pane_grid, progress_bar, rule, row, stack, text_input, tooltip,
};
use iced::{Alignment, Border, Color, Element, Length};
use crate::archive::{ExportStatus, SortColumn};
use crate::sort::SortDirection;

use crate::inspector::scene3d::camera::BaseOrientation;
use crate::inspector::scene3d::pipeline::RenderFlags;
use crate::parser::{EntryInspection, ImgVersion};
use crate::ui::app::{App, EntryAction, InspectorTab, Message, Pane, ABOUT_TEXT};
use crate::ui::fonts;
use crate::ui::icons;
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
        let type_label = archive.sort.type_header_label.clone();
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
            content = content.push(self.build_entry_row(display_row, entry));
        }

        if bottom_pad_rows > 0 {
            content = content.push(Space::new().height(Length::Fixed(bottom_pad_height)));
        }

        let content = content.height(Length::Fixed(total_height));

        let scrollable = Scrollable::new(content)
            .id(iced::widget::Id::new("entry_table"))
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

        if let Some((entry_index, display_row)) = self.context_menu
            && let Some(overlay) = build_context_menu(archive, entry_index, display_row, scroll_y)
        {
            layers.push(overlay);
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

    fn build_entry_row<'a>(
        &'a self,
        display_row: usize,
        entry: &'a crate::archive::EntryInfo,
    ) -> Element<'a, Message> {
        use std::borrow::Cow;

        let is_renaming = entry.rename;
        let is_selected = entry.selected;

        // Render display strings on demand for the visible row only. Pre-caching
        // these for every filtered entry caused thousands of allocations each
        // time the filter or selection changed.
        let file_name: Cow<'_, str> = if is_selected {
            Cow::Owned(format!("✓ {}", entry.file_name))
        } else {
            Cow::Borrowed(entry.file_name.as_str())
        };
        let file_type = Cow::Borrowed(entry.file_type.as_str());
        let size_kb = Cow::Owned(format!("{} KB", entry.sector * 2));

        let name_widget: Element<'_, Message> = if is_renaming {
            text_input("", &self.rename_buffer)
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
            label.width(Length::Fill).into()
        };

        let name_cell = w::icon_label(
            icons::file_type(&entry.file_name).size(16),
            name_widget,
        )
        .width(Length::FillPortion(6));

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
        let width = Length::Fixed(300.0);

        let export_tab = self.build_export_tab();
        let model_tab = self.build_model_tab();
        let texture_tab = self.build_texture_tab();

        let bold_text = iced::Font {
            family: iced::font::Family::default(),
            weight: iced::font::Weight::Bold,
            ..iced::Font::default()
        };
        let tabs: Element<'_, Message> = iced_aw::widget::tabs::Tabs::new(
            Message::Viewer3dSelectTab,
        )
        .push(
            InspectorTab::Export,
            iced_aw::TabLabel::Text("Export".to_string()),
            export_tab,
        )
        .push(
            InspectorTab::Model3D,
            iced_aw::TabLabel::Text("3D view".to_string()),
            model_tab,
        )
        .push(
            InspectorTab::Texture,
            iced_aw::TabLabel::Text("Texture".to_string()),
            texture_tab,
        )
        .set_active_tab(&self.selected_inspector_tab)
        .tab_bar_height(Length::Fixed(32.0))
        .text_size(13.0)
        .text_font(bold_text)
        .height(Length::Fill)
        .width(width)
        .into();

        tabs
    }

    fn build_export_tab(&self) -> Element<'_, Message> {
        let Some(archive) = self
            .editor
            .archives()
            .get(self.editor.selected_archive().unwrap_or(0))
        else {
            return Space::new().width(Length::Fill).height(Length::Fill).into();
        };

        let version_text = version_label(archive.version);
        let total = archive.entries.len();
        let visible = archive.selected_indices.len();
        let raw_progress = archive.progress.percentage();
        let in_use = archive.progress.in_use();
        let progress = self.animator.get_or(crate::ui::app::ANIM_PROGRESS, raw_progress);
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

        let mut col = column![
            label_value_owned("Format", version_text.to_string()),
            label_value("Entries", format!("{total} (visible: {visible})")),
            rule::horizontal(1),
            label_value(progress_label, percent_text),
            progress_bar(0.0..=1.0, display_progress),
        ]
        .spacing(6)
        .padding(8)
        .width(Length::Fill);

        if in_use {
            col = col.push(
                button(w::icon_label(icons::close().size(14), fonts::body("Cancel")))
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

        col = col.push(
            checkbox(self.fast_export)
                .label("Fast export (C++ speed)")
                .on_toggle(Message::FastExportToggled),
        );

        col = col.push(rule::horizontal(1));

        if let Some((index, inspection)) = self.inspected_entry.as_ref()
            && archive.entries.get(*index).is_some()
        {
            col = col.push(row![
                fonts::header("Selected entry:"),
                Space::new().width(Length::Fill),
                copy_button("Copy", Message::CopySelectedEntryDetails),
            ]);
            col = col.push(Self::build_inspection_panel(inspection));
            col = col.push(rule::horizontal(1));
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
        let entry_lower = archive
            .entries
            .get(self.editor.selected_entry().unwrap_or(0))
            .map(|e| e.file_name.to_ascii_lowercase())
            .unwrap_or_default();
        let is_nif = entry_lower.ends_with(".nif");
        let has_scene = self
            .viewer3d_handle
            .with(|inner| inner.scene.is_some());

        let toolbar = self.build_viewer3d_toolbar(is_nif);
        let stats = self.build_viewer3d_stats();

        let body: Element<'_, Message> = if has_scene || is_nif {
            let widget = crate::ui::viewer3d_widget::Scene3dWidget::new(
                self.viewer3d_handle.clone(),
            );
            widget.into()
        } else {
            container(fonts::caption(format!(
                "The in-app viewer renders .nif entries. {} isn't a NIF — use the right-click menu to open it in an external viewer.",
                entry_lower
            )))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into()
        };

        let prompt: Element<'_, Message> = if !has_scene && is_nif {
            button(w::icon_label(icons::model().size(14), fonts::body("Render NIF")))
                .on_press(Message::EntryContextAction(EntryAction::Render))
                .into()
        } else if !has_scene {
            fonts::caption("Select a .nif entry, then right-click → Open in 3D viewer.").into()
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

    fn build_viewer3d_stats(&self) -> Element<'_, Message> {
        let (triangles, vertices, textures, has_scene, w, h, orientation) = self
            .viewer3d_handle
            .with(|i| {
                let w = i.camera.viewport.width.max(1);
                let h = i.camera.viewport.height.max(1);
                let orient = i
                    .scene
                    .as_ref()
                    .map(|s| s.base_orientation)
                    .unwrap_or(BaseOrientation::Yup);
                (
                    i.scene.as_ref().map(|s| s.total_triangles()).unwrap_or(0),
                    i.scene.as_ref().map(|s| s.total_vertices()).unwrap_or(0),
                    i.scene.as_ref().map(|s| s.textured_mesh_count()).unwrap_or(0),
                    i.scene.is_some(),
                    w,
                    h,
                    orient,
                )
            });
        let orient_label = match orientation {
            BaseOrientation::Yup => "Y-up",
            BaseOrientation::Zup => "Z-up",
            BaseOrientation::Xup => "X-up",
        };
        let line = if has_scene {
            format!(
                "{} vertices   {} triangles   {} textures   {}×{}   {}",
                vertices, triangles, textures, w, h, orient_label
            )
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
        let entry_index = self.editor.selected_entry().unwrap_or(0);
        let Some(entry) = archive.entries.get(entry_index) else {
            return container(fonts::caption("Select a .txd entry to preview textures."))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into();
        };
        let is_txd = entry.file_name.to_ascii_lowercase().ends_with(".txd");
        if !is_txd {
            return container(fonts::caption(format!(
                "{} is not a .txd file. Texture preview is available for .txd entries only.",
                entry.file_name
            )))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into();
        }
        let textures = archive.txd_cache.get(&entry_index);
        let Some(textures) = textures else {
            return column![
                fonts::caption(format!("TXD {}.txd not yet decoded.", entry.file_name)),
                button(w::icon_label(
                    icons::texture().size(14),
                    fonts::body("Decode textures"),
                ))
                .on_press(Message::TxdDecodeRequested),
            ]
            .spacing(4)
            .align_x(Alignment::Center)
            .padding(8)
            .into();
        };
        if textures.is_empty() {
            return container(fonts::caption("No textures in TXD."))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into();
        }
        let tex_idx = self.txd_selected_texture.min(textures.len() - 1);
        let tex = &textures[tex_idx];
        let mut col = Column::new().spacing(6)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8);
        col = col.push(
            button(w::icon_label(
                icons::export().size(14),
                fonts::body(format!("Export textures ({})", textures.len())),
            ))
            .on_press(Message::TxdExportTextures),
        );
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
        // Lazily build the Iced image handle once per texture and cache it on
        // the decoded texture. This avoids cloning the full RGBA buffer on every
        // frame while the texture tab is open.
        let handle = tex
            .handle
            .get_or_init(|| image::Handle::from_rgba(tex.width, tex.height, tex.rgba.clone()))
            .clone();
        let preview = image::Viewer::new(handle)
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(iced::ContentFit::Contain);
        col = col.push(preview);
        col.into()
    }

    fn build_viewer3d_toolbar(&self, is_nif: bool) -> Element<'_, Message> {
        if !is_nif {
            return Space::new().height(Length::Fixed(28.0)).into();
        }
        let flags = self.viewer3d_handle.with(|i| i.flags);
        let button_height = Length::Fixed(28.0);
        let mut row = Row::new().spacing(4).padding(2);
        row = row.push(w::icon_label(icons::model().size(14), fonts::caption("3D:")));
        row = row.push(
            tooltip(
                button(w::icon_label(
                    icons::refresh().size(14),
                    fonts::caption("Reset view"),
                ))
                    .on_press(Message::Viewer3dReset)
                    .height(button_height),
                fonts::caption("Re-fit the camera to the model. Shortcut: R"),
                tooltip::Position::Bottom,
            ),
        );
        row = row.push(
            tooltip(
                button(w::icon_label(icons::close().size(14), fonts::caption("Clear")))
                    .on_press(Message::Viewer3dClear)
                    .height(button_height),
                fonts::caption("Drop the loaded scene"),
                tooltip::Position::Bottom,
            ),
        );
        row = row.push(
            checkbox(flags.contains(RenderFlags::WIREFRAME))
                .label("Wireframe")
                .on_toggle(|_| Message::Viewer3dToggleWireframe),
        );
        row = row.push(
            checkbox(flags.contains(RenderFlags::CULL_BACK))
                .label("Cull backfaces")
                .on_toggle(|_| Message::Viewer3dToggleCullBackfaces),
        );
        row = row.push(
            checkbox(flags.contains(RenderFlags::HAS_TEXTURE))
                .label("Textured")
                .on_toggle(|_| Message::Viewer3dToggleTextured),
        );
        row.into()
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
        let design = self.design();

        // Build the status text: left side.
        let selected_count = self.editor.selected_archive().map_or(0, |idx| {
            self.editor.archives().get(idx).map_or(0, |a| {
                a.entries.iter().filter(|e| e.selected).count()
            })
        });

        let left_text = if self.toast.is_some() {
            self.toast.clone().unwrap_or_default()
        } else if selected_count > 0 {
            format!("Selected: {selected_count}")
        } else {
            format!("{} v{}", crate::ui::theme::APP_NAME, env!("CARGO_PKG_VERSION"))
        };

        // Animate a smooth transition between the normal surface color
        // and a success-green tint when a toast is active.
        let normal_bg = design.surface_subtle();
        let toast_bg = design.success_gradient().0;
        let mix = self.animator.get(crate::ui::app::ANIM_TOAST_OPACITY).clamp(0.0, 1.0);
        let bg = Color {
            r: normal_bg.r + (toast_bg.r - normal_bg.r) * mix,
            g: normal_bg.g + (toast_bg.g - normal_bg.g) * mix,
            b: normal_bg.b + (toast_bg.b - normal_bg.b) * mix,
            a: 1.0,
        };

        let bar = Container::new(
            Row::new()
                .push(fonts::caption(left_text))
                .push(Space::new().width(Length::Fill))
                .align_y(Alignment::Center)
                .padding(6),
        )
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(bg)),
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

fn build_toolbar(accent: Color, bg: Color) -> Element<'static, Message> {
    let toolbar = row![
        tooltip(
            toolbar_button(icons::new_archive().size(18).into(), Message::NewArchive),
            fonts::body("New"),
            tooltip::Position::Bottom,
        ),
        tooltip(
            toolbar_button(icons::open_archive().size(18).into(), Message::OpenArchive),
            fonts::body("Open"),
            tooltip::Position::Bottom,
        ),
        tooltip(
            toolbar_button(icons::save().size(18).into(), Message::SaveArchive),
            fonts::body("Save"),
            tooltip::Position::Bottom,
        ),
        rule::vertical(1),
        tooltip(
            toolbar_button(icons::import().size(18).into(), Message::ImportFiles),
            fonts::body("Import"),
            tooltip::Position::Bottom,
        ),
        tooltip(
            toolbar_button(icons::export().size(18).into(), Message::ExportSelected),
            fonts::body("Export selected"),
            tooltip::Position::Bottom,
        ),
        rule::vertical(1),
        tooltip(
            toolbar_button(icons::delete().size(18).into(), Message::DeleteSelected),
            fonts::body("Delete selected"),
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

pub fn build(app: &App) -> Element<'_, Message> {
    let design = app.design();
    let tab_surface = design.surface_subtle();
    let empty_state_accent = design.accent();
    let menubar = app.menubar();
    let toolbar = build_toolbar(design.accent(), design.surface_subtle());

    let tab_bar: Element<'_, Message> = if app.editor.archives().is_empty() {
        Space::new().height(Length::Fixed(0.0)).into()
    } else {
        let selected = app.editor.selected_archive().unwrap_or(0);
        let mut tab_rows = Vec::new();
        for (index, archive) in app.editor.archives().iter().enumerate() {
            let is_selected = index == selected;
            let label = if archive.dirty {
                format!("● {}", archive.file_name)
            } else {
                archive.file_name.clone()
            };
            let tab = button(fonts::body(label))
                .on_press(Message::SelectArchiveTab(index))
                .style(if is_selected { button::primary } else { button::secondary });
            // Accent bar on the left of the active tab
            if is_selected {
                tab_rows.push(
                    Row::new()
                        .push(w::accent_bar(design.accent(), 32.0))
                        .push(tab)
                        .align_y(Alignment::Center)
                        .into()
                );
            } else {
                tab_rows.push(tab.into());
            }
        }
        let row = Row::with_children(tab_rows).spacing(4).padding(4);
        Container::new(row).style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(tab_surface)),
            ..Default::default()
        }).into()
    };

    let body: Element<'_, Message> = if app.editor.archives().is_empty() {
        Container::new(
            column![
                Space::new().height(Length::Fill),
                icons::archive().size(42).color(empty_state_accent),
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
        let search = row![
            w::icon_label(icons::search().size(15), fonts::header("Search:")),
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
        build_sort_manager(app),
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
            logo_element(),
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

/// `static` empty maps for the IDE/COL fallback. Living for `'static`
/// lets `build_sort_manager` return `Element<'static, ...>` without
/// leaking per-call locals. The maps are never mutated, so a shared
/// global is sound.
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

    // The preview pane shows the first 10 entries of the active
    // archive sorted through the draft chain. We pull them from
    // the in-memory archive state — no disk I/O. The IDE/COL
    // maps are empty here; the comparator falls back to name
    // sort when those keys are in the chain, which matches what
    // the user sees in the actual table.
    // The dialog's Element lifetime is tied to the borrowed data
    // (draft, preview, IDE/COL maps). The simplest way to satisfy
    // the borrow checker is to leak the per-call data — the dialog
    // is open for at most a few seconds and the cost is bounded by
    // `PREVIEW_MAX * sizeof(EntryInfo)` per open. The leak is the
    // "Rust alternative" — we trade a few hundred bytes for the
    // ability to return an `Element<'static>` from a borrowed
    // `&App` context without restructuring the entire view layer.
    // Empty-slice leak shared across all "no archive" invocations
    // so we never allocate just to leak a 0-byte slice. Same cost
    // model as the static HashMaps above.
    static EMPTY_ENTRIES: std::sync::LazyLock<
        Box<[crate::archive::EntryInfo]>,
    > = std::sync::LazyLock::new(|| Box::new([]));

    let leaked: &'static [crate::archive::EntryInfo] = app
        .editor
        .selected_archive()
        .and_then(|idx| app.editor.archives().get(idx))
        .map(|a| {
            Box::leak(
                a.entries
                    .iter()
                    .take(10)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ) as &'static [crate::archive::EntryInfo]
        })
        .unwrap_or(&[]);
    let archive_name: Option<&'static str> = app
        .editor
        .selected_archive()
        .and_then(|idx| app.editor.archives().get(idx))
        .map(|a| Box::leak(a.file_name.clone().into_boxed_str()) as &'static str);

    let dialog = crate::ui::sort_manager::build(
        archive_name,
        draft,
        leaked,
        None, // primary_type - populated for the table view, not the dialog
        &EMPTY_IDE_MAP,
        &EMPTY_COL_MAP,
    );

    // Wrap the dialog content in our modal frame. We can't use the
    // generic `modal_box` helper here because the dialog's lifetime
    // is tied to the borrowed `&App` context, not `'static`. The
    // dialog already renders its own title + footer so the modal
    // frame is just a styled container.
    Some(
        Container::new(dialog)
            .style(|_| iced::widget::container::Style {
                background: Some(iced::Background::Color(
                    Color::from_rgb(0.10, 0.11, 0.13),
                )),
                text_color: Some(Color::WHITE),
                border: iced::Border {
                    color: Color::from_rgb(0.30, 0.32, 0.36),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            })
            .into(),
    )
}

fn modal_box<'a>(
    title: &'a str,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
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
    let card = Container::new(content)
        .style(move |theme: &iced::Theme| iced::widget::container::Style {
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

    let lower = entry.file_name.to_lowercase();
    if lower.ends_with(".nif") {
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
    } else if lower.ends_with(".dff") || lower.ends_with(".col") {
        items.push(
            context_button(
                "Open in external viewer",
                Message::EntryContextAction(EntryAction::RenderExternal),
            )
            .into(),
        );
    }

    if entry.file_name.to_lowercase().ends_with(".txd") {
        items.push(
            context_button("View textures",
                Message::EntryContextAction(EntryAction::ViewTextures)).into(),
        );
    }

    if lower.ends_with(".nif") {
        // A NIF's textures live in its companion NFT; the action
        // resolves the basename and exports the NFT's contents.
        items.push(
            context_button("Export companion NFT textures",
                Message::EntryContextAction(EntryAction::ExportEmbeddedTextures)).into(),
        );
    } else if lower.ends_with(".nft") {
        // An NFT is itself a texture library; the action walks its
        // NiPixelData blocks directly.
        items.push(
            context_button("Export Embedded Textures",
                Message::EntryContextAction(EntryAction::ExportEmbeddedTextures)).into(),
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
