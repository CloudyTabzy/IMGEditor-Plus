struct CameraUniform {
    view_proj: mat4x4<f32>,
    inverse_view_proj: mat4x4<f32>,
    key_light: vec4<f32>,
    ambient: vec4<f32>,
    eye_pos: vec4<f32>,
    flags: u32,
    pad: vec3<u32>,
}

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) light_dir: vec3<f32>,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var diffuse_tex: texture_2d<f32>;
@group(1) @binding(1) var diffuse_sampler: sampler;

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip_position = camera.view_proj * vec4<f32>(input.position, 1.0);
    out.world_normal = normalize(input.normal);
    out.world_pos = input.position;
    out.uv = input.uv;
    out.light_dir = normalize(camera.key_light.xyz);
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let has_texture = (camera.flags & 1u) != 0u;
    let sampled = textureSample(diffuse_tex, diffuse_sampler, input.uv);

    // Use a neutral mid-gray default material when no texture is bound.
    // Pure white swamps the Blinn-Phong shading and makes everything look flat.
    let default_base = vec3<f32>(0.502, 0.502, 0.502);
    let base = select(default_base, sampled.rgb, has_texture);
    let alpha = select(1.0, sampled.a, has_texture);

    let n = normalize(input.world_normal);
    let l = input.light_dir;
    let v = normalize(camera.eye_pos.xyz - input.world_pos);

    // Key light (Lambert diffuse).
    let n_dot_l = max(dot(n, l), 0.0);
    let ambient = camera.ambient.xyz * base;
    let diffuse = base * n_dot_l;

    // Fill light from the opposite side, slightly elevated, so back faces
    // are not pitch black and curvature reads clearly.
    let fill_dir = normalize(vec3<f32>(-l.x, 0.35, -l.z));
    let fill = base * max(dot(n, fill_dir), 0.0) * 0.30;

    // Blinn-Phong specular highlight for a subtle plastic/metal sheen.
    let h = normalize(l + v);
    let spec = pow(max(dot(n, h), 0.0), 48.0) * 0.20;

    let lit = ambient + diffuse + fill + spec;

    // Approximate sRGB gamma correction so texture colors match the 2D preview.
    let gamma = vec3<f32>(1.0 / 2.2);
    let output = pow(clamp(lit, vec3<f32>(0.0), vec3<f32>(1.0)), gamma);

    return vec4<f32>(output, alpha);
}
