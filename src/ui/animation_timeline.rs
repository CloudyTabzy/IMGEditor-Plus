//! Custom canvas timeline for the animation dock.
//!
//! Draws a ruler, clip markers, the play range, and the playhead, and
//! turns pointer input into transport messages. It never touches the
//! transport itself: the app applies the message to the session so there
//! is exactly one clock owner. Scrub drags keep tracking the pointer
//! outside the widget bounds because the canvas forwards every event to
//! its program.

use std::sync::Arc;

use iced::widget::canvas::{self, Action, Canvas, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Font, Pixels, Point, Rectangle, Size, Theme, mouse};

use crate::inspector::animation::transport::LoopMode;
use crate::ui::app::Message;
use crate::ui::design::design_for_theme;
use crate::ui::viewer3d_widget::SceneHandle;

pub const TIMELINE_HEIGHT: f32 = 48.0;
const RULER_HEIGHT: f32 = 17.0;
const LANE_PAD: f32 = 8.0;
const MARKER_LANE: f32 = 8.0;
const HANDLE_GRAB: f32 = 6.0;

#[derive(Debug, Clone, Copy)]
enum DragKind {
    Scrub,
    RangeStart,
    RangeEnd,
    Pan { last_x: f32 },
}

/// Transient interaction state (owned by the widget tree).
#[derive(Debug, Default)]
pub struct TimelineState {
    /// Visible time window `(start, end)`; `None` shows the whole clip.
    view: Option<(f64, f64)>,
    /// `(clip id, duration)` the current window was chosen for.
    clip_key: Option<(u32, f32)>,
    drag: Option<DragKind>,
}

struct TimelineView {
    clip: usize,
    duration: f64,
    range: (f64, f64),
    shown: f64,
    markers: Vec<(f64, String)>,
    playing: bool,
    loop_mode: LoopMode,
}

pub struct TimelineProgram {
    handle: Arc<SceneHandle>,
}

impl TimelineProgram {
    pub fn new(handle: Arc<SceneHandle>) -> Self {
        Self { handle }
    }

    fn snapshot(&self) -> Option<TimelineView> {
        self.handle.animation_session(|session| TimelineView {
            clip: session.clip.map(|id| id.0 as usize).unwrap_or(usize::MAX),
            duration: session.transport.duration(),
            range: session.transport.range(),
            shown: session.transport.shown_time(),
            markers: session
                .markers()
                .iter()
                .map(|marker| (marker.time as f64, marker.label.clone()))
                .collect(),
            playing: session.is_playing(),
            loop_mode: session.transport.loop_mode(),
        })
    }
}

/// Resolve the visible window against the clip duration.
fn window_for(view: Option<(f64, f64)>, duration: f64) -> (f64, f64) {
    let full = (0.0, duration.max(1e-3));
    match view {
        Some((start, end)) if end > start && start >= 0.0 && end <= duration + 1e-3 => {
            (start.max(0.0), end.min(duration.max(1e-3)))
        }
        _ => full,
    }
}

fn time_to_x(time: f64, window: (f64, f64), width: f32) -> f32 {
    let span = (window.1 - window.0).max(1e-9);
    LANE_PAD + ((time - window.0) / span) as f32 * (width - 2.0 * LANE_PAD).max(1.0)
}

fn x_to_time(x: f32, window: (f64, f64), width: f32) -> f64 {
    let span = window.1 - window.0;
    let usable = (width - 2.0 * LANE_PAD).max(1.0) as f64;
    window.0 + (((x - LANE_PAD) as f64 / usable).clamp(0.0, 1.0)) * span
}

fn format_seconds(seconds: f64) -> String {
    let seconds = seconds.max(0.0);
    if seconds < 10.0 {
        format!("{seconds:.2}s")
    } else if seconds < 100.0 {
        format!("{seconds:.1}s")
    } else {
        format!("{}s", seconds.round() as i64)
    }
}

fn nice_tick_step(span: f64, width: f32) -> f64 {
    const STEPS: [f64; 14] = [
        0.01, 0.02, 0.05, 0.1, 0.2, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0, 300.0,
    ];
    let usable = (width - 2.0 * LANE_PAD).max(1.0) as f64;
    let min_px = 64.0;
    for step in STEPS {
        if step / span * usable >= min_px {
            return step;
        }
    }
    *STEPS.last().unwrap()
}

impl canvas::Program<Message> for TimelineProgram {
    type State = TimelineState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        if bounds.width <= 1.0 {
            return None;
        }
        let view = self.snapshot()?;
        let key = (view.clip as u32, view.duration as f32);
        if state.clip_key != Some(key) {
            state.clip_key = Some(key);
            state.view = None;
            state.drag = None;
        }
        let window = window_for(state.view, view.duration);
        let width = bounds.width;
        let inside = cursor.position_in(bounds).is_some();
        self.handle.set_timeline_hover(inside);

        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let p = cursor.position()?;
                let x = p.x - bounds.x;
                let t = x_to_time(x, window, width);
                let range_start_x = time_to_x(view.range.0, window, width);
                let range_end_x = time_to_x(view.range.1, window, width);
                if (x - range_start_x).abs() <= HANDLE_GRAB && range_start_x > LANE_PAD + 1.0 {
                    state.drag = Some(DragKind::RangeStart);
                    return Some(
                        Action::publish(Message::AnimationRangeDragStart(t)).and_capture(),
                    );
                }
                if (x - range_end_x).abs() <= HANDLE_GRAB && range_end_x < width - LANE_PAD - 1.0 {
                    state.drag = Some(DragKind::RangeEnd);
                    return Some(Action::publish(Message::AnimationRangeDragEnd(t)).and_capture());
                }
                state.drag = Some(DragKind::Scrub);
                Some(Action::publish(Message::AnimationScrubStart(t)).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                if inside {
                    let p = cursor.position()?;
                    state.drag = Some(DragKind::Pan {
                        last_x: p.x - bounds.x,
                    });
                    Some(Action::request_redraw().and_capture())
                } else {
                    None
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let drag = state.drag?;
                let x = position.x - bounds.x;
                match drag {
                    DragKind::Scrub => Some(
                        Action::publish(Message::AnimationScrubTo(x_to_time(x, window, width)))
                            .and_capture(),
                    ),
                    DragKind::RangeStart => Some(
                        Action::publish(Message::AnimationRangeDragStart(x_to_time(
                            x, window, width,
                        )))
                        .and_capture(),
                    ),
                    DragKind::RangeEnd => Some(
                        Action::publish(Message::AnimationRangeDragEnd(x_to_time(
                            x, window, width,
                        )))
                        .and_capture(),
                    ),
                    DragKind::Pan { last_x } => {
                        let dx = x - last_x;
                        if dx.abs() > f32::EPSILON {
                            let span = window.1 - window.0;
                            let usable = (width - 2.0 * LANE_PAD).max(1.0) as f64;
                            let shift = -(dx as f64 / usable) * span;
                            let mut start = (window.0 + shift).clamp(0.0, view.duration - span);
                            if start < 0.0 {
                                start = 0.0;
                            }
                            let end = (start + span).min(view.duration.max(span));
                            state.view = Some((start, end));
                        }
                        state.drag = Some(DragKind::Pan { last_x: x });
                        Some(Action::request_redraw().and_capture())
                    }
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if matches!(state.drag, Some(DragKind::Scrub)) {
                    state.drag = None;
                    return Some(Action::publish(Message::AnimationScrubEnd).and_capture());
                }
                if state.drag.is_some() {
                    state.drag = None;
                    return Some(Action::request_redraw().and_capture());
                }
                None
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
                if state.drag.is_some() {
                    state.drag = None;
                    return Some(Action::request_redraw().and_capture());
                }
                None
            }
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if !inside {
                    return None;
                }
                let p = cursor.position()?;
                let x = p.x - bounds.x;
                let anchor = x_to_time(x, window, width);
                let amount = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 20.0,
                };
                let factor = (-amount as f64 * 0.18).exp();
                let mut span = (window.1 - window.0) * factor;
                span = span.clamp(0.05, view.duration.max(0.05));
                let ratio = ((anchor - window.0) / (window.1 - window.0).max(1e-9)).clamp(0.0, 1.0);
                let mut start = anchor - ratio * span;
                start = start.clamp(0.0, (view.duration - span).max(0.0));
                let end = (start + span).min(view.duration.max(span));
                state.view = Some((start, end));
                Some(Action::request_redraw().and_capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let design = design_for_theme(theme);
        let Some(view) = self.snapshot() else {
            return vec![frame.into_geometry()];
        };
        let window = window_for(state.view, view.duration);
        let width = bounds.width;
        let height = bounds.height;
        let lane_top = RULER_HEIGHT;
        let lane_height = (height - RULER_HEIGHT).max(1.0);

        frame.fill_rectangle(Point::ORIGIN, Size::new(width, height), design.chrome());
        frame.fill_rectangle(
            Point::new(LANE_PAD, lane_top),
            Size::new((width - 2.0 * LANE_PAD).max(1.0), lane_height),
            design.surface_subtle(),
        );

        // Dim the regions outside the play range, then highlight it.
        let range_start_x = time_to_x(view.range.0, window, width);
        let range_end_x = time_to_x(view.range.1, window, width);
        let dim = Color {
            a: 0.35,
            ..design.page()
        };
        frame.fill_rectangle(
            Point::new(LANE_PAD, lane_top),
            Size::new((range_start_x - LANE_PAD).max(0.0), lane_height),
            dim,
        );
        frame.fill_rectangle(
            Point::new(range_end_x, lane_top),
            Size::new((width - LANE_PAD - range_end_x).max(0.0), lane_height),
            dim,
        );
        frame.fill_rectangle(
            Point::new(range_start_x, lane_top),
            Size::new((range_end_x - range_start_x).max(0.0), lane_height),
            design.accent_weak(),
        );

        // Ruler ticks and labels.
        let step = nice_tick_step(window.1 - window.0, width);
        let first = (window.0 / step).ceil() * step;
        let mut tick = first;
        while tick <= window.1 + 1e-9 {
            let x = time_to_x(tick, window, width);
            frame.stroke(
                &Path::line(Point::new(x, RULER_HEIGHT - 5.0), Point::new(x, height)),
                Stroke::default()
                    .with_width(1.0)
                    .with_color(design.divider()),
            );
            frame.fill_text(Text {
                content: format_seconds(tick),
                position: Point::new(x + 3.0, 1.0),
                color: design.text_muted(),
                size: Pixels(10.0),
                font: Font::MONOSPACE,
                ..Text::default()
            });
            tick += step;
        }

        // Clip markers as small diamonds on the marker lane.
        for (time, label) in &view.markers {
            let x = time_to_x(*time, window, width);
            let y = lane_top + MARKER_LANE;
            let diamond = Path::new(|path| {
                path.move_to(Point::new(x, y - 4.0));
                path.line_to(Point::new(x + 4.0, y));
                path.line_to(Point::new(x, y + 4.0));
                path.line_to(Point::new(x - 4.0, y));
                path.close();
            });
            frame.fill(&diamond, design.warning());
            if let Some(cursor) = cursor.position()
                && (cursor.x - (bounds.x + x)).abs() <= 6.0
                && cursor.y >= bounds.y
            {
                frame.fill_text(Text {
                    content: label.clone(),
                    position: Point::new(x + 6.0, y + 6.0),
                    color: design.text(),
                    size: Pixels(11.0),
                    font: Font::DEFAULT,
                    ..Text::default()
                });
            }
        }

        // Range grips.
        for x in [range_start_x, range_end_x] {
            frame.fill_rectangle(
                Point::new(x - 1.0, lane_top - 3.0),
                Size::new(2.0, lane_height + 3.0),
                design.accent(),
            );
        }

        // Playhead.
        let play_x = time_to_x(view.shown, window, width);
        frame.stroke(
            &Path::line(Point::new(play_x, 0.0), Point::new(play_x, height)),
            Stroke::default()
                .with_width(1.5)
                .with_color(design.accent()),
        );
        let head = Path::new(|path| {
            path.move_to(Point::new(play_x - 5.0, 0.0));
            path.line_to(Point::new(play_x + 5.0, 0.0));
            path.line_to(Point::new(play_x, 7.0));
            path.close();
        });
        frame.fill(&head, design.accent());

        // Live time readout during a scrub.
        if matches!(state.drag, Some(DragKind::Scrub)) {
            frame.fill_text(Text {
                content: format!("{:.2}s", view.shown),
                position: Point::new(play_x + 6.0, lane_top + 2.0),
                color: design.text(),
                size: Pixels(11.0),
                font: Font::MONOSPACE,
                ..Text::default()
            });
        }

        let _ = view.playing;
        let _ = view.loop_mode;
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if matches!(state.drag, Some(DragKind::Pan { .. })) {
            return mouse::Interaction::Grabbing;
        }
        let Some(p) = cursor.position_in(bounds) else {
            return mouse::Interaction::Idle;
        };
        let Some(view) = self.snapshot() else {
            return mouse::Interaction::Idle;
        };
        let window = window_for(state.view, view.duration);
        let x = p.x;
        let range_start_x = time_to_x(view.range.0, window, bounds.width);
        let range_end_x = time_to_x(view.range.1, window, bounds.width);
        if (x - range_start_x).abs() <= HANDLE_GRAB || (x - range_end_x).abs() <= HANDLE_GRAB {
            return mouse::Interaction::ResizingHorizontally;
        }
        if p.y >= RULER_HEIGHT {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::Idle
        }
    }
}

/// Build the timeline canvas element for the animation dock.
pub fn timeline(handle: Arc<SceneHandle>) -> Canvas<TimelineProgram, Message> {
    Canvas::new(TimelineProgram::new(handle))
        .width(iced::Length::Fill)
        .height(iced::Length::Fixed(TIMELINE_HEIGHT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_round_trips_through_x() {
        let window = (1.0, 3.0);
        let width = 200.0;
        for time in [1.0, 1.5, 2.0, 2.999] {
            let x = time_to_x(time, window, width);
            let back = x_to_time(x, window, width);
            assert!((back - time).abs() < 1e-4, "{time} -> {x} -> {back}");
        }
    }

    #[test]
    fn tick_step_keeps_a_readable_spacing() {
        let span = 2.0;
        let width = 400.0;
        let step = nice_tick_step(span, width);
        let spacing = step / span * f64::from(width - 2.0 * LANE_PAD);
        assert!(spacing >= 64.0, "tick spacing {spacing} too tight");
    }

    #[test]
    fn window_falls_back_to_the_full_clip() {
        assert_eq!(window_for(None, 2.0), (0.0, 2.0));
        assert_eq!(window_for(Some((0.5, 1.5)), 2.0), (0.5, 1.5));
        assert_eq!(window_for(Some((5.0, 6.0)), 2.0), (0.0, 2.0));
    }
}
