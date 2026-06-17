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
            Column::new()
                .push(inner)
                .push(
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
            Row::new()
                .push(inner)
                .push(
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
    let elevation = design.tokens.elevation.raised.clone();
    let border = design.iced_border(&elevation);
    let shadow = design.iced_shadow(&elevation);
    let surface = design.surface();
    let radius = design.radius().lg();

    Container::new(content)
        .padding(12)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(surface)),
            border: Border { color: border.color, width: border.width, radius: radius.into() },
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
    let elevation = design.tokens.elevation.floating.clone();
    let border = design.iced_border(&elevation);
    let shadow = design.iced_shadow(&elevation);
    let surface = design.surface();
    let radius = design.radius().xl();

    Container::new(content)
        .padding(16)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(surface)),
            border: Border { color: border.color, width: border.width, radius: radius.into() },
            shadow,
            ..Default::default()
        })
}

/// 3-pixel-wide left accent bar — used to mark the active archive tab.
pub fn accent_bar<'a, Message: 'a>(
    color: Color,
    height: f32,
) -> Container<'a, Message> {
    Container::new(Space::new().width(Length::Fixed(3.0)).height(Length::Fixed(height)))
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(color)),
            border: Border { color: Color::TRANSPARENT, width: 0.0, radius: 0.0.into() },
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
        .padding(Padding { top: 2.0, bottom: 2.0, left: 8.0, right: 8.0 })
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(bg)),
            border: Border { color: Color::TRANSPARENT, width: 0.0, radius: 9999.0.into() },
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
    let elevation = design.tokens.elevation.floating.clone();
    let border = design.iced_border(&elevation);
    let shadow = design.iced_shadow(&elevation);
    let surface = design.surface();
    let radius = design.radius().lg();

    Container::new(content)
        .padding(12)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(surface)),
            border: Border { color: border.color, width: border.width, radius: radius.into() },
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
        border: Border { color: Color::TRANSPARENT, width: 0.0, radius: 4.0.into() },
        ..Default::default()
    })
}

/// Horizontal rule with the design system's border color.
pub fn hairline<'a, Message: 'a>(color: Color) -> Element<'a, Message> {
    rule::horizontal(1).style(move |_theme| iced::widget::rule::Style {
        color,
        radius: 0.0.into(),
        fill_mode: iced::widget::rule::FillMode::Full,
        snap: false,
    }).into()
}

pub fn vhairline<'a, Message: 'a>(color: Color) -> Element<'a, Message> {
    rule::vertical(1).style(move |_theme| iced::widget::rule::Style {
        color,
        radius: 0.0.into(),
        fill_mode: iced::widget::rule::FillMode::Full,
        snap: false,
    }).into()
}

/// Build a styled `Row` for menubar entries.
pub fn menubar_row<'a, Message: 'a>() -> Row<'a, Message> {
    Row::new()
        .spacing(0)
        .padding(Padding { top: 0.0, bottom: 0.0, left: 0.0, right: 0.0 })
        .align_y(Alignment::Center)
}

/// Build a styled `Column` for the root layout with no spacing.
pub fn root_column<'a, Message: 'a>() -> Column<'a, Message> {
    Column::new().spacing(0).width(Length::Fill).height(Length::Fill)
}
