struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
}

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip_position = vec4<f32>(input.position, 1.0);
    out.ndc = input.position.xy;
    return out;
}

const GIZMO_SIZE: f32 = 0.30;
const GIZMO_ORIGIN_X: f32 = 0.78;
const GIZMO_ORIGIN_Y: f32 = -0.78;

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    // The fullscreen quad covers the entire pane in NDC. The gizmo
    // occupies the bottom-right GIZMO_SIZE-by-GIZMO_SIZE box centered
    // at (GIZMO_ORIGIN_X, GIZMO_ORIGIN_Y).
    let origin = vec2<f32>(GIZMO_ORIGIN_X, GIZMO_ORIGIN_Y);
    let scale = GIZMO_SIZE;
    let half = scale * 0.5;
    let inside = step(origin.x - half, input.ndc.x) *
                step(input.ndc.x, origin.x + half) *
                step(origin.y - half, input.ndc.y) *
                step(input.ndc.y, origin.y + half);
    if inside == 0 {
        return vec4<f32>(0.0);
    }

    // Local coords inside the box: -1 to 1 on each axis.
    let local = (input.ndc - origin) / half;

    // Anti-aliasing factor.
    let aa = (length(vec2<f32>(dpdx(local.x), dpdy(local.x))) + length(vec2<f32>(dpdx(local.y), dpdy(local.y)))) * 0.5;

    // Background of the gizmo box.
    let bg = vec3<f32>(0.10, 0.12, 0.14);
    var color = bg;

    // Box border.
    let border = max(max(abs(local.x), abs(local.y)) - 0.96, 0.0);
    if border < aa + 0.02 {
        let alpha = 1.0 - smoothstep(0.0, aa + 0.02, border);
        color = mix(color, vec3<f32>(0.55, 0.58, 0.62), alpha);
    }

    // Origin dot.
    let dot_dist = length(local);
    if dot_dist < 0.04 + aa {
        let alpha = 1.0 - smoothstep(0.02, 0.04 + aa, dot_dist);
        color = mix(color, vec3<f32>(0.95, 0.96, 0.98), alpha);
    }

    // Three colored axes: X (red), Y (green), Z (blue).
    let x_pos = local.x + aa;
    let y_pos = local.y + aa;
    let x_axis = smoothstep(0.020 + aa, 0.015, abs(y_pos)) *
               step(0.0, x_pos) * step(x_pos, 0.85);
    color = mix(color, vec3<f32>(0.96, 0.30, 0.30), x_axis);

    let y_axis = smoothstep(0.020 + aa, 0.015, abs(x_pos)) *
               step(0.0, y_pos) * step(y_pos, 0.85);
    color = mix(color, vec3<f32>(0.45, 0.85, 0.50), y_axis);

    let z_axis = smoothstep(0.020 + aa, 0.015, abs(y_pos)) *
                step(0.85, x_pos) * step(x_pos, 1.0);
    color = mix(color, vec3<f32>(0.45, 0.55, 0.95), z_axis);

    // Letter labels in the same colors.
    let label_size = 0.16;
    let x_label_pos = vec2<f32>(local.x - 0.86, local.y - 0.10);
    let y_label_pos = vec2<f32>(local.x + 0.10, local.y + 0.86);
    let z_label_pos = vec2<f32>(local.x - 0.86, local.y + 0.86);

    return vec4<f32>(color, 1.0);
}
