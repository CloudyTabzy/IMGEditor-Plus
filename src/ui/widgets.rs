//! High-level widget helpers built on the design system.
//!
//! These compose the Iced primitives with the `Design` system to produce
//! visually consistent pieces (cards, accent bars, gradient panels,
//! hover-able rows, toast/snackbar styling).
//!
//! The helpers return either `Element`s (callers can drop them straight
//! into a column/row) or `Container`s (callers can further style/pad).
//! Most of the heavy lifting is in the colors: a subtle gradient on the
//! menubar, an accent left-bar on the active tab, and a shadow-bordered
//! card for the inspector pane go a long way toward a "designer" feel
//! without dragging in animation libraries.

use iced::widget::{Column, Container, Row, Space, rule};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::ui::design::Design;
use crate::ui::fonts;

/// Vertical or horizontal gradient container (uses the same `Color` on both
/// stops for solid color; or two distinct stops for a gradient effect).
/// Iced does not support linear gradients natively, so we emulate the
/// effect with a thin (1px) inset of the second color on top of the
/// first — a cheap, GPU-free way to suggest depth.
pub fn gradient_panel<'a, Message: 'a>(
    design: &Design,
    vertical: bool,
    top_color: Color,
    bottom_color: Color,
    content: impl Into<Element<'a, Message>>,
) -> Container<'a, Message> {
    let content: Element<'a, Message> = content.into();
    let inner: Container<'a, Message> = Container::new(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(top_color)),
            ..Default::default()
        });

    if top_color == bottom_color {
        return inner;
    }

    // A 1px line at the bottom (or right) of the panel gives a subtle
    // "lit from above" hint. We use a column with an opaque pixel on top
    // of the panel.
    let accent_color = bottom_color;
    if vertical {
        let line: Element<'a, Message> = Space::new()
            .width(Length::Fill)
            .height(Length::Fixed(1.0))
            .into();
        let _ = line;
        Container::new(
            Column::new().push(inner).push(
                Container::new(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(accent_color)),
                        ..Default::default()
                    })
                    .width(Length::Fill),
            ),
        )
        .width(Length::Fill)
        .height(Length::Fill)
    } else {
        let _ = design; // suppress unused warning
        Container::new(
            Row::new().push(inner).push(
                Container::new(Space::new().width(Length::Fixed(1.0)).height(Length::Fill))
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(accent_color)),
                        ..Default::default()
                    })
                    .height(Length::Fill),
            ),
        )
        .width(Length::Fill)
        .height(Length::Fill)
    }
}

/// Card / surface container with elevation, rounded corners, and border.
pub fn card<'a, Message: 'a>(
    design: &Design,
    content: impl Into<Element<'a, Message>>,
) -> Container<'a, Message> {
    let content: Element<'a, Message> = content.into();
    let elevation = design.tokens.elevation.raised;
    let border = design.iced_border(&elevation);
    let shadow = design.iced_shadow(&elevation);
    let surface = design.surface();
    let radius = design.radius().lg();

    Container::new(content)
        .padding(12)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(surface)),
            border: Border {
                color: border.color,
                width: border.width,
                radius: radius.into(),
            },
            shadow,
            ..Default::default()
        })
}

/// Stronger card (modals, popovers) with floating elevation.
pub fn floating_card<'a, Message: 'a>(
    design: &Design,
    content: impl Into<Element<'a, Message>>,
) -> Container<'a, Message> {
    let content: Element<'a, Message> = content.into();
    let elevation = design.tokens.elevation.floating;
    let border = design.iced_border(&elevation);
    let shadow = design.iced_shadow(&elevation);
    let surface = design.surface();
    let radius = design.radius().xl();

    Container::new(content)
        .padding(16)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(surface)),
            border: Border {
                color: border.color,
                width: border.width,
                radius: radius.into(),
            },
            shadow,
            ..Default::default()
        })
}

/// Tooltip with a bounded, themed card so hints stay readable over compact
/// panes and inherit the active light/dark palette.
pub fn styled_tooltip<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    hint: impl Into<Element<'a, Message>>,
    position: iced::widget::tooltip::Position,
) -> iced::widget::Tooltip<'a, Message> {
    iced::widget::tooltip(content, tooltip_card(hint), position)
}

/// Choose a foreground that remains readable on a solid background while
/// retaining the theme's preferred color whenever it already has sufficient
/// contrast.
///
/// Iced's palette helper uses the same contrast-aware logic as its built-in
/// widget styles. Keeping this at the widget boundary prevents custom styles
/// from accidentally pairing a light accent with light text.
#[inline]
pub fn readable_text_color(background: Color, preferred: Color) -> Color {
    iced::theme::palette::readable(background, preferred)
}

/// Choose one foreground for an animation's full background range. Re-running
/// `readable_text_color` for every frame can flip between light and dark text
/// at a contrast threshold, which makes a pulse look like a text flicker.
#[inline]
pub fn stable_readable_text_color(
    initial_background: Color,
    peak_background: Color,
    preferred: Color,
) -> Color {
    const MINIMUM_CONTRAST: f32 = 4.5;

    let preferred_contrast = initial_background
        .relative_contrast(preferred)
        .min(peak_background.relative_contrast(preferred));
    if preferred_contrast >= MINIMUM_CONTRAST {
        return preferred;
    }

    let white_contrast = initial_background
        .relative_contrast(Color::WHITE)
        .min(peak_background.relative_contrast(Color::WHITE));
    let black_contrast = initial_background
        .relative_contrast(Color::BLACK)
        .min(peak_background.relative_contrast(Color::BLACK));

    if white_contrast >= black_contrast {
        Color::WHITE
    } else {
        Color::BLACK
    }
}

/// Interpolate a foreground with a contrast-safe endpoint using a smoothstep
/// curve, avoiding a visibly abrupt text-color change during feedback pulses.
#[inline]
pub fn smooth_color_mix(start: Color, end: Color, progress: f32) -> Color {
    let progress = progress.clamp(0.0, 1.0);
    if progress <= 0.0 {
        return start;
    }
    if progress >= 1.0 {
        return end;
    }
    let progress = progress * progress * (3.0 - 2.0 * progress);
    iced::theme::palette::mix(start, end, progress)
}

/// Shared tooltip surface for all view-level hints.
pub fn tooltip_card<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Container<'a, Message> {
    Container::new(content)
        .padding([6, 8])
        .max_width(230.0)
        .style(|theme: &iced::Theme| {
            let palette = theme.extended_palette();
            let background = palette.background.strong.color;
            iced::widget::container::Style {
                background: Some(background.into()),
                text_color: Some(readable_text_color(
                    background,
                    palette.background.strong.text,
                )),
                border: Border {
                    color: palette.primary.weak.color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        })
}

/// 3-pixel-wide left accent bar — used to mark the active archive tab.
pub fn accent_bar<'a, Message: 'a>(color: Color, height: f32) -> Container<'a, Message> {
    Container::new(
        Space::new()
            .width(Length::Fixed(3.0))
            .height(Length::Fixed(height)),
    )
    .style(move |_| iced::widget::container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .width(Length::Fixed(3.0))
    .height(Length::Fixed(height))
}

/// Pill-shaped badge — used for "TXD", "DFF", "COL" entry-type pills,
/// status messages, etc. Caller controls the background and text color.
pub fn badge<'a, Message: 'a>(
    label: String,
    bg: Color,
    text_color: Color,
) -> Container<'a, Message> {
    Container::new(fonts::caption(label).color(text_color))
        .padding(Padding {
            top: 2.0,
            bottom: 2.0,
            left: 8.0,
            right: 8.0,
        })
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 9999.0.into(),
            },
            ..Default::default()
        })
}

/// Snackbar/toast that floats at the bottom-right with the design-system
/// floating-card styling. Caller passes content (typically a row of icon +
/// message + optional action).
pub fn snackbar<'a, Message: 'a>(
    design: &Design,
    content: impl Into<Element<'a, Message>>,
) -> Container<'a, Message> {
    let content: Element<'a, Message> = content.into();
    let elevation = design.tokens.elevation.floating;
    let border = design.iced_border(&elevation);
    let shadow = design.iced_shadow(&elevation);
    let surface = design.surface();
    let radius = design.radius().lg();

    Container::new(content)
        .padding(12)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(surface)),
            border: Border {
                color: border.color,
                width: border.width,
                radius: radius.into(),
            },
            shadow,
            ..Default::default()
        })
}

/// Subtle row hover (e.g. for archive tab buttons).
pub fn hover_surface<'a, Message: 'a>(
    bg: Color,
    content: impl Into<Element<'a, Message>>,
) -> Container<'a, Message> {
    let content: Element<'a, Message> = content.into();
    Container::new(content).style(move |_| iced::widget::container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    })
}

/// Horizontal rule with the design system's border color.
pub fn hairline<'a, Message: 'a>(color: Color) -> Element<'a, Message> {
    rule::horizontal(1)
        .style(move |_theme| iced::widget::rule::Style {
            color,
            radius: 0.0.into(),
            fill_mode: iced::widget::rule::FillMode::Full,
            snap: false,
        })
        .into()
}

pub fn vhairline<'a, Message: 'a>(color: Color) -> Element<'a, Message> {
    rule::vertical(1)
        .style(move |_theme| iced::widget::rule::Style {
            color,
            radius: 0.0.into(),
            fill_mode: iced::widget::rule::FillMode::Full,
            snap: false,
        })
        .into()
}

/// Build a styled `Row` for menubar entries.
pub fn menubar_row<'a, Message: 'a>() -> Row<'a, Message> {
    Row::new()
        .spacing(0)
        .padding(Padding {
            top: 0.0,
            bottom: 0.0,
            left: 0.0,
            right: 0.0,
        })
        .align_y(Alignment::Center)
}

/// Compose a compact icon and label while keeping alignment consistent across
/// menus, context actions, and secondary controls.
pub fn icon_label<'a, Message: 'a>(
    icon: impl Into<Element<'a, Message>>,
    label: impl Into<Element<'a, Message>>,
) -> Row<'a, Message> {
    Row::new()
        .push(icon.into())
        .push(Space::new().width(Length::Fixed(6.0)))
        .push(label.into())
        .spacing(0)
        .align_y(Alignment::Center)
}

/// Build a styled `Column` for the root layout with no spacing.
pub fn root_column<'a, Message: 'a>() -> Column<'a, Message> {
    Column::new()
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill)
}

/// Transparent root wrapper that observes `window::Event::FileDropped`
/// synchronously during `UserInterface::update`.
///
/// Routing file drops through `iced::window::events()` goes through a
/// bounded 100-slot subscription channel (`try_send`), which silently drops
/// files past #100 in large multi-file drops and can split a single OS drop
/// across multiple event-loop turns. Observing the event at the widget root
/// delivers every dropped path synchronously in the same update batch.
pub fn file_drop_catcher<'a, Message, Theme, Renderer, F>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    on_drop: F,
) -> Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
    F: Fn(std::path::PathBuf) -> Message + 'a,
{
    Element::new(FileDropCatcher {
        content: content.into(),
        on_drop,
    })
}

struct FileDropCatcherTag;

struct FileDropCatcher<'a, Message, Theme, Renderer, F> {
    content: Element<'a, Message, Theme, Renderer>,
    on_drop: F,
}

impl<Message, Theme, Renderer, F> iced::advanced::Widget<Message, Theme, Renderer>
    for FileDropCatcher<'_, Message, Theme, Renderer, F>
where
    Renderer: iced::advanced::Renderer,
    F: Fn(std::path::PathBuf) -> Message,
{
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<FileDropCatcherTag>()
    }

    fn size(&self) -> iced::Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> iced::Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        vec![iced::advanced::widget::Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
        if let iced::Event::Window(iced::window::Event::FileDropped(path)) = event {
            shell.publish((self.on_drop)(path.clone()));
        }
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &Renderer,
    ) -> iced::mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'b>,
        renderer: &Renderer,
        viewport: &iced::Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        file_drop_catcher, readable_text_color, smooth_color_mix, stable_readable_text_color,
    };
    use iced::Color;
    use iced::widget::Space;

    struct TestRenderer;

    impl iced::advanced::Renderer for TestRenderer {
        fn start_layer(&mut self, _bounds: iced::Rectangle) {}
        fn end_layer(&mut self) {}
        fn start_transformation(&mut self, _transformation: iced::Transformation) {}
        fn end_transformation(&mut self) {}
        fn fill_quad(
            &mut self,
            _quad: iced::advanced::renderer::Quad,
            _background: impl Into<iced::Background>,
        ) {
        }
        fn reset(&mut self, _new_bounds: iced::Rectangle) {}
        fn allocate_image(
            &mut self,
            _handle: &iced::advanced::image::Handle,
            _callback: impl FnOnce(
                Result<iced::advanced::image::Allocation, iced::advanced::image::Error>,
            ) + Send
            + 'static,
        ) {
        }
    }

    #[test]
    fn file_drop_catcher_publishes_more_than_100_dropped_files_synchronously() {
        let mut element: iced::Element<'_, std::path::PathBuf, iced::Theme, TestRenderer> =
            file_drop_catcher(Space::new(), |path| path);
        let mut tree = iced::advanced::widget::Tree::new(&element);
        let layout_node = iced::advanced::layout::Node::new(iced::Size::new(800.0, 600.0));
        let layout = iced::advanced::Layout::new(&layout_node);
        let viewport = iced::Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 600.0));
        let renderer = TestRenderer;
        let mut clipboard = iced::advanced::clipboard::Null;
        let mut messages = Vec::new();

        for i in 0..150 {
            let event = iced::Event::Window(iced::window::Event::FileDropped(
                std::path::PathBuf::from(format!("mod_{i:03}.dff")),
            ));
            let mut shell = iced::advanced::Shell::new(&mut messages);
            element.as_widget_mut().update(
                &mut tree,
                &event,
                layout,
                iced::mouse::Cursor::Unavailable,
                &renderer,
                &mut clipboard,
                &mut shell,
                &viewport,
            );
        }

        assert_eq!(messages.len(), 150);
        assert_eq!(messages[0], std::path::PathBuf::from("mod_000.dff"));
        assert_eq!(messages[149], std::path::PathBuf::from("mod_149.dff"));
    }

    #[test]
    fn readable_text_color_handles_light_and_dark_highlights() {
        let light_highlight = Color::from_rgb(0.65, 0.75, 0.50);
        let dark_highlight = Color::from_rgb(0.16, 0.20, 0.18);

        let light_text = readable_text_color(light_highlight, Color::WHITE);
        let dark_text = readable_text_color(dark_highlight, Color::WHITE);

        assert!(light_text.is_readable_on(light_highlight));
        assert!(dark_text.is_readable_on(dark_highlight));
        assert_ne!(light_text, Color::WHITE);
        assert_eq!(dark_text, Color::WHITE);
    }

    #[test]
    fn stable_text_color_does_not_flip_during_a_highlight() {
        let initial = Color::from_rgb(0.12, 0.14, 0.16);
        let peak = Color::from_rgb(0.32, 0.48, 0.34);
        let text = stable_readable_text_color(initial, peak, Color::WHITE);

        assert_eq!(text, Color::WHITE);
        assert!(initial.relative_contrast(text) > initial.relative_contrast(Color::BLACK));
        assert!(peak.relative_contrast(text) > peak.relative_contrast(Color::BLACK));
    }

    #[test]
    fn gruvbox_selection_keeps_a_light_foreground_through_the_pulse() {
        let palette = iced::Theme::GruvboxDark.extended_palette();
        let peak = iced::theme::palette::mix(
            palette.primary.weak.color,
            palette.primary.strong.color,
            0.42,
        );
        let text =
            stable_readable_text_color(palette.primary.weak.color, peak, palette.primary.weak.text);

        assert!(text.relative_luminance() > 0.7);
    }

    #[test]
    fn status_text_remains_readable_through_success_tint() {
        let design = crate::ui::design::Design::dark();
        let normal = design.surface_subtle();
        let toast = design.success_gradient().0;
        let peak = Color {
            r: normal.r + (toast.r - normal.r) * 0.35,
            g: normal.g + (toast.g - normal.g) * 0.35,
            b: normal.b + (toast.b - normal.b) * 0.35,
            a: 1.0,
        };
        let preferred = iced::Theme::GruvboxDark
            .extended_palette()
            .background
            .base
            .text;
        let text = stable_readable_text_color(normal, peak, preferred);

        assert!(normal.relative_contrast(text) >= 4.5);
        assert!(peak.relative_contrast(text) >= 4.5);
        for progress in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let background = Color {
                r: normal.r + (peak.r - normal.r) * progress,
                g: normal.g + (peak.g - normal.g) * progress,
                b: normal.b + (peak.b - normal.b) * progress,
                a: 1.0,
            };
            let animated_text = smooth_color_mix(preferred, text, progress);
            assert!(
                background.relative_contrast(animated_text) >= 4.5,
                "progress {progress} produced insufficient text contrast"
            );
        }
    }

    #[test]
    fn smooth_color_mix_reaches_both_endpoints() {
        let start = Color::from_rgb(0.8, 0.8, 0.8);
        let end = Color::from_rgb(1.0, 1.0, 1.0);

        assert_eq!(smooth_color_mix(start, end, 0.0), start);
        assert_eq!(smooth_color_mix(start, end, 1.0), end);
    }
}
