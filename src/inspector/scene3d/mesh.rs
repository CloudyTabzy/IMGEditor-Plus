//! CPU-side mesh representation for the embedded 3D viewer.
//!
//! - [`Vertex`] is a 32-byte interleaved record (position + normal + uv)
//!   marked `bytemuck::Pod` so Phase 17.2 can cast it into a `&[u8]` for a
//!   wgpu vertex buffer without an intermediate copy.
//! - [`SceneMesh`] holds vertex/index data for one `NiTriShape`-shaped
//!   block, plus an optional diffuse texture.
//! - [`Aabb`] is the axis-aligned bounding box, used by the camera to
//!   frame the scene and by the renderer to skip empty draws.

use bytemuck::{Pod, Zeroable};

/// Number of bytes per vertex. Position (12) + normal (12) + uv (8).
pub const VERTEX_STRIDE: usize = 32;

/// Interleaved vertex format used by every mesh in the viewer.
///
/// `#[repr(C)]` plus `Pod` lets us pass `bytemuck::cast_slice(&vertices)`
/// directly to `wgpu::Device::create_buffer_init`. The WGSL shader
/// matches this layout with `@location(0..2)` attributes.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

// SAFETY: Vertex is `#[repr(C)]` with only `Pod`-compatible fields
// ([f32; 3] and [f32; 2] are both plain old data). There is no implicit
// padding; the struct is exactly 8 floats long (32 bytes).
unsafe impl Pod for Vertex {}
unsafe impl Zeroable for Vertex {}

/// Axis-aligned bounding box. The render pipeline uses `extent` to size
/// the orbit camera's initial distance and the bounding-sphere radius to
/// cull meshes that are entirely outside the frustum.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Aabb {
    /// Build an AABB from a non-empty iterator of points. Returns
    /// `None` only if the iterator is empty.
    pub fn from_points(points: &[[f32; 3]]) -> Option<Self> {
        let (first, rest) = points.split_first()?;
        let mut min = *first;
        let mut max = *first;
        for p in rest {
            for axis in 0..3 {
                min[axis] = min[axis].min(p[axis]);
                max[axis] = max[axis].max(p[axis]);
            }
        }
        Some(Self { min, max })
    }

    /// Combine two AABBs by taking the tightest enclosing box. An empty
    /// input (default) yields the other input unchanged.
    pub fn merged(self, other: Self) -> Self {
        let min = [
            self.min[0].min(other.min[0]),
            self.min[1].min(other.min[1]),
            self.min[2].min(other.min[2]),
        ];
        let max = [
            self.max[0].max(other.max[0]),
            self.max[1].max(other.max[1]),
            self.max[2].max(other.max[2]),
        ];
        Self { min, max }
    }

    pub fn center(&self) -> [f32; 3] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }

    pub fn extent(&self) -> [f32; 3] {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }

    /// Smallest sphere that contains the AABB. Used by the orbit camera
    /// to set the default distance.
    pub fn bounding_radius(&self) -> f32 {
        let e = self.extent();
        let half = [e[0] * 0.5, e[1] * 0.5, e[2] * 0.5];
        (half[0] * half[0] + half[1] * half[1] + half[2] * half[2]).sqrt()
    }
}

/// RGBA8 diffuse texture. Width × height × 4 bytes; rows are top-to-bottom
/// in pixel order (the layout the existing `texture.rs` produces after the
/// 18-byte TGA header is stripped).
#[derive(Clone, Debug, PartialEq)]
pub struct SceneTexture {
    pub width: u32,
    pub height: u32,
    /// Row-major RGBA8 bytes, top-to-bottom, length == width*height*4.
    pub rgba: Vec<u8>,
}

impl SceneTexture {
    /// Interpret a TGA blob written by `inspector::texture::dxt{1,5}_to_tga`.
    /// Returns `None` if the header is malformed or the pixel format is
    /// not 32-bit uncompressed RGBA.
    pub fn from_tga(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 18 {
            return None;
        }
        let image_type = bytes[2];
        // 2 = uncompressed true-color; 10 = RLE true-color. We accept
        // both; the existing texture.rs pipeline emits type 2 only.
        if image_type != 2 && image_type != 10 {
            return None;
        }
        let width = u16::from_le_bytes([bytes[12], bytes[13]]) as u32;
        let height = u16::from_le_bytes([bytes[14], bytes[15]]) as u32;
        let bpp = bytes[16];
        if bpp != 32 || width == 0 || height == 0 {
            return None;
        }
        let id_length = bytes[0] as usize;
        let colormap_type = bytes[1];
        if colormap_type != 0 {
            // No palette handling — would have to read colormap spec.
            return None;
        }
        let pixel_offset = 18 + id_length;
        let expected = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        if bytes.len() < pixel_offset + expected {
            return None;
        }
        // The descriptor byte says where the origin is; we always
        // render with top-to-bottom, so the caller is expected to
        // flip if the source had bottom-to-bottom origin. The
        // existing TGA writer uses 0x20 (top-left).
        let descriptor = bytes[17];
        let flipped = (descriptor & 0x10) == 0;
        let raw = &bytes[pixel_offset..pixel_offset + expected];
        let rgba = if flipped {
            flip_vertically(raw, width as usize, height as usize, 4)
        } else {
            raw.to_vec()
        };
        Some(Self {
            width,
            height,
            rgba,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0 || self.rgba.is_empty()
    }
}

fn flip_vertically(src: &[u8], width: usize, height: usize, stride: usize) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    let row = width * stride;
    for y in 0..height {
        let src_row = &src[y * row..(y + 1) * row];
        let dst_row = &mut out[(height - 1 - y) * row..(height - y) * row];
        dst_row.copy_from_slice(src_row);
    }
    out
}

/// Geometry for one mesh inside a `Scene`. The renderer treats
/// `diffuse.is_some()` as the textured path; an untextured fallback
/// passes `(1.0, 1.0, 1.0, 1.0)` as a default colour in the shader.
#[derive(Clone, Debug)]
pub struct SceneMesh {
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub diffuse: Option<SceneTexture>,
    pub aabb: Aabb,
}

impl SceneMesh {
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn has_texture(&self) -> bool {
        self.diffuse
            .as_ref()
            .map(|t| !t.is_empty())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_is_32_bytes() {
        assert_eq!(std::mem::size_of::<Vertex>(), VERTEX_STRIDE);
    }

    #[test]
    fn vertex_pod_round_trip() {
        let v = Vertex {
            position: [1.0, 2.0, 3.0],
            normal: [0.0, 1.0, 0.0],
            uv: [0.25, 0.5],
        };
        let bytes: Vec<u8> = bytemuck::bytes_of(&v).to_vec();
        assert_eq!(bytes.len(), VERTEX_STRIDE);
        let back: &Vertex = bytemuck::from_bytes(&bytes);
        assert_eq!(*back, v);
    }

    #[test]
    fn aabb_from_points_basic() {
        let pts = [[-1.0, -2.0, -3.0], [4.0, 5.0, 6.0], [0.0, 1.0, -1.0]];
        let a = Aabb::from_points(&pts).unwrap();
        assert_eq!(a.min, [-1.0, -2.0, -3.0]);
        assert_eq!(a.max, [4.0, 5.0, 6.0]);
        assert_eq!(a.center(), [1.5, 1.5, 1.5]);
        assert_eq!(a.extent(), [5.0, 7.0, 9.0]);
    }

    #[test]
    fn aabb_from_empty_is_none() {
        let empty: [[f32; 3]; 0] = [];
        assert!(Aabb::from_points(&empty).is_none());
    }

    #[test]
    fn aabb_merged_takes_envelope() {
        let a = Aabb {
            min: [0.0, 0.0, 0.0],
            max: [1.0, 2.0, 3.0],
        };
        let b = Aabb {
            min: [-1.0, 3.0, 1.0],
            max: [2.0, 5.0, 4.0],
        };
        let m = a.merged(b);
        assert_eq!(m.min, [-1.0, 0.0, 0.0]);
        assert_eq!(m.max, [2.0, 5.0, 4.0]);
    }

    #[test]
    fn aabb_bounding_radius() {
        let a = Aabb {
            min: [-2.0, -2.0, -2.0],
            max: [2.0, 2.0, 2.0],
        };
        let r = a.bounding_radius();
        let expected = (3.0f32 * 2.0 * 2.0).sqrt();
        assert!((r - expected).abs() < 1e-5);
    }

    #[test]
    fn texture_from_tga_uncompressed_rgba() {
        // Build a 2x2 TGA with all-white RGBA pixels, top-left origin.
        let mut tga = vec![0u8; 18 + 16];
        tga[2] = 2;
        tga[12..14].copy_from_slice(&2u16.to_le_bytes());
        tga[14..16].copy_from_slice(&2u16.to_le_bytes());
        tga[16] = 32;
        tga[17] = 0x20; // top-left origin
        for i in 18..34 {
            tga[i] = 0xFF;
        }
        let tex = SceneTexture::from_tga(&tga).unwrap();
        assert_eq!(tex.width, 2);
        assert_eq!(tex.height, 2);
        assert_eq!(tex.rgba.len(), 16);
        assert!(tex.rgba.iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn texture_from_tga_rejects_non_rgba() {
        let mut tga = vec![0u8; 18 + 16];
        tga[2] = 2;
        tga[16] = 24; // 24-bit, not 32
        assert!(SceneTexture::from_tga(&tga).is_none());
    }

    #[test]
    fn texture_from_tga_flips_when_origin_bottom() {
        // 2x2 image, bottom-left origin. Source stores the BOTTOM row
        // first (pixel row y=0) then the top row (y=1). The four
        // pixels are written in scanline order: (0,0), (1,0), (0,1),
        // (1,1). After flipping (top row first), the output reads
        // (0,1), (1,1), (0,0), (1,0).
        let mut tga = vec![0u8; 18 + 4 * 4];
        tga[2] = 2;
        tga[12..14].copy_from_slice(&2u16.to_le_bytes());
        tga[14..16].copy_from_slice(&2u16.to_le_bytes());
        tga[16] = 32;
        tga[17] = 0x00; // bottom-left origin → flip
        // Bottom row first (y=0): RED, GREEN.
        tga[18..22].copy_from_slice(&[0xFF, 0x00, 0x00, 0xFF]);
        tga[22..26].copy_from_slice(&[0x00, 0xFF, 0x00, 0xFF]);
        // Top row (y=1): BLUE, WHITE.
        tga[26..30].copy_from_slice(&[0x00, 0x00, 0xFF, 0xFF]);
        tga[30..34].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
        let tex = SceneTexture::from_tga(&tga).unwrap();
        // After flip: top row (y=1) first.
        assert_eq!(&tex.rgba[0..4], &[0x00, 0x00, 0xFF, 0xFF]); // BLUE
        assert_eq!(&tex.rgba[4..8], &[0xFF, 0xFF, 0xFF, 0xFF]); // WHITE
        assert_eq!(&tex.rgba[8..12], &[0xFF, 0x00, 0x00, 0xFF]); // RED
        assert_eq!(&tex.rgba[12..16], &[0x00, 0xFF, 0x00, 0xFF]); // GREEN
    }
}
