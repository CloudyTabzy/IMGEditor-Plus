struct CameraUniform {
    view_proj: mat4x4<f32>,
    key_light: vec4<f32>,
    ambient: vec4<f32>,
    flags: u32,
    pad: vec4<u32>,
}

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) light_dir: vec3<f32>,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var diffuse_tex: texture_2d<f32>;
@group(1) @binding(1) var diffuse_sampler: sampler;

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip_position = camera.view_proj * vec4<f32>(input.position, 1.0);
    out.world_normal = normalize(input.normal);
    out.uv = input.uv;
    out.light_dir = normalize(camera.key_light.xyz);
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let has_texture = (camera.flags & 1u) != 0u;
    let sampled = textureSample(diffuse_tex, diffuse_sampler, input.uv).rgb;
    let base = select(vec3<f32>(1.0, 1.0, 1.0), sampled, has_texture);
    let n_dot_l = max(dot(input.world_normal, input.light_dir), 0.0);
    let ambient = camera.ambient.xyz * base;
    let diffuse = base * n_dot_l;
    let lit = ambient + diffuse;
    let alpha = select(1.0, textureSample(diffuse_tex, diffuse_sampler, input.uv).a, has_texture);
    return vec4<f32>(lit, alpha);
}
