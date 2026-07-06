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
    pub pipelines: std::sync::Arc<ScenePipelines>,
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
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("imgeditor-scene3d-headless"),
                required_features: wgpu::Features::POLYGON_MODE_LINE,
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
        size: (width as u64) * (height as u64) * 4,
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
            .map(|t| GpuTexture::from_scene_texture(device, queue, t, &pipelines.texture_layout));
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

        pass.set_pipeline(if flags.contains(RenderFlags::WIREFRAME) {
            &pipelines.wireframe
        } else {
            &pipelines.lit
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
                bytes_per_row: Some(width * 4),
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
    let rgba: Vec<u8> = mapped.to_vec();
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
        let out = std::path::Path::new("target")
            .join("scene3d-bully-1950fridge.png");
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
}
