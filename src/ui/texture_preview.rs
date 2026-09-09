//! Shared helpers for texture-tab previews and optional NIF UV overlays.

use std::collections::HashSet;
use std::path::Path;

use iced::widget::{canvas, image};
use iced::{Color, Point, Rectangle, Size, Theme, Vector, mouse};

use crate::inspector::scene3d::mesh::SceneMesh;
use crate::inspector::scene3d::scene::Scene;
use crate::parser::DecodedTexture;

pub type UvTriangle = [[f32; 2]; 3];

/// A texture viewport layer that owns the shared image navigation behavior.
/// The texture tab stacks two instances of this program: one raster layer and
/// one overlay layer. Keeping their navigation logic identical makes zooming
/// and panning apply identically to the texture, grid, and UV topology.
#[derive(Debug, Clone)]
pub struct TextureViewport {
    pub handle: image::Handle,
    pub image_width: u32,
    pub image_height: u32,
    pub render_image: bool,
    pub show_grid: bool,
    pub grid_divisions: u32,
    pub show_uv: bool,
    pub uv_triangles: Vec<UvTriangle>,
}

/// Local interaction state for [`TextureViewport`]. The handle is retained so
/// a newly selected texture starts with a clean fit instead of inheriting the
/// previous texture's zoom and pan.
#[derive(Debug, Clone)]
pub struct TextureViewportState {
    scale: f32,
    starting_offset: Vector,
    current_offset: Vector,
    cursor_grabbed_at: Option<Point>,
    handle: Option<image::Handle>,
}

impl Default for TextureViewportState {
    fn default() -> Self {
        Self {
            scale: 1.0,
            starting_offset: Vector::default(),
            current_offset: Vector::default(),
            cursor_grabbed_at: None,
            handle: None,
        }
    }
}

impl TextureViewportState {
    fn reset_for(&mut self, handle: &image::Handle) {
        if self.handle.as_ref() == Some(handle) {
            return;
        }
        self.scale = 1.0;
        self.starting_offset = Vector::default();
        self.current_offset = Vector::default();
        self.cursor_grabbed_at = None;
        self.handle = Some(handle.clone());
    }

    fn matches(&self, handle: &image::Handle) -> bool {
        self.handle.as_ref() == Some(handle)
    }
}

const TEXTURE_MIN_SCALE: f32 = 0.25;
const TEXTURE_MAX_SCALE: f32 = 10.0;
const TEXTURE_SCALE_STEP: f32 = 0.10;

impl<Message: 'static> canvas::Program<Message> for TextureViewport {
    type State = TextureViewportState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        state.reset_for(&self.handle);

        match event {
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let Some(cursor_position) = cursor.position_over(bounds) else {
                    return None;
                };
                let y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                        *y
                    }
                };
                let previous_scale = state.scale;
                let can_zoom = (y < 0.0 && previous_scale > TEXTURE_MIN_SCALE)
                    || (y > 0.0 && previous_scale < TEXTURE_MAX_SCALE);

                if can_zoom {
                    state.scale = (if y > 0.0 {
                        state.scale * (1.0 + TEXTURE_SCALE_STEP)
                    } else {
                        state.scale / (1.0 + TEXTURE_SCALE_STEP)
                    })
                    .clamp(TEXTURE_MIN_SCALE, TEXTURE_MAX_SCALE);

                    let scaled_size = texture_image_size(
                        self.image_width,
                        self.image_height,
                        bounds.size(),
                        state.scale,
                    );
                    let factor = state.scale / previous_scale - 1.0;
                    let cursor_to_center = cursor_position - bounds.center();
                    let adjustment = cursor_to_center * factor + state.current_offset * factor;

                    state.current_offset = Vector::new(
                        if scaled_size.width > bounds.width {
                            state.current_offset.x + adjustment.x
                        } else {
                            0.0
                        },
                        if scaled_size.height > bounds.height {
                            state.current_offset.y + adjustment.y
                        } else {
                            0.0
                        },
                    );
                }

                let action = canvas::Action::request_redraw();
                Some(if self.render_image {
                    action.and_capture()
                } else {
                    action
                })
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(cursor_position) = cursor.position_over(bounds) else {
                    return None;
                };
                state.cursor_grabbed_at = Some(cursor_position);
                state.starting_offset = state.current_offset;
                if self.render_image {
                    Some(canvas::Action::capture())
                } else {
                    None
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.cursor_grabbed_at.take().is_some() {
                    self.render_image.then_some(canvas::Action::capture())
                } else {
                    None
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let Some(origin) = state.cursor_grabbed_at else {
                    return None;
                };
                let scaled_size = texture_image_size(
                    self.image_width,
                    self.image_height,
                    bounds.size(),
                    state.scale,
                );
                let hidden_width = (scaled_size.width - bounds.width / 2.0).max(0.0).round();
                let hidden_height = (scaled_size.height - bounds.height / 2.0).max(0.0).round();
                let delta = *position - origin;
                let x = if bounds.width < scaled_size.width {
                    (state.starting_offset.x - delta.x).clamp(-hidden_width, hidden_width)
                } else {
                    0.0
                };
                let y = if bounds.height < scaled_size.height {
                    (state.starting_offset.y - delta.y).clamp(-hidden_height, hidden_height)
                } else {
                    0.0
                };
                state.current_offset = Vector::new(x, y);
                let action = canvas::Action::request_redraw();
                Some(if self.render_image {
                    action.and_capture()
                } else {
                    action
                })
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let default_state = TextureViewportState::default();
        let state = if state.matches(&self.handle) {
            state
        } else {
            &default_state
        };
        let image_rect = texture_image_rect(
            self.image_width,
            self.image_height,
            bounds.size(),
            state.scale,
            state.current_offset,
        );

        if self.render_image {
            let mut frame = canvas::Frame::new(renderer, bounds.size());
            frame.draw_image(image_rect, canvas::Image::new(&self.handle).snap(true));
            vec![frame.into_geometry()]
        } else {
            let mut frame = canvas::Frame::new(renderer, bounds.size());
            if self.show_grid {
                draw_grid(&mut frame, image_rect, self.grid_divisions);
            } else if self.show_uv {
                draw_image_border(&mut frame, image_rect);
            }
            if self.show_uv {
                draw_uv_triangles(&mut frame, image_rect, &self.uv_triangles);
            }
            vec![frame.into_geometry()]
        }
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if !self.render_image {
            return mouse::Interaction::None;
        }
        let is_grabbed = state.matches(&self.handle) && state.cursor_grabbed_at.is_some();
        if is_grabbed {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::None
        }
    }
}

fn texture_image_size(image_width: u32, image_height: u32, available: Size, scale: f32) -> Size {
    let base = contain_rect(image_width, image_height, available);
    Size::new(base.width * scale, base.height * scale)
}

fn clamp_offset(offset: Vector, image_size: Size, available: Size) -> Vector {
    let hidden_width = (image_size.width - available.width / 2.0).max(0.0).round();
    let hidden_height = (image_size.height - available.height / 2.0)
        .max(0.0)
        .round();
    Vector::new(
        offset.x.clamp(-hidden_width, hidden_width),
        offset.y.clamp(-hidden_height, hidden_height),
    )
}

fn texture_image_rect(
    image_width: u32,
    image_height: u32,
    available: Size,
    scale: f32,
    offset: Vector,
) -> Rectangle {
    let image_size = texture_image_size(image_width, image_height, available, scale);
    let offset = clamp_offset(offset, image_size, available);
    Rectangle::new(
        Point::new(
            (available.width - image_size.width) * 0.5 - offset.x,
            (available.height - image_size.height) * 0.5 - offset.y,
        ),
        image_size,
    )
}

fn draw_uv_triangles(frame: &mut canvas::Frame, image_rect: Rectangle, triangles: &[UvTriangle]) {
    let stroke = canvas::Stroke::default()
        .with_color(Color::from_rgba(1.0, 0.84, 0.22, 0.95))
        .with_width(1.2)
        .with_line_join(canvas::LineJoin::Round)
        .with_line_cap(canvas::LineCap::Round);
    for triangle in triangles {
        for edge in [(0, 1), (1, 2), (2, 0)] {
            for segment in wrapped_uv_edge(triangle[edge.0], triangle[edge.1]) {
                let points = segment.map(|uv| uv_to_point(image_rect, uv));
                frame.stroke(&canvas::Path::line(points[0], points[1]), stroke);
            }
        }
    }
}

/// Every Nth internal grid line renders as a major line.
const GRID_MAJOR_EVERY: u32 = 4;

// The grid is drawn in two passes — a dark shadow stroke under a
// bright stroke — because textures come in every color and a single
// grid color is invisible on half of them.
const GRID_MINOR_DARK: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.25);
const GRID_MINOR_LIGHT: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.28);
const GRID_MAJOR_DARK: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.45);
const GRID_MAJOR_LIGHT: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.45);

fn draw_grid(frame: &mut canvas::Frame, rect: Rectangle, divisions: u32) {
    let divisions = divisions.max(1);
    let minor = (GRID_MINOR_DARK, GRID_MINOR_LIGHT, 1.6, 1.0, false);
    let major = (GRID_MAJOR_DARK, GRID_MAJOR_LIGHT, 2.4, 1.4, true);

    for (dark, light, dark_width, light_width, is_major) in [minor, major] {
        for (offset, line_is_major) in grid_line_offsets(rect.width, divisions) {
            if line_is_major != is_major {
                continue;
            }
            let x = rect.x + offset;
            for (color, width) in [(dark, dark_width), (light, light_width)] {
                frame.stroke(
                    &canvas::Path::line(Point::new(x, rect.y), Point::new(x, rect.y + rect.height)),
                    canvas::Stroke::default()
                        .with_color(color)
                        .with_width(width),
                );
            }
        }
        for (offset, line_is_major) in grid_line_offsets(rect.height, divisions) {
            if line_is_major != is_major {
                continue;
            }
            let y = rect.y + offset;
            for (color, width) in [(dark, dark_width), (light, light_width)] {
                frame.stroke(
                    &canvas::Path::line(Point::new(rect.x, y), Point::new(rect.x + rect.width, y)),
                    canvas::Stroke::default()
                        .with_color(color)
                        .with_width(width),
                );
            }
        }
    }

    draw_image_border(frame, rect);
}

fn draw_image_border(frame: &mut canvas::Frame, rect: Rectangle) {
    // Image border: one crisp outline on top of everything.
    frame.stroke_rectangle(
        Point::new(rect.x, rect.y),
        rect.size(),
        canvas::Stroke::default()
            .with_color(GRID_MAJOR_LIGHT)
            .with_width(1.2),
    );
}

/// Interior grid line offsets across `length` screen px for
/// `divisions` cells. Returns `(offset, is_major)` pairs; the outer
/// edges (0 and `length`) are not included — `draw_grid` strokes the
/// image border separately.
fn grid_line_offsets(length: f32, divisions: u32) -> Vec<(f32, bool)> {
    if length <= 0.0 {
        return Vec::new();
    }
    (1..divisions.max(1))
        .map(|i| {
            (
                length * i as f32 / divisions as f32,
                i % GRID_MAJOR_EVERY == 0,
            )
        })
        .collect()
}

/// Compute the same centered `Contain` rectangle used by the plain Iced image
/// widget. UV v-coordinates are kept in their existing NIF convention: the
/// current Bully renderer passes them directly to the texture sampler, so
/// `v = 0` maps to the top row shown in the preview.
pub fn contain_rect(image_width: u32, image_height: u32, available: Size) -> Rectangle {
    if image_width == 0 || image_height == 0 || available.width <= 0.0 || available.height <= 0.0 {
        return Rectangle::new(Point::ORIGIN, Size::ZERO);
    }
    let image_ratio = image_width as f32 / image_height as f32;
    let available_ratio = available.width / available.height;
    let (width, height) = if available_ratio > image_ratio {
        (available.height * image_ratio, available.height)
    } else {
        (available.width, available.width / image_ratio)
    };
    Rectangle::new(
        Point::new(
            (available.width - width) * 0.5,
            (available.height - height) * 0.5,
        ),
        Size::new(width, height),
    )
}

fn uv_to_point(image_rect: Rectangle, uv: [f32; 2]) -> Point {
    Point::new(
        image_rect.x + uv[0] * image_rect.width,
        image_rect.y + uv[1] * image_rect.height,
    )
}

const UV_EPSILON: f32 = 1e-6;
const MAX_UV_TILE_CROSSES: usize = 256;

/// Split a UV edge at texture-repeat boundaries and return the portions that
/// belong to the displayed [0, 1] × [0, 1] texture tile.
///
/// NIF assets commonly use repeated UVs (for example, a door texture may be
/// tiled twice across a mesh). The 3D sampler repeats those coordinates, but
/// drawing the raw endpoints would place the overlay outside the image and
/// hide most of the useful topology. Splitting before wrapping preserves the
/// seam locations; simply taking `fract` of both endpoints would connect the
/// wrong corners across a repeat boundary.
fn wrapped_uv_edge(start: [f32; 2], end: [f32; 2]) -> Vec<[[f32; 2]; 2]> {
    if !start
        .iter()
        .chain(end.iter())
        .all(|value| value.is_finite())
    {
        return Vec::new();
    }

    if start
        .iter()
        .chain(end.iter())
        .all(|value| *value >= -UV_EPSILON && *value <= 1.0 + UV_EPSILON)
    {
        return vec![[
            [clamp_uv(start[0]), clamp_uv(start[1])],
            [clamp_uv(end[0]), clamp_uv(end[1])],
        ]];
    }

    let mut cuts = vec![0.0, 1.0];
    for axis in 0..2 {
        let delta = end[axis] - start[axis];
        if delta.abs() <= UV_EPSILON {
            continue;
        }
        let lower = start[axis].min(end[axis]).floor() + 1.0;
        let upper = start[axis].max(end[axis]).ceil();
        let mut boundary = lower;
        let mut crossings = 0;
        while boundary < upper && crossings < MAX_UV_TILE_CROSSES {
            let t = (boundary - start[axis]) / delta;
            if t > UV_EPSILON && t < 1.0 - UV_EPSILON {
                cuts.push(t);
            }
            boundary += 1.0;
            crossings += 1;
        }
    }
    cuts.sort_by(f32::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() <= UV_EPSILON);

    let mut segments = Vec::with_capacity(cuts.len().saturating_sub(1));
    for pair in cuts.windows(2) {
        let t0 = pair[0];
        let t1 = pair[1];
        let midpoint = lerp_uv(start, end, (t0 + t1) * 0.5);
        let tile = [midpoint[0].floor(), midpoint[1].floor()];
        let a = lerp_uv(start, end, t0);
        let b = lerp_uv(start, end, t1);
        let local_a = [clamp_uv(a[0] - tile[0]), clamp_uv(a[1] - tile[1])];
        let local_b = [clamp_uv(b[0] - tile[0]), clamp_uv(b[1] - tile[1])];
        if (local_a[0] - local_b[0]).abs() > UV_EPSILON
            || (local_a[1] - local_b[1]).abs() > UV_EPSILON
        {
            segments.push([local_a, local_b]);
        }
    }
    segments
}

fn lerp_uv(start: [f32; 2], end: [f32; 2], t: f32) -> [f32; 2] {
    [
        start[0] + (end[0] - start[0]) * t,
        start[1] + (end[1] - start[1]) * t,
    ]
}

fn clamp_uv(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

/// Return triangles belonging to the selected diffuse texture.
pub fn uv_triangles_for_texture(scene: &Scene, texture_name: &str) -> Vec<UvTriangle> {
    let wanted = texture_key(texture_name);
    let mut triangles = Vec::new();
    for mesh in &scene.meshes {
        if mesh
            .texture_name
            .as_deref()
            .map(texture_key)
            .is_none_or(|name| name != wanted)
        {
            continue;
        }
        append_mesh_uvs(mesh, &mut triangles);
    }
    triangles
}

fn append_mesh_uvs(mesh: &SceneMesh, triangles: &mut Vec<UvTriangle>) {
    for indices in mesh.indices.chunks_exact(3) {
        let Some(a) = mesh.vertices.get(indices[0] as usize) else {
            continue;
        };
        let Some(b) = mesh.vertices.get(indices[1] as usize) else {
            continue;
        };
        let Some(c) = mesh.vertices.get(indices[2] as usize) else {
            continue;
        };
        let triangle = [a.uv, b.uv, c.uv];
        if triangle
            .iter()
            .flatten()
            .all(|coordinate| coordinate.is_finite())
        {
            triangles.push(triangle);
        }
    }
}

/// Convert the textures already resolved by the NIF scene loader into the
/// same cache record used by TXD and NFT previews. This makes a rendered NIF
/// immediately available in the Texture tab without decoding the same pixels
/// a second time.
pub fn decoded_textures_from_scene(scene: &Scene) -> Vec<DecodedTexture> {
    let mut seen = HashSet::new();
    let mut textures = Vec::new();
    for mesh in &scene.meshes {
        let Some(texture_name) = mesh.texture_name.as_deref() else {
            continue;
        };
        let Some(texture) = mesh.diffuse.as_ref() else {
            continue;
        };
        let key = texture_key(texture_name);
        if !seen.insert(key.clone()) {
            continue;
        }
        let name = Path::new(texture_name)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or(&key)
            .to_string();
        let has_alpha = texture.rgba.chunks_exact(4).any(|pixel| pixel[3] < 255);
        textures.push(DecodedTexture {
            name,
            width: texture.width,
            height: texture.height,
            rgba: texture.rgba.clone(),
            has_alpha,
            format_name: "Model companion texture".to_string(),
            mipmap_count: 1,
            handle: std::sync::OnceLock::new(),
        });
    }
    textures.sort_by_key(|texture| texture.name.to_ascii_lowercase());
    textures
}

fn texture_key(name: &str) -> String {
    name.rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspector::scene3d::camera::BaseOrientation;
    use crate::inspector::scene3d::mesh::{Aabb, SceneTexture, Vertex};

    #[test]
    fn contain_rect_preserves_aspect_and_centers() {
        let rect = contain_rect(100, 50, Size::new(300.0, 300.0));
        assert_eq!(rect.size(), Size::new(300.0, 150.0));
        assert_eq!(rect.position(), Point::new(0.0, 75.0));
    }

    #[test]
    fn viewport_transform_is_shared_by_image_grid_and_uv_layers() {
        let rect = texture_image_rect(
            100,
            100,
            Size::new(400.0, 200.0),
            2.0,
            Vector::new(25.0, -10.0),
        );
        assert_eq!(rect.size(), Size::new(400.0, 400.0));
        assert_eq!(rect.position(), Point::new(-25.0, -90.0));
        assert_eq!(uv_to_point(rect, [0.0, 0.0]), rect.position());
        assert_eq!(uv_to_point(rect, [1.0, 1.0]), Point::new(375.0, 310.0));
    }

    #[test]
    fn viewport_state_resets_when_texture_changes() {
        let first = image::Handle::from_rgba(1, 1, vec![255, 0, 0, 255]);
        let second = image::Handle::from_rgba(1, 1, vec![0, 255, 0, 255]);
        let mut state = TextureViewportState::default();

        state.reset_for(&first);
        state.scale = 3.0;
        state.current_offset = Vector::new(12.0, -8.0);
        state.reset_for(&first);
        assert_eq!(state.scale, 3.0);
        assert_eq!(state.current_offset, Vector::new(12.0, -8.0));

        state.reset_for(&second);
        assert_eq!(state.scale, 1.0);
        assert_eq!(state.current_offset, Vector::default());
        assert!(state.cursor_grabbed_at.is_none());
    }

    #[test]
    fn viewport_zoom_and_pan_capture_events() {
        let handle = image::Handle::from_rgba(2, 2, vec![255; 16]);
        let viewport = TextureViewport {
            handle,
            image_width: 400,
            image_height: 200,
            render_image: true,
            show_grid: true,
            grid_divisions: 16,
            show_uv: true,
            uv_triangles: Vec::new(),
        };
        let bounds = Rectangle::new(Point::new(10.0, 20.0), Size::new(400.0, 200.0));
        let cursor = mouse::Cursor::Available(Point::new(110.0, 70.0));
        let wheel = canvas::Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
        });
        let mut state = TextureViewportState::default();

        assert!(
            <TextureViewport as canvas::Program<()>>::update(
                &viewport, &mut state, &wheel, bounds, cursor,
            )
            .is_some()
        );
        assert!(state.scale > 1.0);

        let press = canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        <TextureViewport as canvas::Program<()>>::update(
            &viewport, &mut state, &press, bounds, cursor,
        );
        let moved = canvas::Event::Mouse(mouse::Event::CursorMoved {
            position: Point::new(130.0, 100.0),
        });
        <TextureViewport as canvas::Program<()>>::update(
            &viewport,
            &mut state,
            &moved,
            bounds,
            mouse::Cursor::Available(Point::new(130.0, 100.0)),
        );
        assert_ne!(state.current_offset, Vector::default());
    }

    #[test]
    fn overlay_viewport_updates_without_capturing_navigation_events() {
        let viewport = TextureViewport {
            handle: image::Handle::from_rgba(2, 2, vec![255; 16]),
            image_width: 400,
            image_height: 200,
            render_image: false,
            show_grid: true,
            grid_divisions: 16,
            show_uv: true,
            uv_triangles: Vec::new(),
        };
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(400.0, 200.0));
        let cursor = mouse::Cursor::Available(Point::new(100.0, 100.0));
        let mut state = TextureViewportState::default();
        let wheel = canvas::Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
        });

        let action = <TextureViewport as canvas::Program<()>>::update(
            &viewport, &mut state, &wheel, bounds, cursor,
        )
        .expect("overlay should request a redraw");
        let (_, _, status) = action.into_inner();
        assert_eq!(status, iced::event::Status::Ignored);

        let press = canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        assert!(
            <TextureViewport as canvas::Program<()>>::update(
                &viewport, &mut state, &press, bounds, cursor,
            )
            .is_none()
        );
        assert!(state.cursor_grabbed_at.is_some());

        let moved = canvas::Event::Mouse(mouse::Event::CursorMoved {
            position: Point::new(120.0, 130.0),
        });
        let action = <TextureViewport as canvas::Program<()>>::update(
            &viewport,
            &mut state,
            &moved,
            bounds,
            mouse::Cursor::Available(Point::new(120.0, 130.0)),
        )
        .expect("overlay should request a redraw while panning");
        let (_, _, status) = action.into_inner();
        assert_eq!(status, iced::event::Status::Ignored);
        assert_ne!(state.current_offset, Vector::default());

        let release = canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        assert!(
            <TextureViewport as canvas::Program<()>>::update(
                &viewport, &mut state, &release, bounds, cursor,
            )
            .is_none()
        );
        assert!(state.cursor_grabbed_at.is_none());
    }

    #[test]
    fn uv_triangles_match_texture_basename_case_insensitively() {
        let scene = Scene {
            meshes: vec![SceneMesh {
                name: "mesh".into(),
                texture_name: Some("Z:\\textures\\Brick_D.TGA".into()),
                vertices: vec![
                    Vertex {
                        position: [0.0; 3],
                        normal: [0.0; 3],
                        uv: [0.0, 0.0],
                    },
                    Vertex {
                        position: [0.0; 3],
                        normal: [0.0; 3],
                        uv: [1.0, 0.0],
                    },
                    Vertex {
                        position: [0.0; 3],
                        normal: [0.0; 3],
                        uv: [0.0, 1.0],
                    },
                ],
                indices: vec![0, 1, 2],
                diffuse: Some(SceneTexture {
                    width: 2,
                    height: 2,
                    rgba: vec![255; 16],
                }),
                aabb: Aabb::default(),
            }],
            ..Scene::empty(BaseOrientation::Yup)
        };
        assert_eq!(uv_triangles_for_texture(&scene, "brick_d.tga").len(), 1);
        assert!(uv_triangles_for_texture(&scene, "other.tga").is_empty());
    }

    #[test]
    fn repeated_truck_barr_uvs_stay_inside_the_preview_tile_when_present() {
        let path = "C:/Dev/bully-nif-tools/Nif_Files/3_06TruckBarr.nif";
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(_) => return,
        };
        let scene = crate::inspector::scene3d::decode::parse_and_build_scene(
            &bytes,
            BaseOrientation::Zup,
            |_| None,
        )
        .expect("truck barr should decode");
        let triangles = uv_triangles_for_texture(&scene, "Traindoor_d.tga");
        assert_eq!(triangles.len(), 4);
        assert!(
            triangles
                .iter()
                .flatten()
                .any(|uv| uv[0] < 0.0 || uv[0] > 1.0 || uv[1] < 0.0 || uv[1] > 1.0)
        );

        let segments = triangles
            .iter()
            .flat_map(|triangle| {
                [(0, 1), (1, 2), (2, 0)]
                    .into_iter()
                    .flat_map(move |(a, b)| wrapped_uv_edge(triangle[a], triangle[b]))
            })
            .collect::<Vec<_>>();
        assert!(!segments.is_empty());
        assert!(
            segments
                .iter()
                .flatten()
                .flatten()
                .all(|value| { *value >= -UV_EPSILON && *value <= 1.0 + UV_EPSILON })
        );
    }

    #[test]
    fn uv_edges_crossing_repeat_boundaries_are_split() {
        let segments = wrapped_uv_edge([-0.25, 0.25], [1.25, 0.75]);
        assert!(segments.len() >= 2);
        assert!(
            segments
                .iter()
                .flatten()
                .flatten()
                .all(|value| { *value >= -UV_EPSILON && *value <= 1.0 + UV_EPSILON })
        );
        assert_eq!(wrapped_uv_edge([0.1, 0.2], [0.8, 0.9]).len(), 1);
    }

    #[test]
    fn grid_line_offsets_exclude_edges_and_mark_majors() {
        let lines = grid_line_offsets(160.0, 8);
        assert_eq!(lines.len(), 7);
        assert!((lines[0].0 - 20.0).abs() < 1e-4);
        // Every 4th interior line is major (with GRID_MAJOR_EVERY = 4).
        assert!(lines.iter().filter(|(_, major)| *major).count() == 1);
        assert!(!lines.iter().any(|(offset, _)| *offset >= 160.0));
    }

    #[test]
    fn grid_line_offsets_handle_degenerate_input() {
        assert!(grid_line_offsets(0.0, 8).is_empty());
        assert!(grid_line_offsets(-5.0, 8).is_empty());
        // divisions clamped to at least 1 → no interior lines.
        assert!(grid_line_offsets(100.0, 0).is_empty());
    }
}
