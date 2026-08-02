//! GPU side of the embedded 3D viewer.
//!
//! Two render pipelines plus a composite step keep the in-pane model
//! from wiping the surrounding Iced UI:
//!
//! - **Lit** — vertex + fragment WGSL, full Lambert + ambient, optional
//!   diffuse texture selected at draw time by a flag in the camera UBO.
//!   Renders to a private `scene_color_target`, never to the surface.
//!   Has a real depth attachment so triangle ordering is correct.
//! - **Wireframe** — same vertex stage, line-list rasteriser, no depth.
//! - **Compositor** — a separate pipeline with no depth and a one-line
//!   fragment shader that samples the `scene_color_target`. The widget
//!   uses this in `Primitive::draw` to blit the offscreen texture into
//!   Iced's main render pass under the compositor's scissor, so the
//!   3D model stays inside its pane rectangle and the rest of the GUI
//!   remains untouched.
//!
//! One 1×1 white "default diffuse" texture is allocated so untextured
//! meshes can use the same bind-group layout as textured meshes.
//!
//! The CPU [`Scene`] and the GPU [`ScenePipeline`] are decoupled on
//! purpose: CPU data lives in the widget `State`, GPU resources are
//! owned here and rebuilt only when the scene changes or the viewport
//! resizes.

use bytemuck::{Pod, Zeroable};

use crate::inspector::scene3d::camera::OrbitCamera;
use crate::inspector::scene3d::mesh::{SceneMesh, SceneTexture, VERTEX_STRIDE};
use crate::inspector::scene3d::scene::Scene;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub inverse_view_proj: [[f32; 4]; 4],
    pub key_light: [f32; 4],
    pub ambient: [f32; 4],
    pub eye_pos: [f32; 4],
    pub flags: u32,
    pub _pad: [u32; 3],
}

impl CameraUniform {
    pub fn from_camera(camera: &OrbitCamera, key_light: [f32; 3], ambient: [f32; 3]) -> Self {
        let view = camera.view();
        let proj = camera.projection();
        let view_proj = proj * view;
        let inverse_view_proj = view_proj.inverse();
        let eye = camera.eye();
        Self {
            view_proj: view_proj.to_cols_array_2d(),
            inverse_view_proj: inverse_view_proj.to_cols_array_2d(),
            key_light: [key_light[0], key_light[1], key_light[2], 0.0],
            ambient: [ambient[0], ambient[1], ambient[2], 0.0],
            eye_pos: [eye[0], eye[1], eye[2], 0.0],
            flags: 0,
            _pad: [0; 3],
        }
    }
}

pub const LIT_WGSL: &str = include_str!("shaders/lit.wgsl");
pub const WIREFRAME_WGSL: &str = include_str!("shaders/wireframe.wgsl");
pub const COMPOSITOR_WGSL: &str = include_str!("shaders/compositor.wgsl");
pub const GRID_WGSL: &str = include_str!("shaders/grid.wgsl");
pub const GIZMO_WGSL: &str = include_str!("shaders/gizmo.wgsl");

pub fn lit_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("imgeditor-scene3d/lit"),
        source: wgpu::ShaderSource::Wgsl(LIT_WGSL.into()),
    })
}

pub fn wireframe_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("imgeditor-scene3d/wireframe"),
        source: wgpu::ShaderSource::Wgsl(WIREFRAME_WGSL.into()),
    })
}

pub fn compositor_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("imgeditor-scene3d/compositor"),
        source: wgpu::ShaderSource::Wgsl(COMPOSITOR_WGSL.into()),
    })
}

pub fn grid_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("imgeditor-scene3d/grid"),
        source: wgpu::ShaderSource::Wgsl(GRID_WGSL.into()),
    })
}

pub fn gizmo_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("imgeditor-scene3d/gizmo"),
        source: wgpu::ShaderSource::Wgsl(GIZMO_WGSL.into()),
    })
}

pub fn depth_format() -> wgpu::TextureFormat {
    wgpu::TextureFormat::Depth32Float
}

pub fn scene_color_format() -> wgpu::TextureFormat {
    wgpu::TextureFormat::Rgba8UnormSrgb
}

pub fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    sample_count: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("imgeditor-scene3d/depth"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: depth_format(),
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn create_scene_color_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("imgeditor-scene3d/scene_color"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: scene_color_format(),
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl GpuMesh {
    pub fn from_scene_mesh(device: &wgpu::Device, queue: &wgpu::Queue, mesh: &SceneMesh) -> Self {
        use wgpu::util::DeviceExt;

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("imgeditor-scene3d/vertex"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("imgeditor-scene3d/index"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let _ = queue;
        Self {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
        }
    }
}

pub fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: VERTEX_STRIDE as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x2,
            },
        ],
    }
}

pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
}

impl GpuTexture {
    pub fn from_scene_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        tex: &SceneTexture,
        layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("imgeditor-scene3d/diffuse"),
            size: wgpu::Extent3d {
                width: tex.width,
                height: tex.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &tex.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(tex.width * 4),
                rows_per_image: Some(tex.height),
            },
            wgpu::Extent3d {
                width: tex.width,
                height: tex.height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                label: Some("imgeditor-scene3d/diffuse_bind_group"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&default_sampler(device)),
                    },
                ],
            },
        );
        Self {
            texture,
            view,
            bind_group,
        }
    }

    pub fn default_white(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("imgeditor-scene3d/diffuse_default"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                label: Some("imgeditor-scene3d/diffuse_default_bind_group"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&default_sampler(device)),
                    },
                ],
            },
        );
        Self {
            texture,
            view,
            bind_group,
        }
    }
}

pub fn default_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("imgeditor-scene3d/default_sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    })
}

pub struct ScenePipelines {
    pub lit: wgpu::RenderPipeline,
    pub lit_cull_back: wgpu::RenderPipeline,
    pub wireframe: Option<wgpu::RenderPipeline>,
    pub grid: wgpu::RenderPipeline,
    pub gizmo: wgpu::RenderPipeline,
    pub compositor: wgpu::RenderPipeline,
    pub camera_layout: wgpu::BindGroupLayout,
    pub texture_layout: wgpu::BindGroupLayout,
    pub compositor_layout: wgpu::BindGroupLayout,
    pub compositor_sampler: wgpu::Sampler,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub default_diffuse: GpuTexture,
    /// Offscreen render target + view + bind-group, populated lazily and
    /// recreated on viewport resize.
    pub scene_color_tex: Option<wgpu::Texture>,
    pub scene_color_view: Option<wgpu::TextureView>,
    pub compositor_bind_group: Option<wgpu::BindGroup>,
    /// Fullscreen-triangle vertex/index buffers reused every frame to
    /// blit the offscreen scene texture into the compositor's render pass.
    pub quad_vertex_buffer: wgpu::Buffer,
    pub quad_index_buffer: wgpu::Buffer,
}

impl ScenePipelines {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let lit_module = lit_shader_module(device);
        let wire_module = wireframe_shader_module(device);
        let compositor_module = compositor_shader_module(device);
        let grid_module = grid_shader_module(device);
        let gizmo_module = gizmo_shader_module(device);

        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("imgeditor-scene3d/camera_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(256),
                },
                count: None,
            }],
        });

        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("imgeditor-scene3d/texture_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("imgeditor-scene3d/pipeline_layout"),
            bind_group_layouts: &[&camera_layout, &texture_layout],
            push_constant_ranges: &[],
        });

        let compositor_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("imgeditor-scene3d/compositor_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let compositor_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("imgeditor-scene3d/compositor_pipeline_layout"),
                bind_group_layouts: &[&compositor_layout],
                push_constant_ranges: &[],
            });

        // The lit, wireframe, grid, and gizmo pipelines all render
        // into the OFFSCREEN target (scene_color_format), not the
        // surface. The compositor pipeline is the only one that
        // targets the Iced surface directly. Passing the surface
        // format to the scene-rendering pipelines caused a Vulkan
        // validation panic when the offscreen render pass's Rgba8UnormSrgb
        // attachment didn't match the pipeline's Bgra8Unorm expectations.
        let lit = build_lit_pipeline(
            device,
            &lit_module,
            &pipeline_layout,
            scene_color_format(),
            wgpu::PolygonMode::Fill,
            None,
            "imgeditor-scene3d/lit_pipeline",
        );
        let lit_cull_back = build_lit_pipeline(
            device,
            &lit_module,
            &pipeline_layout,
            scene_color_format(),
            wgpu::PolygonMode::Fill,
            Some(wgpu::Face::Back),
            "imgeditor-scene3d/lit_cull_back_pipeline",
        );

        let wireframe = if device.features().contains(wgpu::Features::POLYGON_MODE_LINE) {
            Some(build_lit_pipeline(
                device,
                &wire_module,
                &pipeline_layout,
                scene_color_format(),
                wgpu::PolygonMode::Line,
                None,
                "imgeditor-scene3d/wireframe_pipeline",
            ))
        } else {
            None
        };

        let compositor = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("imgeditor-scene3d/compositor_pipeline"),
            layout: Some(&compositor_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &compositor_module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[vertex_buffer_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &compositor_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("imgeditor-scene3d/camera_ubo"),
            size: 256,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("imgeditor-scene3d/camera_bind_group"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let default_diffuse = GpuTexture::default_white(device, queue, &texture_layout);

        let compositor_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("imgeditor-scene3d/compositor_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let quad_vertex_buffer = build_quad_vertex_buffer(device);
        let quad_index_buffer = build_quad_index_buffer(device);

        // The grid and gizmo shaders don't sample a texture, so they use
        // a dedicated layout with only the camera bind group (or none
        // at all). Sharing the model pipeline's 2-bind-group layout here
        // would trip Vulkan's "BindGroup to be set at index 1" check
        // because the grid shader has no group(1) binding.
        let grid_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("imgeditor-scene3d/grid_layout"),
            bind_group_layouts: &[&camera_layout],
            push_constant_ranges: &[],
        });
        let gizmo_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("imgeditor-scene3d/gizmo_layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let grid = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("imgeditor-scene3d/grid_pipeline"),
            layout: Some(&grid_layout),
            vertex: wgpu::VertexState {
                module: &grid_module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[vertex_buffer_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &grid_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: scene_color_format(),
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format(),
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let gizmo = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("imgeditor-scene3d/gizmo_pipeline"),
            layout: Some(&gizmo_layout),
            vertex: wgpu::VertexState {
                module: &gizmo_module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[vertex_buffer_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &gizmo_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: scene_color_format(),
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            // The gizmo is drawn inside the offscreen render pass, which
            // carries a depth-stencil attachment (Depth32Float). Vulkan
            // requires every pipeline used in such a pass to declare a
            // matching depth_stencil state, even if the pipeline never
            // reads or writes depth. We declare Depth32Float with
            // `Always` compare + write-disabled so the gizmo draws
            // unconditionally but never disturbs the floor/model depth
            // values written earlier in the same pass.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format(),
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Self {
            lit,
            lit_cull_back,
            wireframe,
            grid,
            gizmo,
            compositor,
            camera_layout,
            texture_layout,
            compositor_layout,
            compositor_sampler,
            camera_buffer,
            camera_bind_group,
            default_diffuse,
            scene_color_tex: None,
            scene_color_view: None,
            compositor_bind_group: None,
            quad_vertex_buffer,
            quad_index_buffer,
        }
    }

    pub fn update_camera(
        &self,
        queue: &wgpu::Queue,
        camera: &OrbitCamera,
        key_light: [f32; 3],
        ambient: [f32; 3],
        flags: RenderFlags,
    ) {
        let mut uniform = CameraUniform::from_camera(camera, key_light, ambient);
        uniform.flags = flags.bits();
        // Pad to 256 bytes so the UBO write always satisfies the WGSL
        // struct-size minimum (the Rust struct is currently 180 B; the
        // shader rounds up to 192 B, and we leave 64 B of headroom).
        let mut bytes = bytemuck::bytes_of(&uniform).to_vec();
        bytes.resize(256, 0);
        queue.write_buffer(&self.camera_buffer, 0, &bytes);
    }

    pub fn ensure_scene_color(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) {
        if self.scene_color_view.is_some()
            && let Some(tex) = self.scene_color_tex.as_ref()
            && tex.width() == width
            && tex.height() == height
        {
            return;
        }
        let (tex, view) = create_scene_color_texture(device, width, height);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("imgeditor-scene3d/compositor_bind_group"),
            layout: &self.compositor_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.compositor_sampler),
                },
            ],
        });
        self.scene_color_tex = Some(tex);
        self.scene_color_view = Some(view);
        self.compositor_bind_group = Some(bind_group);
    }
}

fn build_lit_pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    polygon_mode: wgpu::PolygonMode,
    cull_mode: Option<wgpu::Face>,
    label: &str,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[vertex_buffer_layout()],
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode,
            unclipped_depth: false,
            polygon_mode,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format(),
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    })
}

/// Two-triangle fullscreen quad in clip space ([-1, 1] x [-1, 1]).
/// UV (0, 0) is the top-left of the texture, (1, 1) is the bottom-right;
/// the compositor WGSL flips V explicitly so the texture appears upright.
fn build_quad_vertex_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    let verts = [
        crate::inspector::scene3d::mesh::Vertex {
            position: [-1.0, -1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 1.0],
        },
        crate::inspector::scene3d::mesh::Vertex {
            position: [-1.0, 1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 0.0],
        },
        crate::inspector::scene3d::mesh::Vertex {
            position: [1.0, -1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [1.0, 1.0],
        },
        crate::inspector::scene3d::mesh::Vertex {
            position: [1.0, 1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [1.0, 0.0],
        },
    ];
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("imgeditor-scene3d/quad_vertex"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    })
}

fn build_quad_index_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    let indices: [u32; 6] = [0, 1, 2, 1, 3, 2];
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("imgeditor-scene3d/quad_index"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    })
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct RenderFlags: u32 {
        const HAS_TEXTURE       = 1 << 0;
        const WIREFRAME         = 1 << 1;
        const CULL_BACK         = 1 << 2;
    }
}

/// Compute the render flags that should actually be uploaded to the GPU.
///
/// The `HAS_TEXTURE` bit is cleared when the scene has no textured meshes
/// or when the user has disabled texturing. This keeps the shader from
/// sampling a default-white texture and makes the flag state deterministic
/// regardless of how the UI toggles are wired.
pub fn effective_texture_flag(scene: &Scene, flags: RenderFlags) -> RenderFlags {
    let mut eff = flags;
    if scene.textured_mesh_count() == 0 || !flags.contains(RenderFlags::HAS_TEXTURE) {
        eff.remove(RenderFlags::HAS_TEXTURE);
    }
    eff
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_uniform_is_pod_and_aligned() {
        let size = std::mem::size_of::<CameraUniform>();
        assert!(size <= 256, "size {size} exceeded padded 256 budget");
        let mut u = CameraUniform::from_camera(
            &OrbitCamera::new(crate::inspector::scene3d::camera::Viewport {
                width: 800,
                height: 600,
            }),
            [1.0, 0.5, 0.0],
            [0.2, 0.2, 0.2],
        );
        u.flags = RenderFlags::HAS_TEXTURE.bits();
        let bytes = bytemuck::bytes_of(&u).to_vec();
        let back: &CameraUniform = bytemuck::from_bytes(&bytes);
        assert_eq!(back.flags, 1);
    }

    #[test]
    fn vertex_layout_matches_struct_stride() {
        let layout = vertex_buffer_layout();
        assert_eq!(layout.array_stride as usize, VERTEX_STRIDE);
        assert_eq!(layout.attributes.len(), 3);
    }

    #[test]
    fn render_flags_round_trip() {
        let f = RenderFlags::HAS_TEXTURE | RenderFlags::WIREFRAME;
        assert_eq!(f.bits(), 0b11);
        let u = CameraUniform {
            flags: f.bits(),
            ..Default::default()
        };
        let bytes = bytemuck::bytes_of(&u).to_vec();
        let back: &CameraUniform = bytemuck::from_bytes(&bytes);
        assert_eq!(back.flags, 0b11);
    }

    #[test]
    fn render_flags_use_correct_bit_values() {
        assert_eq!(RenderFlags::HAS_TEXTURE.bits(), 1 << 0);
        assert_eq!(RenderFlags::WIREFRAME.bits(), 1 << 1);
        assert_eq!(RenderFlags::CULL_BACK.bits(), 1 << 2);
    }
}
