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
use crate::inspector::scene3d::pipeline::{self, GpuMesh, GpuTexture, RenderFlags, ScenePipelines};
use crate::inspector::scene3d::scene::Scene;

/// Headless wgpu context: instance, adapter, device, queue, and the
/// scene pipelines for one output format.
///
/// Constructing an instance initializes the platform graphics drivers;
/// creating several renderers concurrently can race the driver loaders
/// (seen as STATUS_ACCESS_VIOLATION on Windows Vulkan). Construct on a
/// single thread, or share one renderer behind a lock — the test module
/// below does the latter.
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
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("imgeditor-scene3d-headless"),
            // The wire overlay is built from explicit line-list indices, so
            // the headless renderer does not need the optional native-only
            // POLYGON_MODE_LINE feature.
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| format!("device request failed: {e}"))?;
        // 1x sample count keeps the headless output byte-deterministic
        // for the pixel-diff tests; the embedded viewer renders at
        // SCENE_MSAA_SAMPLES instead.
        let pipelines = ScenePipelines::new(&device, &queue, color_format, 1);
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
    frustum_cam.base_orientation = scene.base_orientation;

    renderer
        .pipelines
        .update_camera(queue, &frustum_cam, scene.key_light, scene.ambient, flags);

    let mut mesh_gpus = Vec::with_capacity(scene.meshes.len());
    for mesh in &scene.meshes {
        let gpu = GpuMesh::from_scene_mesh(device, queue, mesh);
        let tex = mesh.diffuse.as_ref().map(|t| {
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
                    // Only the color attachment is read back; depth is
                    // never sampled after the pass.
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if flags.contains(RenderFlags::SHOW_GRID) {
            pass.set_pipeline(&pipelines.grid);
            pass.set_bind_group(0, &pipelines.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, pipelines.quad_vertex_buffer.slice(..));
            pass.set_index_buffer(
                pipelines.quad_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            pass.draw_indexed(0..6, 0, 0..1);
        }

        let lit_pipeline = if flags.contains(RenderFlags::ALPHA_BLEND) {
            if flags.contains(RenderFlags::CULL_BACK) {
                &pipelines.lit_cull_back_alpha
            } else {
                &pipelines.lit_alpha
            }
        } else if flags.contains(RenderFlags::CULL_BACK) {
            &pipelines.lit_cull_back
        } else {
            &pipelines.lit
        };
        pass.set_pipeline(lit_pipeline);
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

        if flags.contains(RenderFlags::WIREFRAME) {
            pass.set_pipeline(&pipelines.wireframe);
            pass.set_bind_group(0, &pipelines.camera_bind_group, &[]);
            for (gpu_mesh, tex) in &mesh_gpus {
                let bg: &wgpu::BindGroup = match tex {
                    Some(t) => &t.bind_group,
                    None => &pipelines.default_diffuse.bind_group,
                };
                pass.set_bind_group(1, bg, &[]);
                pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(
                    gpu_mesh.wire_index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..gpu_mesh.wire_index_count, 0, 0..1);
            }
        }

        if flags.contains(RenderFlags::SHOW_NAVIGATION) {
            pass.set_pipeline(&pipelines.gizmo);
            pass.set_bind_group(0, &pipelines.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, pipelines.quad_vertex_buffer.slice(..));
            pass.set_index_buffer(
                pipelines.quad_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            pass.draw_indexed(0..6, 0, 0..1);
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

pub fn write_png<P: AsRef<Path>>(frame: &RenderedFrame, path: P) -> Result<(), String> {
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
    use std::sync::{Mutex, MutexGuard, OnceLock};

    /// Shared GPU context guard for headless tests.
    ///
    /// Each `HeadlessRenderer::new()` creates a `wgpu::Instance`, and
    /// concurrent instance creation races the platform driver loaders —
    /// on Windows this intermittently aborts the whole test process
    /// with STATUS_ACCESS_VIOLATION. All tests therefore render against
    /// one lazily-created shared renderer. The guard's mutex also
    /// serializes the GPU work itself: `render_frame` stores the camera
    /// uniform in the shared `ScenePipelines` buffer, so two concurrent
    /// renders would overwrite each other's camera mid-frame.
    struct Gpu<'a> {
        renderer: &'a HeadlessRenderer,
        _lock: MutexGuard<'a, ()>,
    }

    impl std::ops::Deref for Gpu<'_> {
        type Target = HeadlessRenderer;

        fn deref(&self) -> &HeadlessRenderer {
            self.renderer
        }
    }

    fn gpu() -> Result<Gpu<'static>, String> {
        static RENDERER: OnceLock<Result<HeadlessRenderer, String>> = OnceLock::new();
        static LOCK: Mutex<()> = Mutex::new(());
        let renderer = RENDERER
            .get_or_init(HeadlessRenderer::new)
            .as_ref()
            .map_err(|e| e.clone())?;
        Ok(Gpu {
            renderer,
            _lock: LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
        })
    }

    fn triangle_scene() -> Scene {
        let positions = [[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]];
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
            texture_name: None,
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
        let renderer = gpu().expect("renderer");
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
        let renderer = gpu().expect("renderer");
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
    fn msaa_scene_pass_renders_and_resolves() {
        // The embedded viewer renders its scene pass at SCENE_MSAA_SAMPLES
        // and resolves into a 1x color texture. This exercises that exact
        // wiring headlessly: 4x pipelines + 4x attachments + resolve. Any
        // sample-count, format, or usage mismatch raises a wgpu validation
        // error and fails the test.
        let renderer = gpu().expect("renderer");
        let device = &renderer.device;
        let queue = &renderer.queue;

        let pipelines = pipeline::ScenePipelines::new(
            device,
            queue,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            pipeline::SCENE_MSAA_SAMPLES,
        );
        let scene = triangle_scene();
        let mut camera = OrbitCamera::new(Viewport {
            width: 64,
            height: 64,
        });
        camera.reset_to_aabb(&scene.aabb);
        pipelines.update_camera(queue, &camera, scene.key_light, scene.ambient, RenderFlags::empty());

        let width = 64u32;
        let height = 64u32;
        let (_msaa_tex, msaa_view) = pipeline::create_msaa_color_texture(device, width, height);
        let (_depth_tex, depth_view) =
            pipeline::create_depth_texture(device, width, height, pipeline::SCENE_MSAA_SAMPLES);
        // 1x resolve target with the same usage flags as the widget's
        // scene color texture, plus COPY_SRC for the readback below.
        let resolve = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("imgeditor-scene3d-msaa-test/resolve"),
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
        let resolve_view = resolve.create_view(&wgpu::TextureViewDescriptor::default());

        let mesh_gpu = GpuMesh::from_scene_mesh(device, queue, &scene.meshes[0]);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("imgeditor-scene3d-msaa-test/encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("imgeditor-scene3d-msaa-test/pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &msaa_view,
                    depth_slice: None,
                    resolve_target: Some(&resolve_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.06,
                            g: 0.07,
                            b: 0.09,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&pipelines.lit);
            pass.set_bind_group(0, &pipelines.camera_bind_group, &[]);
            pass.set_bind_group(1, &pipelines.default_diffuse.bind_group, &[]);
            pass.set_vertex_buffer(0, mesh_gpu.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh_gpu.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh_gpu.index_count, 0, 0..1);
        }
        // 64 * 4 bytes per row already satisfies the copy row alignment.
        let read_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("imgeditor-scene3d-msaa-test/read"),
            size: (width * height * 4) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &resolve,
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
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: Some(submit_info),
            timeout: None,
        });
        let mapped = slice.get_mapped_range();
        // The triangle covers a solid chunk of the frame; count pixels
        // that differ from the pure-background corner pixel.
        let bg = &mapped[0..4];
        let lit_pixels = mapped
            .chunks_exact(4)
            .filter(|px| *px != bg)
            .count();
        drop(mapped);
        read_buf.unmap();
        assert!(
            lit_pixels > (width * height) as usize / 10,
            "resolved image should show the triangle (lit pixels: {lit_pixels})"
        );
    }

    #[test]
    fn grid_visibility_flag_changes_rendered_frame() {
        let renderer = gpu().expect("renderer");
        let scene = triangle_scene();
        let mut camera = OrbitCamera::new(Viewport {
            width: 128,
            height: 128,
        });
        camera.reset_to_aabb(&scene.aabb);

        let with_grid = render_frame(&renderer, &scene, &camera, 128, 128, RenderFlags::SHOW_GRID)
            .expect("grid frame");
        let without_grid = render_frame(&renderer, &scene, &camera, 128, 128, RenderFlags::empty())
            .expect("plain frame");

        assert_ne!(with_grid.rgba, without_grid.rgba);
    }

    #[test]
    fn navigation_visibility_flag_changes_rendered_frame() {
        let renderer = gpu().expect("renderer");
        let scene = triangle_scene();
        let mut camera = OrbitCamera::new(Viewport {
            width: 256,
            height: 256,
        });
        camera.reset_to_aabb(&scene.aabb);

        let with_navigation = render_frame(
            &renderer,
            &scene,
            &camera,
            256,
            256,
            RenderFlags::SHOW_NAVIGATION,
        )
        .expect("navigation frame");
        let without_navigation =
            render_frame(&renderer, &scene, &camera, 256, 256, RenderFlags::empty())
                .expect("plain frame");

        assert_ne!(with_navigation.rgba, without_navigation.rgba);
    }

    #[test]
    fn render_wireframe_flag_changes_pipeline() {
        let renderer = gpu().expect("renderer");
        let scene = triangle_scene();
        let mut camera = OrbitCamera::new(Viewport {
            width: 128,
            height: 128,
        });
        camera.reset_to_aabb(&scene.aabb);
        let frame_lit =
            render_frame(&renderer, &scene, &camera, 128, 128, RenderFlags::empty()).expect("lit");
        let frame_wire = render_frame(&renderer, &scene, &camera, 128, 128, RenderFlags::WIREFRAME)
            .expect("wireframe");
        assert_eq!(frame_lit.rgba.len(), frame_wire.rgba.len());
        // Lit and wireframe should differ somewhere.
        assert_ne!(frame_lit.rgba, frame_wire.rgba);
    }

    #[test]
    fn render_textured_mesh_uses_path() {
        let renderer = gpu().expect("renderer");
        let mut scene = triangle_scene();
        // 2x2 solid red texture
        scene.meshes[0].diffuse = Some(crate::inspector::scene3d::mesh::SceneTexture {
            width: 2,
            height: 2,
            rgba: vec![
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
            ],
        });
        let mut camera = OrbitCamera::new(Viewport {
            width: 64,
            height: 64,
        });
        camera.reset_to_aabb(&scene.aabb);
        let _ = render_frame(&renderer, &scene, &camera, 64, 64, RenderFlags::HAS_TEXTURE)
            .expect("textured render");
    }

    #[test]
    fn alpha_blend_flag_reveals_background_through_transparent_texels() {
        let renderer = gpu().expect("renderer");
        let mut scene = triangle_scene();
        // A fully transparent texel models the cutout pixels used by common
        // GTA foliage, fencing, and outline textures.
        scene.meshes[0].diffuse = Some(crate::inspector::scene3d::mesh::SceneTexture {
            width: 1,
            height: 1,
            rgba: vec![255, 0, 0, 0],
        });
        let mut camera = OrbitCamera::new(Viewport {
            width: 128,
            height: 128,
        });
        camera.reset_to_aabb(&scene.aabb);

        let opaque = render_frame(
            &renderer,
            &scene,
            &camera,
            128,
            128,
            RenderFlags::HAS_TEXTURE,
        )
        .expect("opaque textured render");
        let alpha = render_frame(
            &renderer,
            &scene,
            &camera,
            128,
            128,
            RenderFlags::HAS_TEXTURE | RenderFlags::ALPHA_BLEND,
        )
        .expect("alpha textured render");

        let mut background_scene = scene.clone();
        background_scene.meshes.clear();
        let background = render_frame(
            &renderer,
            &background_scene,
            &camera,
            128,
            128,
            RenderFlags::empty(),
        )
        .expect("background including navigation overlay");
        let opaque_model_pixels = opaque
            .rgba
            .chunks_exact(4)
            .zip(background.rgba.chunks_exact(4))
            .filter(|(pixel, background)| pixel != background)
            .count();
        assert!(
            opaque_model_pixels > 100,
            "opaque mode should draw the triangle"
        );
        assert_eq!(
            alpha.rgba, background.rgba,
            "fully transparent texels must reveal the unchanged background"
        );
    }

    #[test]
    fn floor_renders_dimmed_from_below() {
        // The orbit allows the eye below the floor plane (bottom
        // view). The ray-cast floor must still render from
        // underneath — dimmed toward the background — instead of
        // leaving a black void. The headless target is Rgba8UnormSrgb:
        // the dimmed minor/major grid lines store as ~(101, 106, 115)
        // and ~(128, 134, 143).
        let renderer = gpu().expect("renderer");
        let scene = triangle_scene();
        let mut cam = OrbitCamera::new(Viewport {
            width: 256,
            height: 256,
        });
        cam.reset_to_aabb(&scene.aabb);
        cam.pitch = -1.2; // eye well below the plane, looking up
        assert!(cam.eye()[1] < 0.0);
        let f =
            render_frame(&renderer, &scene, &cam, 256, 256, RenderFlags::SHOW_GRID).expect("frame");
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
    fn floor_does_not_occlude_geometry_below_world_plane() {
        let renderer = gpu().expect("renderer");
        let positions = [[-0.8, -2.0, 0.0], [0.8, -2.0, 0.0], [0.0, -0.5, 0.0]];
        let aabb = Aabb::from_points(&positions).unwrap();
        let vertices = positions
            .into_iter()
            .map(|position| Vertex {
                position,
                normal: [0.0, 0.0, 1.0],
                uv: [0.5, 0.5],
            })
            .collect();
        let mesh = SceneMesh {
            name: "below-floor".into(),
            texture_name: Some("below-floor.tga".into()),
            vertices,
            indices: vec![0, 1, 2],
            diffuse: Some(crate::inspector::scene3d::mesh::SceneTexture {
                width: 2,
                height: 2,
                rgba: vec![
                    255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
                ],
            }),
            aabb,
        };
        let scene = Scene {
            meshes: vec![mesh],
            aabb,
            ambient: [0.2, 0.2, 0.22],
            key_light: [0.45, 0.75, 0.45],
            base_orientation: crate::inspector::scene3d::camera::BaseOrientation::Yup,
        };
        let mut camera = OrbitCamera::new(Viewport {
            width: 256,
            height: 256,
        });
        camera.target = [0.0, -1.25, 0.0];
        camera.distance = 8.0;
        camera.pitch = 0.3;
        assert!(camera.eye()[1] > 0.0, "camera should look down through Y=0");
        let frame = render_frame(
            &renderer,
            &scene,
            &camera,
            256,
            256,
            RenderFlags::HAS_TEXTURE | RenderFlags::SHOW_GRID,
        )
        .expect("frame");
        let red_pixels = frame
            .rgba
            .chunks_exact(4)
            .filter(|pixel| pixel[0] > 150 && pixel[1] < 80 && pixel[2] < 80)
            .count();
        assert!(
            red_pixels > 100,
            "geometry below the reference floor should remain visible (found {red_pixels} red pixels)"
        );
    }

    #[test]
    fn zero_viewport_is_rejected() {
        let renderer = gpu().expect("renderer");
        let scene = triangle_scene();
        let camera = OrbitCamera::new(Viewport {
            width: 0,
            height: 0,
        });
        assert!(render_frame(&renderer, &scene, &camera, 0, 0, RenderFlags::empty()).is_err());
    }

    #[test]
    fn gizmo_axes_follow_camera_orbit() {
        // Sample only the opaque navigation disc, excluding scene pixels.
        let renderer = gpu().expect("renderer");
        let scene = triangle_scene();
        let mut cam_a = OrbitCamera::new(Viewport {
            width: 256,
            height: 256,
        });
        cam_a.reset_to_aabb(&scene.aabb);
        let mut cam_b = cam_a.clone();
        cam_b.yaw += std::f32::consts::FRAC_PI_2;
        let fa = render_frame(
            &renderer,
            &scene,
            &cam_a,
            256,
            256,
            RenderFlags::SHOW_NAVIGATION,
        )
        .expect("frame a");
        let fb = render_frame(
            &renderer,
            &scene,
            &cam_b,
            256,
            256,
            RenderFlags::SHOW_NAVIGATION,
        )
        .expect("frame b");
        let nav = crate::inspector::scene3d::navigation::NavigationUniform::new(
            &cam_a, 256.0, 256.0, 1.0,
        );
        let [cx, cy, scale, _] = nav.layout;
        let mut box_pixels = Vec::new();
        for row in 0..256u32 {
            for col in 0..256u32 {
                if (col as f32 - cx).hypot(row as f32 - cy) < 57.0 * scale {
                    box_pixels.push(((row * 256 + col) * 4) as usize);
                }
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
    fn navigation_renders_at_hit_targets_in_all_axis_views() {
        use crate::inspector::scene3d::navigation::{AxisView, NavigationUniform};
        let renderer = gpu().expect("renderer");
        let mut scene = triangle_scene();
        scene.base_orientation = crate::inspector::scene3d::camera::BaseOrientation::Zup;
        for axis in AxisView::ALL {
            let mut camera = OrbitCamera::new(Viewport {
                width: 640,
                height: 480,
            });
            camera.base_orientation = scene.base_orientation;
            camera.reset_to_aabb(&scene.aabb);
            camera.snap_to_axis(axis);
            let frame = render_frame(
                &renderer,
                &scene,
                &camera,
                640,
                480,
                RenderFlags::SHOW_GRID | RenderFlags::SHOW_NAVIGATION,
            )
            .expect("orthographic GPU render");
            let nav = NavigationUniform::new(&camera, 640.0, 480.0, 1.0);
            // The axis facing the camera is the last disc drawn and the
            // first hit target. Its highlight ring must land at that point.
            let tip = nav.tips.last().unwrap();
            let mut bright = 0;
            for y in tip[1] as u32 - 12..=tip[1] as u32 + 12 {
                for x in tip[0] as u32 - 12..=tip[0] as u32 + 12 {
                    let pixel = &frame.rgba[((y * 640 + x) * 4) as usize..][..3];
                    if pixel.iter().all(|v| *v > 210) {
                        bright += 1;
                    }
                }
            }
            assert!(
                bright > 25,
                "selected {axis:?} highlight did not render at its hit target"
            );
            write_png(&frame, format!("target/navigation-{axis:?}.png"))
                .expect("navigation snapshot");
        }
        let mut camera = OrbitCamera::new(Viewport {
            width: 640,
            height: 480,
        });
        camera.reset_to_aabb(&scene.aabb);
        camera.yaw = 0.65;
        let plain = render_frame(
            &renderer,
            &scene,
            &camera,
            640,
            480,
            RenderFlags::SHOW_GRID | RenderFlags::SHOW_NAVIGATION,
        )
        .unwrap();
        camera.navigation_hover = 7;
        let hover = render_frame(
            &renderer,
            &scene,
            &camera,
            640,
            480,
            RenderFlags::SHOW_GRID | RenderFlags::SHOW_NAVIGATION,
        )
        .unwrap();
        assert_ne!(
            plain.rgba, hover.rgba,
            "projection button hover must be visible"
        );
        write_png(&plain, "target/navigation-perspective.png").unwrap();
    }

    #[test]
    fn navigation_fixture_views_when_present() {
        use crate::inspector::scene3d::camera::BaseOrientation;
        use crate::inspector::scene3d::navigation::AxisView;
        let fixtures = [
            (
                "gta-tank",
                "C:/Dev/IMGEditor-master/Gta_3_img/Exported/ci_watertank.dff",
            ),
            (
                "bully-lamp",
                "C:/Games/Bully - Scholarship Edition/Stream/NIF/adm_lamp.nif",
            ),
        ];
        for (name, path) in fixtures {
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            let mut scene = if path.ends_with(".dff") {
                crate::inspector::scene3d::decode::parse_and_build_scene_from_dff(
                    &bytes,
                    BaseOrientation::Zup,
                    |_| None,
                )
            } else {
                crate::inspector::scene3d::decode::parse_and_build_scene(
                    &bytes,
                    BaseOrientation::Zup,
                    |_| None,
                )
            }
            .expect("fixture scene");
            // Match the GUI's centered mode without changing the source file.
            let center = glam::Vec3::from(scene.aabb.center());
            for mesh in &mut scene.meshes {
                for vertex in &mut mesh.vertices {
                    vertex.position = (glam::Vec3::from(vertex.position) - center).into();
                }
            }
            scene.aabb.min = (glam::Vec3::from(scene.aabb.min) - center).into();
            scene.aabb.max = (glam::Vec3::from(scene.aabb.max) - center).into();
            let renderer = gpu().expect("renderer");
            for (label, axis) in [
                ("perspective", None),
                ("top", Some(AxisView::PositiveZ)),
                ("front", Some(AxisView::NegativeY)),
            ] {
                let mut camera = OrbitCamera::new(Viewport {
                    width: 800,
                    height: 600,
                });
                camera.base_orientation = BaseOrientation::Zup;
                camera.reset_to_aabb(&scene.aabb);
                camera.yaw = 0.65;
                camera.pitch = 0.45;
                if let Some(axis) = axis {
                    camera.snap_to_axis(axis);
                }
                let frame = render_frame(
                    &renderer,
                    &scene,
                    &camera,
                    800,
                    600,
                    RenderFlags::SHOW_GRID | RenderFlags::SHOW_NAVIGATION,
                )
                .expect("fixture navigation render");
                write_png(&frame, format!("target/navigation-{name}-{label}.png")).unwrap();
            }
        }
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
        let renderer = gpu().expect("renderer");
        let frame = render_frame(&renderer, &scene, &camera, 512, 512, RenderFlags::empty())
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
    fn adm_lamp_fixture_renders_when_present() {
        let candidates = [
            std::path::Path::new("C:/Dev/bully-nif-tools/Nif_Files/adm_lamp.nif"),
            std::path::Path::new("C:/Games/Bully - Scholarship Edition/Stream/NIF/adm_lamp.nif"),
        ];
        let Some(path) = candidates.iter().find(|path| path.is_file()) else {
            return;
        };
        let bytes = std::fs::read(path).expect("adm_lamp fixture should be readable");
        let scene = crate::inspector::scene3d::decode::parse_and_build_scene(
            &bytes,
            crate::inspector::scene3d::camera::BaseOrientation::Zup,
            |_| None,
        )
        .expect("adm_lamp should decode");
        assert!(scene.has_geometry());
        assert_eq!(scene.total_vertices(), 292);
        assert_eq!(scene.total_triangles(), 168);

        let mut camera = OrbitCamera::new(Viewport {
            width: 512,
            height: 512,
        });
        camera.reset_to_aabb(&scene.aabb);
        let renderer = gpu().expect("renderer");
        let frame = render_frame(&renderer, &scene, &camera, 512, 512, RenderFlags::empty())
            .expect("adm_lamp frame rendered");
        let out = std::path::Path::new("target").join("scene3d-adm-lamp.png");
        if let Some(parent) = out.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        write_png(&frame, &out).expect("adm_lamp PNG written");
        assert!(
            std::fs::metadata(&out).expect("adm_lamp PNG exists").len() > 200,
            "adm_lamp PNG suspiciously small: {}",
            out.display()
        );
    }

    #[test]
    fn mascot_fixtures_render_to_png_when_present() {
        let root = std::path::Path::new("C:/Games/Bully - Scholarship Edition/Stream/NIF");
        let archive_path =
            std::path::Path::new("C:/Games/Bully - Scholarship Edition/Stream/World.img");
        let names = [
            "Player_Mascot.nif",
            "Player_Mascot_nh.nif",
            "Player_Mascot_W.nif",
        ];
        let existing = names.iter().filter(|name| root.join(name).exists()).count();
        if existing == 0 || !archive_path.is_file() {
            return;
        }

        let archive = crate::archive::ArchiveInfo::open(archive_path).expect("World.img");
        let archive_index = crate::inspector::texture::ArchiveTextureIndex::from_entries(
            &archive.entries,
            archive.path.as_deref(),
        );
        let renderer = gpu().expect("renderer");
        for name in names {
            let Ok(bytes) = std::fs::read(root.join(name)) else {
                continue;
            };
            let nif_basename = std::path::Path::new(name)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("mascot");
            let catalog = archive_index
                .resolve_textures_for_nif(nif_basename, None)
                .expect("mascot NFT should be found in World.img");
            let scene = crate::inspector::scene3d::decode::parse_and_build_scene(
                &bytes,
                crate::inspector::scene3d::camera::BaseOrientation::Zup,
                |texture_name| {
                    catalog
                        .get_pixels(texture_name)
                        .and_then(crate::inspector::scene3d::mesh::SceneTexture::from_tga)
                        .or_else(|| {
                            archive_index.read(texture_name).and_then(|bytes| {
                                crate::inspector::scene3d::mesh::SceneTexture::from_tga(&bytes)
                            })
                        })
                },
            )
            .expect("mascot scene decoded");
            assert!(
                scene.textured_mesh_count() > 0,
                "{name} should resolve at least one diffuse texture"
            );
            let mut camera = OrbitCamera::new(Viewport {
                width: 512,
                height: 512,
            });
            camera.reset_to_aabb(&scene.aabb);
            let frame = render_frame(
                &renderer,
                &scene,
                &camera,
                512,
                512,
                RenderFlags::HAS_TEXTURE,
            )
            .expect("mascot frame rendered");
            let stem = std::path::Path::new(name)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("mascot");
            let out = std::path::Path::new("target").join(format!("scene3d-{stem}.png"));
            if let Some(parent) = out.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            write_png(&frame, &out).expect("mascot PNG written");
            assert!(
                std::fs::metadata(&out).expect("mascot PNG exists").len() > 200,
                "mascot PNG suspiciously small: {}",
                out.display()
            );
        }
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
        let Ok(bytes) = std::fs::read(path) else {
            return;
        };
        let scene = crate::inspector::scene3d::decode::parse_and_build_scene(
            &bytes,
            crate::inspector::scene3d::camera::BaseOrientation::Zup,
            |_| None,
        )
        .expect("scene decoded");

        let width = 800u32;
        let height = 600u32;
        let renderer = gpu().expect("renderer");
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

        let mut camera =
            OrbitCamera::new(crate::inspector::scene3d::camera::Viewport { width, height });
        camera.reset_to_aabb(&scene.aabb);
        pipelines.update_camera(
            queue,
            &camera,
            scene.key_light,
            scene.ambient,
            RenderFlags::SHOW_GRID,
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
                        store: wgpu::StoreOp::Discard,
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
        assert!(
            meta.len() > 200,
            "PNG suspiciously small: {} bytes",
            meta.len()
        );
    }
}
