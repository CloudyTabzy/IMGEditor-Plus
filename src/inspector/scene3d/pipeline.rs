//! GPU side of the embedded 3D viewer.
//!
//! Two render pipelines are built once per device and reused:
//!
//! - **Lit** — vertex + fragment WGSL, full Lambert + ambient, optional
//!   diffuse texture selected at draw time by a flag in the camera UBO.
//! - **Wireframe** — same vertex stage, line-list rasteriser,
//!   solid-colour fragment.
//!
//! One 1×1 white "default diffuse" texture is allocated so untextured
//! meshes can use the same bind group layout as textured meshes; the
//! lit fragment shader multiplies by `flags & 1` to select between the
//! two.
//!
//! The CPU `Scene` and the GPU `SceneGpu` are decoupled on purpose:
//! CPU data lives in the widget `State`, GPU resources are owned here
//! and rebuilt only when the scene changes or the viewport resizes.

use std::num::NonZeroU32;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

use crate::inspector::scene3d::camera::OrbitCamera;
use crate::inspector::scene3d::mesh::{SceneMesh, SceneTexture, VERTEX_STRIDE};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub key_light: [f32; 4],
    pub ambient: [f32; 4],
    pub flags: u32,
    pub _pad: [u32; 4],
}

impl CameraUniform {
    pub fn from_camera(camera: &OrbitCamera, key_light: [f32; 3], ambient: [f32; 3]) -> Self {
        Self {
            view_proj: camera.view_proj().to_cols_array_2d(),
            key_light: [key_light[0], key_light[1], key_light[2], 0.0],
            ambient: [ambient[0], ambient[1], ambient[2], 0.0],
            flags: 0,
            _pad: [0; 4],
        }
    }
}

pub const LIT_WGSL: &str = include_str!("shaders/lit.wgsl");
pub const WIREFRAME_WGSL: &str = include_str!("shaders/wireframe.wgsl");

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

pub fn depth_format() -> wgpu::TextureFormat {
    wgpu::TextureFormat::Depth32Float
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

pub fn create_msaa_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    sample_count: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("imgeditor-scene3d/msaa"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
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
    pub wireframe: wgpu::RenderPipeline,
    pub camera_layout: wgpu::BindGroupLayout,
    pub texture_layout: wgpu::BindGroupLayout,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub default_diffuse: GpuTexture,
}

impl ScenePipelines {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
    ) -> Arc<Self> {
        let lit_module = lit_shader_module(device);
        let wire_module = wireframe_shader_module(device);

        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("imgeditor-scene3d/camera_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(128),
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

        let lit = build_pipeline(
            device,
            &lit_module,
            &pipeline_layout,
            target_format,
            wgpu::PolygonMode::Fill,
            "imgeditor-scene3d/lit_pipeline",
        );

        let wireframe = build_pipeline(
            device,
            &wire_module,
            &pipeline_layout,
            target_format,
            wgpu::PolygonMode::Line,
            "imgeditor-scene3d/wireframe_pipeline",
        );

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("imgeditor-scene3d/camera_ubo"),
            size: 128,
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

        Arc::new(Self {
            lit,
            wireframe,
            camera_layout,
            texture_layout,
            camera_buffer,
            camera_bind_group,
            default_diffuse,
        })
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
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
    }
}

fn build_pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    polygon_mode: wgpu::PolygonMode,
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
            cull_mode: Some(wgpu::Face::Back),
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

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct RenderFlags: u32 {
        const HAS_TEXTURE       = 1 << 0;
        const WIREFRAME         = 1 << 1;
        const CULL_BACK         = 1 << 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_uniform_is_pod_and_aligned() {
        let size = std::mem::size_of::<CameraUniform>();
        assert!(size <= 128, "size {size} exceeded padded 128 budget");
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
        let mut u = CameraUniform::default();
        u.flags = f.bits();
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

#[allow(unused_imports)]
const _NONZERO_PROBE: Option<NonZeroU32> = None;
