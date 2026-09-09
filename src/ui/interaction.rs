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

/// Canvas program for the "sheen" sweep that travels across the filled part
/// of a progress bar while a task runs. `phase` is a 0..1 repeating clock;
/// `value` is the bar's current fill fraction so the sweep never draws on
/// the unfilled track.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ShimmerOverlay {
    pub(crate) phase: f32,
    pub(crate) value: f32,
}

pub(crate) fn shimmer_overlay<Message>(
    phase: f32,
    value: f32,
) -> canvas::Canvas<ShimmerOverlay, Message> {
    canvas::Canvas::new(ShimmerOverlay { phase, value })
}

impl<Message> canvas::Program<Message> for ShimmerOverlay {
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
        let fill_width = bounds.width * self.value.clamp(0.0, 1.0);
        if fill_width <= 0.0 || bounds.height <= 0.0 {
            return vec![frame.into_geometry()];
        }

        const STRIPE_WIDTH: f32 = 28.0;
        const SLANT: f32 = 10.0;
        let travel = fill_width + STRIPE_WIDTH * 2.0;
        let center = self.phase.clamp(0.0, 1.0) * travel - STRIPE_WIDTH;

        // The sheen uses the theme's base text color at low alpha so it
        // reads as a light sweep on dark fills and a soft shadow sweep on
        // light fills without a per-theme constant.
        let sheen = theme.extended_palette().background.base.text;
        let color = Color::from_rgba(sheen.r, sheen.g, sheen.b, 0.16);

        frame.with_clip(
            Rectangle::new(
                Point::ORIGIN,
                iced::Size::new(fill_width, bounds.height),
            ),
            |clipped| {
                let stripe = canvas::Path::new(|builder| {
                    builder.move_to(Point::new(center - SLANT, bounds.height));
                    builder.line_to(Point::new(center + STRIPE_WIDTH - SLANT, bounds.height));
                    builder.line_to(Point::new(center + STRIPE_WIDTH, 0.0));
                    builder.line_to(Point::new(center, 0.0));
                    builder.close();
                });
                clipped.fill(&stripe, color);
            },
        );

        vec![frame.into_geometry()]
    }
}
