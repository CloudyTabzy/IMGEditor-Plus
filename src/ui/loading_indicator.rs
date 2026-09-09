//! Small, theme-aware indeterminate loading indicators.

use iced::widget::canvas;
use iced::{Color, Point, Rectangle, Theme, mouse};

const DOT_COUNT: usize = 12;

/// A compact spinning indicator for work whose exact progress cannot be
/// measured without misleading the user.
#[derive(Debug, Clone, Copy)]
pub struct LoadingSpinner {
    phase: f32,
}

impl LoadingSpinner {
    pub fn new(phase: f32) -> Self {
        Self {
            phase: phase.rem_euclid(1.0),
        }
    }
}

fn dot_opacity(index: usize, phase: f32) -> f32 {
    let offset = (index as f32 / DOT_COUNT as f32 - phase).rem_euclid(1.0);
    0.12 + 0.88 * (1.0 - offset).powi(3)
}

impl<Message> canvas::Program<Message> for LoadingSpinner {
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
        let diameter = bounds.width.min(bounds.height);
        if diameter <= 0.0 {
            return vec![frame.into_geometry()];
        }

        let center = frame.center();
        let ring_radius = diameter * 0.32;
        let dot_radius = (diameter * 0.065).clamp(1.5, 3.5);
        let base = theme.extended_palette().primary.strong.color;
        for index in 0..DOT_COUNT {
            let angle = std::f32::consts::TAU * index as f32 / DOT_COUNT as f32
                - std::f32::consts::FRAC_PI_2;
            let color = Color {
                a: base.a * dot_opacity(index, self.phase),
                ..base
            };
            let point = Point::new(
                center.x + angle.cos() * ring_radius,
                center.y + angle.sin() * ring_radius,
            );
            frame.fill(&canvas::Path::circle(point, dot_radius), color);
        }

        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_spinner_dot_is_brightest() {
        assert!(dot_opacity(0, 0.0) > dot_opacity(1, 0.0));
        assert!(dot_opacity(1, 0.0) > dot_opacity(2, 0.0));
    }

    #[test]
    fn spinner_phase_wraps_without_a_visual_jump() {
        let wrapped = LoadingSpinner::new(1.25);
        let direct = LoadingSpinner::new(0.25);
        assert_eq!(wrapped.phase, direct.phase);
        assert!((0.12..=1.0).contains(&dot_opacity(6, wrapped.phase)));
    }
}
