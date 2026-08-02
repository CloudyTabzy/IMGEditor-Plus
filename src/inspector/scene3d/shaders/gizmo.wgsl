struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
}

// Mirrors the Rust `CameraUniform`; only `view` is read here. The
// pad fields keep `view` at its Rust offset (192).
struct CameraUniform {
    view_proj: mat4x4<f32>,
    inverse_view_proj: mat4x4<f32>,
    key_light: vec4<f32>,
    ambient: vec4<f32>,
    eye_pos: vec4<f32>,
    flags: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
    view: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;

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

// Distance mask of the segment from the box centre to dir*0.85.
fn axis_mask(p: vec2<f32>, dir: vec2<f32>, aa: f32) -> f32 {
    let end = dir * 0.85;
    let t = clamp(dot(p, end) / max(dot(end, end), 1e-5), 0.0, 1.0);
    let d = length(p - end * t);
    return 1.0 - smoothstep(0.015, 0.020 + aa, d);
}

fn tip_mask(p: vec2<f32>, dir: vec2<f32>, aa: f32) -> f32 {
    let d = length(p - dir * 0.85);
    return 1.0 - smoothstep(0.05, 0.06 + aa, d);
}

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

    // Box border: a thin ring just inside |local| = 1.
    let edge = max(abs(local.x), abs(local.y));
    if edge > 0.96 {
        let alpha = 1.0 - smoothstep(0.96, 0.96 + aa + 0.02, edge);
        color = mix(color, vec3<f32>(0.55, 0.58, 0.62), alpha);
    }

    // Origin dot.
    let dot_dist = length(local);
    if dot_dist < 0.04 + aa {
        let alpha = 1.0 - smoothstep(0.02, 0.04 + aa, dot_dist);
        color = mix(color, vec3<f32>(0.95, 0.96, 0.98), alpha);
    }

    // Axes track the camera: rotate the world axes by the view
    // rotation and draw their screen-space projection. In view space
    // +x is right, +y is up, and +z points toward the viewer; axes
    // pointing away are dimmed and painted first so nearer axes win.
    let rot = mat3x3<f32>(camera.view[0].xyz, camera.view[1].xyz, camera.view[2].xyz);
    var dirs = array<vec3<f32>, 3>(
        rot * vec3<f32>(1.0, 0.0, 0.0),
        rot * vec3<f32>(0.0, 1.0, 0.0),
        rot * vec3<f32>(0.0, 0.0, 1.0),
    );
    var cols = array<vec3<f32>, 3>(
        vec3<f32>(0.96, 0.30, 0.30),
        vec3<f32>(0.45, 0.85, 0.50),
        vec3<f32>(0.45, 0.55, 0.95),
    );
    var idx = array<i32, 3>(0, 1, 2);
    if dirs[idx[0]].z > dirs[idx[1]].z { let t0 = idx[0]; idx[0] = idx[1]; idx[1] = t0; }
    if dirs[idx[1]].z > dirs[idx[2]].z { let t1 = idx[1]; idx[1] = idx[2]; idx[2] = t1; }
    if dirs[idx[0]].z > dirs[idx[1]].z { let t2 = idx[0]; idx[0] = idx[1]; idx[1] = t2; }

    for (var i = 0; i < 3; i = i + 1) {
        let a = dirs[idx[i]];
        let dim = select(0.40, 1.0, a.z > 0.0);
        let c = cols[idx[i]] * dim;
        color = mix(color, c, axis_mask(local, a.xy, aa));
        color = mix(color, c, tip_mask(local, a.xy, aa));
    }

    return vec4<f32>(color, 1.0);
}
