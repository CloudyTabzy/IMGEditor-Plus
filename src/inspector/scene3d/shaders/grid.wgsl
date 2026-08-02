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
        // Blender-style grid: measure the distance to the nearest
        // grid line in *pixel* units via screen-space derivatives, so
        // lines keep a constant ~1px width with coverage anti-aliasing
        // at every distance. This kills the moiré and thickness
        // pumping that fixed world-space widths produce.
        let minor_uv = vec2<f32>(world_pos.x, world_pos.z); // cell = 1.0
        let major_uv = minor_uv / 5.0;                      // cell = 5.0

        let minor_deriv = max(vec2<f32>(fwidth(minor_uv.x), fwidth(minor_uv.y)), vec2<f32>(1e-6));
        let major_deriv = max(vec2<f32>(fwidth(major_uv.x), fwidth(major_uv.y)), vec2<f32>(1e-6));

        let minor_d = abs(fract(minor_uv - vec2<f32>(0.5)) - vec2<f32>(0.5)) / minor_deriv;
        let major_d = abs(fract(major_uv - vec2<f32>(0.5)) - vec2<f32>(0.5)) / major_deriv;
        let minor_line = 1.0 - smoothstep(0.2, 1.0, min(minor_d.x, minor_d.y));
        let major_line = 1.0 - smoothstep(0.4, 1.5, min(major_d.x, major_d.y));

        // Once a cell shrinks below ~2px the per-pixel line position
        // becomes noise; fade that level out so the far floor settles
        // to a flat color instead of shimmering.
        let minor_fade = 1.0 - smoothstep(0.25, 0.5, max(minor_deriv.x, minor_deriv.y));
        let major_fade = 1.0 - smoothstep(0.25, 0.5, max(major_deriv.x, major_deriv.y));

        let dist = length(vec2<f32>(world_pos.x, world_pos.z));
        let fade = 1.0 - smoothstep(10.0, 80.0, dist);

        let bg_color = vec3<f32>(0.07, 0.08, 0.10);
        let minor_color = vec3<f32>(0.20, 0.23, 0.27);
        let major_color = vec3<f32>(0.42, 0.46, 0.52);
        color = mix(bg_color, minor_color, minor_line * minor_fade * fade);
        color = mix(color, major_color, major_line * major_fade * fade);

        // Origin axes on the floor (world Y=0), also pixel-width:
        // - X axis (red) along z=0
        // - Z axis (green) along x=0
        let axis_wx = max(fwidth(world_pos.x), 1e-6);
        let axis_wz = max(fwidth(world_pos.z), 1e-6);
        let on_x_axis = (1.0 - smoothstep(0.8, 1.8, abs(world_pos.z) / axis_wz)) *
                        (1.0 - smoothstep(11.0, 12.0, abs(world_pos.x)));
        let on_z_axis = (1.0 - smoothstep(0.8, 1.8, abs(world_pos.x) / axis_wx)) *
                        (1.0 - smoothstep(11.0, 12.0, abs(world_pos.z)));
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