//! Small, clipped interaction effects used by the desktop UI.

use iced::widget::canvas;
use iced::{Color, Point, Rectangle, Theme, mouse};

/// The current state needed to draw one expanding click ripple.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RippleVisual {
    pub(crate) origin: Option<Point>,
    pub(crate) progress: f32,
}

/// Canvas program for a short-lived expanding wave. The canvas is layered on
/// the target widget, so its bounds provide the clipping region automatically.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RippleOverlay {
    pub(crate) origin: Option<Point>,
    pub(crate) progress: f32,
}

pub(crate) fn ripple_overlay<Message>(
    visual: RippleVisual,
) -> canvas::Canvas<RippleOverlay, Message> {
    canvas::Canvas::new(RippleOverlay {
        origin: visual.origin,
        progress: visual.progress,
    })
}

impl<Message> canvas::Program<Message> for RippleOverlay {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return vec![frame.into_geometry()];
        }

        let progress = self.progress.clamp(0.0, 1.0);
        let center = self
            .origin
            .filter(|point| point.x.is_finite() && point.y.is_finite() && bounds.contains(*point))
            .map(|point| Point::new(point.x - bounds.x, point.y - bounds.y))
            .unwrap_or_else(|| Point::new(bounds.width / 2.0, bounds.height / 2.0));
        let diagonal = bounds
            .width
            .mul_add(bounds.width, bounds.height * bounds.height)
            .sqrt();
        let radius = (diagonal * progress).max(0.5);
        let fill_alpha = (0.24 * (1.0 - progress)).clamp(0.0, 0.24);
        let color = theme.extended_palette().primary.base.color;
        let edge = theme.extended_palette().primary.strong.color;

        frame.fill(
            &canvas::Path::circle(center, radius),
            Color::from_rgba(color.r, color.g, color.b, fill_alpha),
        );
        frame.stroke(
            &canvas::Path::circle(center, radius),
            canvas::Stroke::default()
                .with_width(2.0)
                .with_color(Color::from_rgba(
                    edge.r,
                    edge.g,
                    edge.b,
                    (0.78 * (1.0 - progress)).clamp(0.0, 0.78),
                )),
        );

        vec![frame.into_geometry()]
    }
}
