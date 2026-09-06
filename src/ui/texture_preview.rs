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
            let points = triangle.map(|uv| uv_to_point(image_rect, uv));
            for edge in [(0, 1), (1, 2), (2, 0)] {
                frame.stroke(&canvas::Path::line(points[edge.0], points[edge.1]), stroke);
            }
        }

        vec![frame.into_geometry()]
    }
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
}
