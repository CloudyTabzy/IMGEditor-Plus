//! Custom window chrome for the frameless main window: the draggable title
//! area, the caption buttons, and the invisible resize edges.
//!
//! winit keeps the native caption/sizing window styles on an undecorated
//! window and only zeroes the non-client area, so Aero Snap, Win+Arrow,
//! minimize/maximize animations and the Alt+Space system menu keep working.
//! What it does not provide is hit-testing for the missing frame, so moving
//! and resizing are started from Iced through `window::drag` and
//! `window::drag_resize`, which enter the native move/size loops.

use std::sync::LazyLock;

use iced::widget::{Space, button, column, container, image, mouse_area, row};
use iced::window::Direction;
use iced::{Alignment, Background, Color, Element, Length, Point, mouse};

use crate::ui::design::Design;
use crate::ui::fonts;

/// Height of the merged menu/title bar.
pub const TITLE_BAR_HEIGHT: f32 = 32.0;
/// Width of each caption button, matching the Windows 11 caption metrics.
const CAPTION_BUTTON_WIDTH: f32 = 46.0;
/// Width of the minimize/maximize/close group.
pub const CAPTION_BUTTONS_WIDTH: f32 = CAPTION_BUTTON_WIDTH * 3.0;
/// Thickness of the invisible resize strips along the window edges. Kept
/// thin because they sit inside the client area, over real content such as
/// the entry table's scrollbar.
const RESIZE_EDGE: f32 = 4.0;
/// Corner grabs are larger than the edges so diagonal resizing is easy to
/// hit without widening the edge strips.
const RESIZE_CORNER: f32 = 10.0;
/// Pointer travel before a press on the title area becomes a window move,
/// matching the Windows `SM_CXDRAG` default. Below it the press stays a
/// click, so double-click-to-maximize is never swallowed by the move loop.
const DRAG_THRESHOLD: f32 = 4.0;
/// Standard Windows close-button hover color.
const CLOSE_HOVER: Color = Color::from_rgb(0.769, 0.169, 0.110);
const LOGO_SIZE: u32 = 18;

#[derive(Debug, Clone, Copy)]
pub enum ChromeMessage {
    /// Pointer moved over the title area (position relative to it).
    TitleHovered(Point),
    TitlePressed,
    TitleReleased,
    TitleDoubleClicked,
    TitleRightClicked,
    Minimize,
    ToggleMaximize,
    Close,
    ResizeStart(Direction),
    /// The window was resized; re-query whether it is maximized.
    Resized,
    MaximizedChanged(bool),
}

/// Press-then-move tracking for the title area.
#[derive(Debug, Default, Clone, Copy)]
pub struct TitleDrag {
    hover: Option<Point>,
    anchor: Option<Point>,
}

impl TitleDrag {
    pub fn press(&mut self) {
        self.anchor = self.hover;
    }

    pub fn release(&mut self) {
        self.anchor = None;
    }

    /// Record a pointer position; returns `true` exactly once per press,
    /// when the pointer has travelled far enough to start a window move.
    pub fn hover(&mut self, position: Point) -> bool {
        self.hover = Some(position);
        match self.anchor {
            Some(anchor) if anchor.distance(position) >= DRAG_THRESHOLD => {
                self.anchor = None;
                true
            }
            _ => false,
        }
    }
}

/// The title-bar logo, downscaled once: a handle built inside `view` would
/// get a fresh id every frame and re-upload the texture each redraw.
static LOGO: LazyLock<image::Handle> = LazyLock::new(|| {
    let bytes = include_bytes!("../../asset/logo/IMGEditorLogo.png");
    let size = LOGO_SIZE * 2;
    match ::image::load_from_memory_with_format(bytes, ::image::ImageFormat::Png) {
        Ok(logo) => {
            let logo = logo
                .resize(size, size, ::image::imageops::FilterType::Lanczos3)
                .to_rgba8();
            let (width, height) = logo.dimensions();
            image::Handle::from_rgba(width, height, logo.into_raw())
        }
        Err(_) => image::Handle::from_rgba(1, 1, vec![0, 0, 0, 0]),
    }
});

pub fn logo<'a, Message: 'a>() -> Element<'a, Message> {
    container(
        image(LOGO.clone())
            .width(Length::Fixed(LOGO_SIZE as f32))
            .height(Length::Fixed(LOGO_SIZE as f32)),
    )
    .padding([0, 10])
    .height(Length::Fill)
    .align_y(Alignment::Center)
    .into()
}

/// The empty, draggable middle of the bar with the centered window title.
pub fn title_area<'a, Message: Clone + 'a>(
    design: &Design,
    title: String,
    wrap: impl Fn(ChromeMessage) -> Message + Copy + 'a,
) -> Element<'a, Message> {
    let muted = design.text_muted();
    let label = container(fonts::caption(title).color(muted))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .align_y(Alignment::Center)
        .clip(true);
    drag_region(label, wrap)
}

/// Window controls kept usable while a modal is open. Modal scrims swallow
/// every event beneath them, so this layer sits above the scrim: the whole
/// bar drags the window (the menus stay blocked underneath) and the caption
/// buttons keep working, with Close still routed through the unsaved-changes
/// guard.
pub fn modal_controls<'a, Message: Clone + 'a>(
    design: &Design,
    maximized: bool,
    wrap: impl Fn(ChromeMessage) -> Message + Copy + 'a,
) -> Element<'a, Message> {
    row![
        drag_region(Space::new().width(Length::Fill).height(Length::Fill), wrap),
        caption_buttons(design, maximized, wrap),
    ]
    .width(Length::Fill)
    .height(Length::Fixed(TITLE_BAR_HEIGHT))
    .into()
}

fn drag_region<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    wrap: impl Fn(ChromeMessage) -> Message + Copy + 'a,
) -> Element<'a, Message> {
    mouse_area(content)
        .on_move(move |position| wrap(ChromeMessage::TitleHovered(position)))
        .on_press(wrap(ChromeMessage::TitlePressed))
        .on_release(wrap(ChromeMessage::TitleReleased))
        .on_double_click(wrap(ChromeMessage::TitleDoubleClicked))
        .on_right_press(wrap(ChromeMessage::TitleRightClicked))
        .into()
}

/// Minimize, maximize/restore and close.
pub fn caption_buttons<'a, Message: Clone + 'a>(
    design: &Design,
    maximized: bool,
    wrap: impl Fn(ChromeMessage) -> Message + Copy + 'a,
) -> Element<'a, Message> {
    let text = design.text();
    let hover = design.hover_overlay();
    let caption = |icon: iced::widget::Text<'a>, message: ChromeMessage, close: bool| {
        button(
            container(icon.size(14))
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .width(Length::Fixed(CAPTION_BUTTON_WIDTH))
        .height(Length::Fill)
        .padding(0)
        .on_press(wrap(message))
        .style(move |_, status| {
            let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let (background, foreground) = match (active, close) {
                (true, true) => (
                    Some(Background::Color(if status == button::Status::Pressed {
                        CLOSE_HOVER.scale_alpha(0.85)
                    } else {
                        CLOSE_HOVER
                    })),
                    Color::WHITE,
                ),
                (true, false) => (Some(Background::Color(hover)), text),
                (false, _) => (None, text),
            };
            button::Style {
                background,
                text_color: foreground,
                ..button::Style::default()
            }
        })
    };
    let maximize_icon = if maximized {
        iced_fonts::lucide::copy()
    } else {
        iced_fonts::lucide::square()
    };
    row![
        caption(iced_fonts::lucide::minus(), ChromeMessage::Minimize, false),
        caption(maximize_icon, ChromeMessage::ToggleMaximize, false),
        caption(iced_fonts::lucide::x(), ChromeMessage::Close, true),
    ]
    .height(Length::Fill)
    .into()
}

/// Invisible resize strips along every edge and corner. Everything between
/// them is empty space, which reports no mouse interaction, so the `Stack`
/// passes the pointer through to the application underneath.
pub fn resize_edges<'a, Message: Clone + 'a>(
    wrap: impl Fn(ChromeMessage) -> Message + Copy + 'a,
) -> Element<'a, Message> {
    let grip = |width: Length, height: Length, direction: Direction| -> Element<'a, Message> {
        let interaction = match direction {
            Direction::North | Direction::South => mouse::Interaction::ResizingVertically,
            Direction::East | Direction::West => mouse::Interaction::ResizingHorizontally,
            Direction::NorthWest | Direction::SouthEast => {
                mouse::Interaction::ResizingDiagonallyDown
            }
            Direction::NorthEast | Direction::SouthWest => mouse::Interaction::ResizingDiagonallyUp,
        };
        mouse_area(Space::new().width(width).height(height))
            .interaction(interaction)
            .on_press(wrap(ChromeMessage::ResizeStart(direction)))
            .into()
    };
    let corner = Length::Fixed(RESIZE_CORNER);
    let edge = Length::Fixed(RESIZE_EDGE);
    let top = row![
        grip(corner, corner, Direction::NorthWest),
        column![grip(Length::Fill, edge, Direction::North)].width(Length::Fill),
        grip(corner, corner, Direction::NorthEast),
    ];
    let middle = row![
        grip(edge, Length::Fill, Direction::West),
        Space::new().width(Length::Fill).height(Length::Fill),
        grip(edge, Length::Fill, Direction::East),
    ]
    .height(Length::Fill);
    let bottom = row![
        grip(corner, corner, Direction::SouthWest),
        column![
            Space::new().height(Length::Fixed(RESIZE_CORNER - RESIZE_EDGE)),
            grip(Length::Fill, edge, Direction::South),
        ]
        .width(Length::Fill),
        grip(corner, corner, Direction::SouthEast),
    ];
    column![top, middle, bottom]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_without_travel_never_starts_a_move() {
        let mut drag = TitleDrag::default();
        assert!(!drag.hover(Point::new(100.0, 10.0)));
        drag.press();
        assert!(!drag.hover(Point::new(101.0, 11.0)));
        drag.release();
        assert!(!drag.hover(Point::new(140.0, 10.0)));
    }

    #[test]
    fn travel_past_the_threshold_starts_one_move_per_press() {
        let mut drag = TitleDrag::default();
        drag.hover(Point::new(100.0, 10.0));
        drag.press();
        assert!(drag.hover(Point::new(100.0 + DRAG_THRESHOLD, 10.0)));
        assert!(!drag.hover(Point::new(150.0, 10.0)));
    }

    #[test]
    fn moving_without_a_press_is_ignored() {
        let mut drag = TitleDrag::default();
        drag.hover(Point::new(0.0, 0.0));
        assert!(!drag.hover(Point::new(300.0, 0.0)));
    }
}
