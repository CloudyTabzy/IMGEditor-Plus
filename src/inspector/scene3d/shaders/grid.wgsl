struct CameraUniform {
    view_proj: mat4x4<f32>,
    inverse_view_proj: mat4x4<f32>,
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
    @location(0) world_pos: vec3<f32>,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip_position = vec4<f32>(input.position, 1.0);
    out.world_pos = input.position;
    return out;
}

struct FragOut {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fs_main(input: VertexOut) -> FragOut {
    // The fullscreen quad's vertices have z=0 (clip-space), so
    // world_pos.z is constant across the screen. Use the inverse VP
    // to map this NDC pixel back to a world ray, then intersect with
    // the floor plane (world Y=0) for a proper 3D grid floor.
    let ndc_far = vec3<f32>(input.world_pos.x, input.world_pos.y, 1.0);
    let world_pos_far = camera.inverse_view_proj * vec4<f32>(ndc_far, 1.0);
    let world_far = world_pos_far.xyz / world_pos_far.w;
    let ndc_near = vec3<f32>(input.world_pos.x, input.world_pos.y, -1.0);
    let world_pos_near = camera.inverse_view_proj * vec4<f32>(ndc_near, 1.0);
    let world_near = world_pos_near.xyz / world_pos_near.w;
    let ray_dir = world_far - world_near;
    var t: f32 = 0.0;
    if abs(ray_dir.y) > 0.0001 {
        t = -world_near.y / ray_dir.y;
    }
    var world_pos = vec3<f32>(0.0);
    var valid: bool = false;
    if t > 0.0 {
        world_pos = world_near + ray_dir * t;
        valid = true;
    }

    var color = vec3<f32>(0.07, 0.08, 0.10);
    var depth: f32 = 0.0;
    if valid {
        let cell_size = 1.0;
        let line_width = 0.018;
        let line_anti = max(length(vec2<f32>(dpdx(world_pos.x), dpdy(world_pos.x))),
                           length(vec2<f32>(dpdx(world_pos.z), dpdy(world_pos.z))));

        let grid_uv = vec2<f32>(world_pos.x, world_pos.z) / cell_size;
        let cell_uv = grid_uv - floor(grid_uv) - vec2<f32>(0.5);
        let cell_dist = abs(cell_uv);
        let line_mask = step(cell_dist.x * cell_size, line_width + line_anti) +
                        step(cell_dist.y * cell_size, line_width + line_anti);
        let line_strength = clamp(f32(line_mask), 0.0, 1.0);

        let major_grid_uv = vec2<f32>(world_pos.x, world_pos.z) / 5.0;
        let major_uv = major_grid_uv - floor(major_grid_uv) - vec2<f32>(0.5);
        let major_dist = abs(major_uv);
        let major_mask = step(major_dist.x * 5.0, 0.04 + line_anti * 5.0) +
                         step(major_dist.y * 5.0, 0.04 + line_anti * 5.0);
        let major_strength = clamp(f32(major_mask), 0.0, 1.0);

        let dist = length(vec2<f32>(world_pos.x, world_pos.z));
        let fade = 1.0 - smoothstep(10.0, 80.0, dist);

        let minor_color = vec3<f32>(0.20, 0.23, 0.27);
        let major_color = vec3<f32>(0.42, 0.46, 0.52);
        color = mix(minor_color, major_color, major_strength);
        let bg_color = vec3<f32>(0.07, 0.08, 0.10);
        color = mix(bg_color, color, line_strength * fade);

        // Origin axes on the floor (world Y=0):
        // - X axis (red) along z=0
        // - Z axis (green) along x=0
        let axis_size = 0.02;
        let on_x_axis = step(abs(world_pos.z), axis_size + line_anti) *
                       step(abs(world_pos.x), 12.0);
        let on_z_axis = step(abs(world_pos.x), axis_size + line_anti) *
                       step(abs(world_pos.z), 12.0);
        color = mix(color, vec3<f32>(0.96, 0.27, 0.27), on_x_axis * fade);
        color = mix(color, vec3<f32>(0.40, 0.85, 0.50), on_z_axis * fade);

        // Output the correct depth so the model sorts against the
        // floor instead of being clipped by the screen-aligned quad.
        let floor_clip = camera.view_proj * vec4<f32>(world_pos, 1.0);
        depth = floor_clip.z / floor_clip.w;
    } else {
        // Ray points up; sky region. Push depth to far so the lit
        // pass overwrites cleanly.
        depth = 1.0;
    }

    return FragOut(vec4<f32>(color, 1.0), depth * 0.5 + 0.5);
}