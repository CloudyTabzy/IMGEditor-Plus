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

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip_position = vec4<f32>(input.position, 1.0);
    // Clip-space fullscreen quad. Pass through world position so the
    // fragment shader can compute world-space coords without an inverse
    // VP. For our grid-only quad, `position` IS the world position
    // (the quad's verts are at (x, y, z) in world space).
    out.world_pos = input.position;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let world_pos = input.world_pos;
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
    var color = mix(minor_color, major_color, major_strength);
    let bg_color = vec3<f32>(0.07, 0.08, 0.10);
    color = mix(bg_color, color, line_strength * fade);

    // The XZ floor is below the model. Highlight the origin axes:
    // - X axis (red) along z=0, with x in [-axis_size, axis_size]
    // - Z axis (green) along x=0, with z in [-axis_size, axis_size]
    // Render thin lines for the main axes.
    let axis_size = 0.02;
    let on_x_axis = step(abs(world_pos.z), axis_size + line_anti) *
                   step(abs(world_pos.x), 12.0);
    let on_z_axis = step(abs(world_pos.x), axis_size + line_anti) *
                   step(abs(world_pos.z), 12.0);
    color = mix(color, vec3<f32>(0.96, 0.27, 0.27), on_x_axis * fade);
    color = mix(color, vec3<f32>(0.40, 0.85, 0.50), on_z_axis * fade);

    return vec4<f32>(color, 1.0);
}
