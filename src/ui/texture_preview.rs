//! Shared helpers for texture-tab previews and optional NIF UV overlays.

use std::collections::HashSet;
use std::path::Path;

use iced::widget::canvas;
use iced::{Color, Point, Rectangle, Size, Theme, mouse};

use crate::inspector::scene3d::mesh::SceneMesh;
use crate::inspector::scene3d::scene::Scene;
use crate::parser::DecodedTexture;

pub type UvTriangle = [[f32; 2]; 3];

/// A fit-to-preview UV overlay. It intentionally has no interaction state:
/// the image and the overlay use the same contain rectangle, so the mapping
/// stays aligned instead of drifting with a separate zoom/pan state.
#[derive(Debug, Clone)]
pub struct TextureUvOverlay {
    pub image_width: u32,
    pub image_height: u32,
    pub triangles: Vec<UvTriangle>,
}

impl<Message> canvas::Program<Message> for TextureUvOverlay {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let image_rect = contain_rect(self.image_width, self.image_height, bounds.size());

        frame.stroke_rectangle(
            Point::new(image_rect.x, image_rect.y),
            image_rect.size(),
            canvas::Stroke::default()
                .with_color(Color::from_rgba(0.35, 0.9, 0.95, 0.7))
                .with_width(1.0),
        );

        let stroke = canvas::Stroke::default()
            .with_color(Color::from_rgba(1.0, 0.84, 0.22, 0.95))
            .with_width(1.2)
            .with_line_join(canvas::LineJoin::Round)
            .with_line_cap(canvas::LineCap::Round);
        for triangle in &self.triangles {
            for edge in [(0, 1), (1, 2), (2, 0)] {
                for segment in wrapped_uv_edge(triangle[edge.0], triangle[edge.1]) {
                    let points = segment.map(|uv| uv_to_point(image_rect, uv));
                    frame.stroke(&canvas::Path::line(points[0], points[1]), stroke);
                }
            }
        }

        vec![frame.into_geometry()]
    }
}

/// Photoshop-style view decorations over the texture preview: a
/// proportional grid and pixel rulers drawn on the same `Contain`
/// rectangle the image is displayed in, so lines always line up with
/// texel positions regardless of the preview pane size.
///
/// The whole texture is always visible in the Contain fit (no
/// zoom/pan), which lets the rulers map texture pixels to screen
/// positions with a single linear scale.
#[derive(Debug, Clone)]
pub struct TextureViewOverlay {
    pub image_width: u32,
    pub image_height: u32,
    pub show_grid: bool,
    pub show_rulers: bool,
    /// Number of grid cells per axis. Only honored when `show_grid`.
    pub grid_divisions: u32,
}

/// Screen size of the ruler strips anchored to the viewport's top and
/// left edges.
const RULER_SIZE: f32 = 18.0;
/// Every Nth internal grid line renders as a major line.
const GRID_MAJOR_EVERY: u32 = 4;
/// Screen distance (px) major ruler ticks should try to keep apart.
const RULER_TARGET_SPACING: f32 = 56.0;

// The grid is drawn in two passes — a dark shadow stroke under a
// bright stroke — because textures come in every color and a single
// grid color is invisible on half of them.
const GRID_MINOR_DARK: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.25);
const GRID_MINOR_LIGHT: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.28);
const GRID_MAJOR_DARK: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.45);
const GRID_MAJOR_LIGHT: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.45);

const RULER_BG: Color = Color::from_rgba(0.07, 0.07, 0.09, 0.82);
const RULER_TICK: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.85);
const RULER_MINOR_TICK: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.4);
const RULER_TEXT: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.95);
const CURSOR_GUIDE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.18);

impl<Message: 'static> canvas::Program<Message> for TextureViewOverlay {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let image_rect = contain_rect(self.image_width, self.image_height, bounds.size());
        if image_rect.width <= 0.0 || image_rect.height <= 0.0 {
            return vec![frame.into_geometry()];
        }

        if self.show_grid {
            draw_grid(&mut frame, image_rect, self.grid_divisions);
        }
        if self.show_rulers {
            draw_rulers(&mut frame, self, image_rect, bounds);
        }
        draw_cursor_readout(
            &mut frame,
            self.image_width,
            self.image_height,
            image_rect,
            bounds,
            cursor,
        );

        vec![frame.into_geometry()]
    }
}

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

fn draw_rulers(
    frame: &mut canvas::Frame,
    overlay: &TextureViewOverlay,
    rect: Rectangle,
    bounds: Rectangle,
) {
    // Strip backgrounds anchored to the viewport top-left, Photoshop
    // style. They overlap the image edges only when the Contain margins
    // are thinner than the strips.
    frame.fill_rectangle(
        Point::new(bounds.x, bounds.y),
        Size::new(bounds.width, RULER_SIZE),
        RULER_BG,
    );
    frame.fill_rectangle(
        Point::new(bounds.x, bounds.y),
        Size::new(RULER_SIZE, bounds.height),
        RULER_BG,
    );

    if overlay.image_width == 0 || overlay.image_height == 0 {
        return;
    }
    let scale_x = rect.width / overlay.image_width as f32;
    let scale_y = rect.height / overlay.image_height as f32;
    let step = nice_step(RULER_TARGET_SPACING, scale_x.min(scale_y));
    let major_every = if step >= 5.0 { 5u32 } else { 2u32 };
    let minor = step / major_every as f32;

    // Horizontal (top) ruler: minor + major ticks, majors labeled with
    // the texture-pixel coordinate. The whole image is always visible
    // in the Contain fit, so the tick range is simply 0..width.
    let minor_count = (overlay.image_width as f32 / minor).floor() as u32;
    for i in 0..=minor_count {
        let v = i as f32 * minor;
        let x = rect.x + v * scale_x;
        let is_major = i % major_every == 0;
        if x >= bounds.x + RULER_SIZE {
            let (top, color) = if is_major {
                (bounds.y, RULER_TICK)
            } else {
                (bounds.y + RULER_SIZE * 0.55, RULER_MINOR_TICK)
            };
            frame.stroke(
                &canvas::Path::line(Point::new(x, top), Point::new(x, bounds.y + RULER_SIZE)),
                canvas::Stroke::default().with_color(color).with_width(1.0),
            );
            if is_major && v > 0.0 {
                frame.fill_text(canvas::Text {
                    content: format!("{}", v as u32),
                    position: Point::new(x, bounds.y + 2.5),
                    color: RULER_TEXT,
                    size: 9.0.into(),
                    align_x: iced::widget::text::Alignment::Center,
                    ..Default::default()
                });
            }
        }
    }

    // Vertical (left) ruler: ticks only — canvas text can't be
    // rotated, and horizontal labels would overflow the 18 px strip.
    let minor_count = (overlay.image_height as f32 / minor).floor() as u32;
    for i in 0..=minor_count {
        let v = i as f32 * minor;
        let y = rect.y + v * scale_y;
        let is_major = i % major_every == 0;
        if y >= bounds.y + RULER_SIZE {
            let (left, color) = if is_major {
                (bounds.x, RULER_TICK)
            } else {
                (bounds.x + RULER_SIZE * 0.55, RULER_MINOR_TICK)
            };
            frame.stroke(
                &canvas::Path::line(Point::new(left, y), Point::new(bounds.x + RULER_SIZE, y)),
                canvas::Stroke::default().with_color(color).with_width(1.0),
            );
        }
    }
}

/// Nice tick step (1, 2, 5 × 10ⁿ texture pixels) so consecutive major
/// ticks are at least `min_screen_px` apart at the given doc→screen
/// scale.
fn nice_step(min_screen_px: f32, scale: f32) -> f32 {
    let min_doc = (min_screen_px / scale.max(f32::EPSILON)).max(1.0);
    let mut base = 1.0f32;
    loop {
        for m in [1.0, 2.0, 5.0] {
            if base * m >= min_doc {
                return base * m;
            }
        }
        base *= 10.0;
    }
}

fn draw_cursor_readout(
    frame: &mut canvas::Frame,
    image_width: u32,
    image_height: u32,
    rect: Rectangle,
    bounds: Rectangle,
    cursor: mouse::Cursor,
) {
    let Some(pos) = cursor.position_over(bounds) else {
        return;
    };

    // Subtle full-length guides make it easy to read the exact
    // position against both rulers.
    frame.stroke(
        &canvas::Path::line(
            Point::new(pos.x, bounds.y),
            Point::new(pos.x, bounds.y + bounds.height),
        ),
        canvas::Stroke::default()
            .with_color(CURSOR_GUIDE)
            .with_width(1.0),
    );
    frame.stroke(
        &canvas::Path::line(
            Point::new(bounds.x, pos.y),
            Point::new(bounds.x + bounds.width, pos.y),
        ),
        canvas::Stroke::default()
            .with_color(CURSOR_GUIDE)
            .with_width(1.0),
    );

    // Coordinates in texture pixels. Values outside the image (the
    // Contain margins) are shown as-is, Photoshop-style.
    let doc_x = if rect.width > 0.0 {
        (pos.x - rect.x) / rect.width * image_width as f32
    } else {
        0.0
    };
    let doc_y = if rect.height > 0.0 {
        (pos.y - rect.y) / rect.height * image_height as f32
    } else {
        0.0
    };
    let label = format!("{:.0}, {:.0}", doc_x, doc_y);

    // Pill background so the readout stays readable over any texture.
    let text_width = label.len() as f32 * 5.6 + 10.0;
    let text_height = 14.0;
    let mut x = pos.x + 14.0;
    let mut y = pos.y + 14.0;
    if x + text_width > bounds.x + bounds.width {
        x = pos.x - 14.0 - text_width;
    }
    if y + text_height > bounds.y + bounds.height {
        y = pos.y - 14.0 - text_height;
    }
    frame.fill_rectangle(
        Point::new(x, y),
        Size::new(text_width, text_height),
        RULER_BG,
    );
    frame.fill_text(canvas::Text {
        content: label,
        position: Point::new(x + text_width / 2.0, y + 1.5),
        color: RULER_TEXT,
        size: 10.0.into(),
        align_x: iced::widget::text::Alignment::Center,
        ..Default::default()
    });
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
            format_name: "NIF companion texture".to_string(),
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
    fn nice_step_is_always_at_least_the_requested_spacing() {
        let scale = 0.25; // 1 screen px = 4 doc px
        let step = nice_step(56.0, scale);
        assert!(step * scale >= 56.0);
        // 1-2-5 progression: the previous ladder value (step / 2.5)
        // must not fit.
        assert!((step / 2.5) * scale < 56.0);
        // min_doc = 224 → 2 × 10² = 200 is too small, so 5 × 10².
        assert_eq!(step, 500.0);
    }

    #[test]
    fn nice_step_never_goes_below_one() {
        // Extremely fine scale should clamp at 1 doc px, not divide forever.
        assert_eq!(nice_step(56.0, 1000.0), 1.0);
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
