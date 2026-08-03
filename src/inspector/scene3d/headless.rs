//! Headless GPU renderer used by tests and ad-hoc screenshots.
//!
//! Spawns its own `wgpu::Instance`, runs against a software fallback
//! adapter by default (so it works on machines without a real GPU
//! and in CI), and renders one frame of a [`Scene`] to an offscreen
//! RGBA8 texture. Output bytes are written as either raw `rgba.bin`
//! files or PNG via the `image` crate.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::inspector::scene3d::camera::{OrbitCamera, Viewport};
use crate::inspector::scene3d::pipeline::{
    self, GpuMesh, GpuTexture, RenderFlags, ScenePipelines,
};
use crate::inspector::scene3d::scene::Scene;

pub struct HeadlessRenderer {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub pipelines: ScenePipelines,
    pub color_format: wgpu::TextureFormat,
}

impl HeadlessRenderer {
    pub fn new() -> Result<Self, String> {
        Self::with_format(wgpu::TextureFormat::Rgba8UnormSrgb)
    }

    pub fn with_format(color_format: wgpu::TextureFormat) -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: true,
            ..Default::default()
        }))
        .map_err(|e| format!("adapter request failed: {e}"))?;
        let required_features = if adapter
            .features()
            .contains(wgpu::Features::POLYGON_MODE_LINE)
        {
            wgpu::Features::POLYGON_MODE_LINE
        } else {
            wgpu::Features::empty()
        };
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("imgeditor-scene3d-headless"),
                required_features,
                required_limits: wgpu::Limits::downlevel_defaults(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            },
        ))
        .map_err(|e| format!("device request failed: {e}"))?;
        let pipelines = ScenePipelines::new(&device, &queue, color_format);
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            pipelines,
            color_format,
        })
    }

    pub fn renderer_info(&self) -> String {
        let info = self.adapter.get_info();
        format!("{} ({:?})", info.name, info.backend)
    }
}

/// One prepared frame: the offscreen color view + depth view + a CPU
/// `Vec<u8>` of the readback pixels after submit.
pub struct RenderedFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub submit_info: wgpu::SubmissionIndex,
}

    pub fn render_frame(
        renderer: &HeadlessRenderer,
        scene: &Scene,
        camera: &OrbitCamera,
        width: u32,
        height: u32,
        flags: RenderFlags,
    ) -> Result<RenderedFrame, String> {
        let device = &renderer.device;
        let queue = &renderer.queue;
        let pipelines = &renderer.pipelines;
        let color_format = renderer.color_format;

        if width == 0 || height == 0 {
            return Err("zero-area render".into());
        }
        pipeline::validate_scene_for_device(device, scene, width, height)?;
        let unpadded_bytes_per_row = width
            .checked_mul(4)
            .ok_or_else(|| "readback row size overflowed".to_string())?;
        let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row
            .checked_add(alignment - 1)
            .ok_or_else(|| "aligned readback row size overflowed".to_string())?
            / alignment
            * alignment;
        let readback_size = (padded_bytes_per_row as u64)
            .checked_mul(height as u64)
            .ok_or_else(|| "readback buffer size overflowed".to_string())?;

        let color_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("imgeditor-scene3d-headless/color"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color_view = color_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let (_depth_tex, depth_view) = pipeline::create_depth_texture(device, width, height, 1);

        let read_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("imgeditor-scene3d-headless/readback"),
            size: readback_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut frustum_cam = camera.clone();
        frustum_cam.set_viewport(Viewport { width, height });

        renderer.pipelines.update_camera(
            queue,
            &frustum_cam,
            scene.key_light,
            scene.ambient,
            flags,
        );

        let mut mesh_gpus = Vec::with_capacity(scene.meshes.len());
        for mesh in &scene.meshes {
            let gpu = GpuMesh::from_scene_mesh(device, queue, mesh);
            let tex = mesh
                .diffuse
                .as_ref()
                .map(|t| {
                    GpuTexture::from_scene_texture(
                        device,
                        queue,
                        t,
                        &pipelines.texture_layout,
                        &pipelines.texture_sampler,
                    )
                });
            mesh_gpus.push((gpu, tex));
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("imgeditor-scene3d-headless/encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("imgeditor-scene3d-headless/pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.06,
                            g: 0.07,
                            b: 0.09,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&pipelines.grid);
            pass.set_bind_group(0, &pipelines.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, pipelines.quad_vertex_buffer.slice(..));
            pass.set_index_buffer(
                pipelines.quad_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            pass.draw_indexed(0..6, 0, 0..1);

            pass.set_pipeline(match (
                flags.contains(RenderFlags::WIREFRAME),
                pipelines.wireframe.as_ref(),
            ) {
                (true, Some(wf)) => wf,
                _ => &pipelines.lit,
            });
            pass.set_bind_group(0, &pipelines.camera_bind_group, &[]);

            for (gpu_mesh, tex) in &mesh_gpus {
                let bg: &wgpu::BindGroup = match tex {
                    Some(t) => &t.bind_group,
                    None => &pipelines.default_diffuse.bind_group,
                };
                pass.set_bind_group(1, bg, &[]);
                pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..gpu_mesh.index_count, 0, 0..1);
            }

            pass.set_pipeline(&pipelines.gizmo);
            pass.set_bind_group(0, &pipelines.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, pipelines.quad_vertex_buffer.slice(..));
            pass.set_index_buffer(
                pipelines.quad_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            pass.draw_indexed(0..6, 0, 0..1);
        }

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &color_tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &read_buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    let submit_info = queue.submit(std::iter::once(encoder.finish()));
    let slice = read_buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    let poll_index = submit_info.clone();
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: Some(poll_index),
        timeout: None,
    });

    let mapped = slice.get_mapped_range();
    let row_size = unpadded_bytes_per_row as usize;
    let padded_row_size = padded_bytes_per_row as usize;
    let mut rgba = Vec::with_capacity(row_size * height as usize);
    for row in mapped.chunks(padded_row_size).take(height as usize) {
        rgba.extend_from_slice(&row[..row_size]);
    }
    drop(mapped);
    read_buf.unmap();

    Ok(RenderedFrame {
        width,
        height,
        rgba,
        submit_info,
    })
}

pub fn write_png<P: AsRef<Path>>(
    frame: &RenderedFrame,
    path: P,
) -> Result<(), String> {
    let bytes = frame.rgba.as_slice();
    let img = image::RgbaImage::from_raw(frame.width, frame.height, bytes.to_vec())
        .ok_or_else(|| "rgba buffer did not match dimensions".to_string())?;
    img.save(path.as_ref())
        .map_err(|e| format!("png write failed: {e}"))
}

pub fn write_raw<P: AsRef<Path>>(frame: &RenderedFrame, path: P) -> std::io::Result<()> {
    let mut f = File::create(path)?;
    f.write_all(&frame.rgba)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspector::scene3d::camera::{OrbitCamera, Viewport};
    use crate::inspector::scene3d::mesh::{Aabb, SceneMesh, Vertex};
    use crate::inspector::scene3d::scene::Scene;

    fn triangle_scene() -> Scene {
        let positions = [
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let normals = [[0.0, 0.0, 1.0]; 3];
        let uvs = [[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]];
        let vertices: Vec<Vertex> = positions
            .iter()
            .zip(normals.iter())
            .zip(uvs.iter())
            .map(|((p, n), uv)| Vertex {
                position: *p,
                normal: *n,
                uv: *uv,
            })
            .collect();
        let indices = vec![0u32, 1, 2];
        let aabb = Aabb::from_points(&positions).unwrap();
        let mesh = SceneMesh {
            name: "tri".into(),
            vertices,
            indices,
            diffuse: None,
            aabb,
        };
        Scene {
            meshes: vec![mesh],
            aabb,
            ambient: [0.2, 0.2, 0.22],
            key_light: [0.45, 0.75, 0.45],
            base_orientation: crate::inspector::scene3d::camera::BaseOrientation::Yup,
        }
    }

    #[test]
    fn render_triangle_to_png() {
        let renderer = HeadlessRenderer::new().expect("renderer");
        let scene = triangle_scene();
        let mut camera = OrbitCamera::new(Viewport {
            width: 256,
            height: 256,
        });
        camera.reset_to_aabb(&scene.aabb);
        let frame = render_frame(&renderer, &scene, &camera, 256, 256, RenderFlags::empty())
            .expect("render");
        assert_eq!(frame.rgba.len(), 256 * 256 * 4);
        // PNG smoke test: write to temp dir, check non-empty.
        let tmp = std::env::temp_dir().join("imgeditor-scene3d-test-triangle.png");
        write_png(&frame, &tmp).expect("png");
        let meta = std::fs::metadata(&tmp).unwrap();
        assert!(meta.len() > 100);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn readback_strips_alignment_padding() {
        let renderer = HeadlessRenderer::new().expect("renderer");
        let scene = triangle_scene();
        let camera = OrbitCamera::new(Viewport {
            width: 17,
            height: 9,
        });
        let frame = render_frame(&renderer, &scene, &camera, 17, 9, RenderFlags::empty())
            .expect("unaligned render");
        assert_eq!(frame.rgba.len(), 17 * 9 * 4);
    }

    #[test]
    fn render_wireframe_flag_changes_pipeline() {
        let renderer = HeadlessRenderer::new().expect("renderer");
        let scene = triangle_scene();
        let mut camera = OrbitCamera::new(Viewport {
            width: 128,
            height: 128,
        });
        camera.reset_to_aabb(&scene.aabb);
        let frame_lit = render_frame(
            &renderer,
            &scene,
            &camera,
            128,
            128,
            RenderFlags::empty(),
        )
        .expect("lit");
        let frame_wire = render_frame(
            &renderer,
            &scene,
            &camera,
            128,
            128,
            RenderFlags::WIREFRAME,
        )
        .expect("wireframe");
        assert_eq!(frame_lit.rgba.len(), frame_wire.rgba.len());
        // Lit and wireframe should differ somewhere.
        assert_ne!(frame_lit.rgba, frame_wire.rgba);
    }

    #[test]
    fn render_textured_mesh_uses_path() {
        let renderer = HeadlessRenderer::new().expect("renderer");
        let mut scene = triangle_scene();
        // 2x2 solid red texture
        scene.meshes[0].diffuse = Some(crate::inspector::scene3d::mesh::SceneTexture {
            width: 2,
            height: 2,
            rgba: vec![255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255],
        });
        let mut camera = OrbitCamera::new(Viewport {
            width: 64,
            height: 64,
        });
        camera.reset_to_aabb(&scene.aabb);
        let _ = render_frame(
            &renderer,
            &scene,
            &camera,
            64,
            64,
            RenderFlags::HAS_TEXTURE,
        )
        .expect("textured render");
    }

    #[test]
    fn floor_renders_dimmed_from_below() {
        // The orbit allows the eye below the floor plane (bottom
        // view). The ray-cast floor must still render from
        // underneath — dimmed toward the background — instead of
        // leaving a black void. The headless target is Rgba8UnormSrgb:
        // the dimmed minor/major grid lines store as ~(101, 106, 115)
        // and ~(128, 134, 143).
        let renderer = HeadlessRenderer::new().expect("renderer");
        let scene = triangle_scene();
        let mut cam = OrbitCamera::new(Viewport {
            width: 256,
            height: 256,
        });
        cam.reset_to_aabb(&scene.aabb);
        cam.pitch = -1.2; // eye well below the plane, looking up
        assert!(cam.eye()[1] < 0.0);
        let f = render_frame(&renderer, &scene, &cam, 256, 256, RenderFlags::empty())
            .expect("frame");
        let matches = |i: usize, (r, g, b): (i32, i32, i32)| {
            (f.rgba[i] as i32 - r).abs() < 8
                && (f.rgba[i + 1] as i32 - g).abs() < 8
                && (f.rgba[i + 2] as i32 - b).abs() < 8
        };
        let mut grid_pixels = 0usize;
        for row in 0..128u32 {
            for col in 0..256u32 {
                let i = ((row * 256 + col) * 4) as usize;
                if matches(i, (101, 106, 115)) || matches(i, (128, 134, 143)) {
                    grid_pixels += 1;
                }
            }
        }
        assert!(
            grid_pixels > 200,
            "floor should render dimmed grid lines from below (found {grid_pixels})"
        );
    }

    #[test]
    fn zero_viewport_is_rejected() {
        let renderer = HeadlessRenderer::new().expect("renderer");
        let scene = triangle_scene();
        let camera = OrbitCamera::new(Viewport {
            width: 0,
            height: 0,
        });
        assert!(render_frame(&renderer, &scene, &camera, 0, 0, RenderFlags::empty()).is_err());
    }

    #[test]
    fn gizmo_axes_follow_camera_orbit() {
        // The gizmo box is opaque (alpha = 1 inside), so within its
        // screen region only gizmo content is visible — the grid and
        // model behind it are fully covered. Any pixel change there
        // after orbiting can only come from the axes tracking the
        // camera's view rotation.
        let renderer = HeadlessRenderer::new().expect("renderer");
        let scene = triangle_scene();
        let mut cam_a = OrbitCamera::new(Viewport {
            width: 256,
            height: 256,
        });
        cam_a.reset_to_aabb(&scene.aabb);
        let mut cam_b = cam_a.clone();
        cam_b.yaw += std::f32::consts::FRAC_PI_2;
        let fa = render_frame(&renderer, &scene, &cam_a, 256, 256, RenderFlags::empty())
            .expect("frame a");
        let fb = render_frame(&renderer, &scene, &cam_b, 256, 256, RenderFlags::empty())
            .expect("frame b");
        // Locate the gizmo box by its border ring. The headless target
        // is Rgba8UnormSrgb, so the shader's linear border (0.55, 0.58,
        // 0.62) is stored as sRGB bytes ~(196, 200, 206) — distinct
        // from anything else in the frame. Axis strokes and the box bg
        // sit inside the ring, so the box region is the bounding rect
        // of the border pixels.
        let is_border = |f: &RenderedFrame, i: usize| {
            (f.rgba[i] as i32 - 196).abs() < 8
                && (f.rgba[i + 1] as i32 - 200).abs() < 8
                && (f.rgba[i + 2] as i32 - 206).abs() < 8
        };
        let (mut r0, mut r1, mut c0, mut c1) = (u32::MAX, 0u32, u32::MAX, 0u32);
        for row in 0..256u32 {
            for col in 0..256u32 {
                let i = ((row * 256 + col) * 4) as usize;
                if is_border(&fa, i) {
                    r0 = r0.min(row);
                    r1 = r1.max(row);
                    c0 = c0.min(col);
                    c1 = c1.max(col);
                }
            }
        }
        assert!(r0 <= r1, "gizmo box border not found in frame");
        let mut box_pixels = Vec::new();
        for row in r0..=r1 {
            for col in c0..=c1 {
                box_pixels.push(((row * 256 + col) * 4) as usize);
            }
        }

        // Sanity: frame a has saturated axis-colored pixels somewhere
        // inside the box (proves the axes render at all).
        let has_axis_color = box_pixels.iter().any(|&i| {
            let (r, g, b) = (
                fa.rgba[i] as i32,
                fa.rgba[i + 1] as i32,
                fa.rgba[i + 2] as i32,
            );
            r.max(g).max(b) - r.min(g).min(b) > 60
        });
        assert!(has_axis_color, "no axis-colored pixels inside gizmo box");

        let diffs = box_pixels
            .iter()
            .filter(|&&i| {
                let da = (fa.rgba[i] as i32 - fb.rgba[i] as i32).abs()
                    + (fa.rgba[i + 1] as i32 - fb.rgba[i + 1] as i32).abs()
                    + (fa.rgba[i + 2] as i32 - fb.rgba[i + 2] as i32).abs();
                da > 30
            })
            .count();
        assert!(
            diffs > 50,
            "gizmo box should visibly change after orbiting (diffs = {diffs})"
        );
    }

    #[test]
    fn bully_fixture_renders_to_png_when_present() {
        // End-to-end smoke: parse a Bully NIF, decode the geometry,
        // upload to GPU, render one frame, write a PNG to the target
        // directory so a human can eyeball the output. Skips silently
        // when the fixture is not on the dev machine (matches the
        // existing `decoder_handles_bully_fixture_when_present` pattern).
        let path = std::path::Path::new(
            "C:/Games/Bully - Scholarship Edition/Stream/test1/1950Fridge.nif",
        );
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => return,
        };
        let scene = crate::inspector::scene3d::decode::parse_and_build_scene(
            &bytes,
            crate::inspector::scene3d::camera::BaseOrientation::Zup,
            |_| None,
        )
        .expect("scene decoded");
        assert!(scene.has_geometry());
        let mut camera = OrbitCamera::new(Viewport {
            width: 512,
            height: 512,
        });
        camera.reset_to_aabb(&scene.aabb);
        let renderer = HeadlessRenderer::new().expect("renderer");
        let frame = render_frame(
            &renderer,
            &scene,
            &camera,
            512,
            512,
            RenderFlags::empty(),
        )
        .expect("frame");
        let out = std::path::Path::new("target").join("scene3d-bully-1950fridge.png");
        if let Some(parent) = out.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        write_png(&frame, &out).expect("png write");
        let meta = std::fs::metadata(&out).expect("file exists");
        assert!(
            meta.len() > 200,
            "PNG suspiciously small: {} bytes",
            meta.len()
        );
    }

    #[test]
    fn full_pipeline_with_grid_and_gizmo() {
        // Smoke-renders the full GUI pipeline (clear + grid + lit +
        // gizmo) using a real NIF. Output to
        // target/scene3d-full-pipeline.png so the rendering can be
        // eyeballed without launching the GUI. Skips silently when
        // the Bully fixture is not on the dev machine.
        let path = std::path::Path::new(
            "C:/Games/Bully - Scholarship Edition/Stream/test1/1950Fridge.nif",
        );
        let Ok(bytes) = std::fs::read(path) else { return };
        let scene = crate::inspector::scene3d::decode::parse_and_build_scene(
            &bytes,
            crate::inspector::scene3d::camera::BaseOrientation::Zup,
            |_| None,
        )
        .expect("scene decoded");

        let width = 800u32;
        let height = 600u32;
        let renderer = HeadlessRenderer::new().expect("renderer");
        let device = &renderer.device;
        let queue = &renderer.queue;
        let pipelines = &renderer.pipelines;

        let align: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let unpadded_bpr = width * 4;
        let padded_bpr = unpadded_bpr.div_ceil(align) * align;
        let readback_size = (padded_bpr as u64) * (height as u64);

        let color_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("imgeditor-scene3d-full/color"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color_view = color_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let (_depth_tex, depth_view) =
            crate::inspector::scene3d::pipeline::create_depth_texture(device, width, height, 1);

        let read_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("imgeditor-scene3d-full/readback"),
            size: readback_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut camera = OrbitCamera::new(crate::inspector::scene3d::camera::Viewport {
            width,
            height,
        });
        camera.reset_to_aabb(&scene.aabb);
        pipelines.update_camera(
            queue,
            &camera,
            scene.key_light,
            scene.ambient,
            RenderFlags::empty(),
        );

        let mesh_gpus: Vec<_> = scene
            .meshes
            .iter()
            .map(|m| {
                let gpu = GpuMesh::from_scene_mesh(device, queue, m);
                let tex = m.diffuse.as_ref().map(|t| {
                    GpuTexture::from_scene_texture(
                        device,
                        queue,
                        t,
                        &pipelines.texture_layout,
                        &pipelines.texture_sampler,
                    )
                });
                (gpu, tex)
            })
            .collect();

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("imgeditor-scene3d-full/encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("imgeditor-scene3d-full/pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.06,
                            g: 0.07,
                            b: 0.09,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&pipelines.grid);
            pass.set_bind_group(0, &pipelines.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, pipelines.quad_vertex_buffer.slice(..));
            pass.set_index_buffer(
                pipelines.quad_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            pass.draw_indexed(0..6, 0, 0..1);

            pass.set_pipeline(&pipelines.lit);
            pass.set_bind_group(0, &pipelines.camera_bind_group, &[]);

            for (gpu_mesh, tex) in &mesh_gpus {
                let bg: &wgpu::BindGroup = match tex {
                    Some(t) => &t.bind_group,
                    None => &pipelines.default_diffuse.bind_group,
                };
                pass.set_bind_group(1, bg, &[]);
                pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(
                    gpu_mesh.index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..gpu_mesh.index_count, 0, 0..1);
            }

            pass.set_pipeline(&pipelines.gizmo);
            pass.set_bind_group(0, &pipelines.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, pipelines.quad_vertex_buffer.slice(..));
            pass.set_index_buffer(
                pipelines.quad_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            pass.draw_indexed(0..6, 0, 0..1);
        }

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &color_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &read_buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bpr),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let submit = queue.submit(std::iter::once(encoder.finish()));
        let slice = read_buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let poll_index = submit.clone();
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: Some(poll_index),
            timeout: None,
        });
        let mapped = slice.get_mapped_range();
        let raw: Vec<u8> = mapped.to_vec();
        drop(mapped);
        read_buf.unmap();
        let mut rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);
        for row in 0..height as usize {
            let start = row * padded_bpr as usize;
            rgba.extend_from_slice(&raw[start..start + unpadded_bpr as usize]);
        }
        let frame = RenderedFrame {
            width,
            height,
            rgba,
            submit_info: submit,
        };
        let out = std::path::Path::new("target").join("scene3d-full-pipeline.png");
        if let Some(parent) = out.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        write_png(&frame, &out).expect("png write");
        let meta = std::fs::metadata(&out).expect("file exists");
        assert!(meta.len() > 200, "PNG suspiciously small: {} bytes", meta.len());
    }
}
