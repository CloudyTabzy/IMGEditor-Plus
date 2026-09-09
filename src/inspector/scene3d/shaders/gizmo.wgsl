struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}
struct VertexOut { @builtin(position) clip_position: vec4<f32>, }

// The prefix matches CameraUniform; pixel positions are shared with CPU hit testing.
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
    navigation_layout: vec4<f32>,
    tips: array<vec4<f32>, 6>,
    navigation_state: vec4<f32>,
}
@group(0) @binding(0) var<uniform> camera: CameraUniform;

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip_position = vec4<f32>(input.position, 1.0);
    return out;
}

fn segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, width: f32, aa: f32) -> f32 {
    let d = b - a;
    let t = clamp(dot(p - a, d) / max(dot(d, d), 0.0001), 0.0, 1.0);
    return 1.0 - smoothstep(width, width + aa, length(p - a - d * t));
}

// Original vector letter shapes remain antialiased at any DPI scale.
fn glyph(p: vec2<f32>, letter: u32, aa: f32) -> f32 {
    let tl = vec2<f32>(0.5, 0.5);
    let tr = vec2<f32>(4.5, 0.5);
    let ml = vec2<f32>(0.5, 3.5);
    let mr = vec2<f32>(4.5, 3.5);
    let bl = vec2<f32>(0.5, 6.5);
    let br = vec2<f32>(4.5, 6.5);
    let top = segment(p, tl, tr, 0.4, aa);
    let mid = segment(p, ml, mr, 0.4, aa);
    let bottom = segment(p, bl, br, 0.4, aa);
    let left = segment(p, tl, bl, 0.4, aa);
    let upper_right = segment(p, tr, mr, 0.4, aa);
    let right = segment(p, tr, br, 0.4, aa);
    switch letter {
        case 0u: { return max(max(top, mid), max(left, upper_right)); } // P
        case 1u: { return max(max(top, mid), max(left, bottom)); } // E
        case 2u: { return max(max(max(top, mid), max(left, upper_right)), segment(p, vec2<f32>(2.0, 3.5), br, 0.4, aa)); } // R
        case 3u: { return max(max(max(top, mid), bottom), max(segment(p, tl, ml, 0.4, aa), segment(p, mr, br, 0.4, aa))); } // S
        case 4u: { return max(max(top, bottom), max(left, right)); } // O
        case 5u: { return max(top, segment(p, vec2<f32>(2.5, 0.5), vec2<f32>(2.5, 6.5), 0.4, aa)); } // T
        case 6u: { return max(mid, max(left, right)); } // H
        case 7u: { return max(segment(p, tl, br, 0.4, aa), segment(p, tr, bl, 0.4, aa)); } // X
        case 8u: { return max(max(segment(p, tl, vec2<f32>(2.5, 3.5), 0.4, aa), segment(p, tr, vec2<f32>(2.5, 3.5), 0.4, aa)), segment(p, vec2<f32>(2.5, 3.5), vec2<f32>(2.5, 6.5), 0.4, aa)); } // Y
        default: { return max(max(top, bottom), segment(p, tr, bl, 0.4, aa)); } // Z
    }
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let scale = camera.navigation_layout.z;
    let p = (input.clip_position.xy - camera.navigation_layout.xy) / scale;
    let aa = 1.0 / scale;
    let hover = u32(camera.navigation_layout.w);
    let ortho = camera.navigation_state.x > 0.5;
    let bg = vec3<f32>(0.035, 0.042, 0.055);
    let light = vec3<f32>(0.85, 0.89, 0.96);

    if abs(p.x) <= 34.0 && p.y >= 63.0 && p.y <= 85.0 {
        var color = select(bg, vec3<f32>(0.16, 0.19, 0.24), hover == 7u);
        let edge = min(34.0 - abs(p.x), min(p.y - 63.0, 85.0 - p.y));
        color = mix(light * 0.45, color, smoothstep(0.0, aa, edge));
        var letters = array<u32, 5>(0,1,2,3,0);
        if ortho { letters = array<u32, 5>(4,2,5,6,4); }
        for (var i = 0u; i < 5u; i++) {
            let gp = (p - vec2<f32>(-23.2 + f32(i) * 9.6, 68.4)) / 1.6;
            color = mix(color, light, glyph(gp, letters[i], aa / 1.6));
        }
        return vec4<f32>(color, 1.0);
    }
    if length(p) > 59.0 { return vec4<f32>(0.0); }

    var color = bg;
    let colors = array<vec3<f32>, 3>(
        vec3<f32>(0.94, 0.20, 0.24),
        vec3<f32>(0.30, 0.78, 0.20),
        vec3<f32>(0.16, 0.43, 0.98),
    );
    for (var i = 0u; i < 6u; i++) {
        let tip = camera.tips[i];
        let id = u32(tip.w) - 1u;
        let endpoint = (tip.xy - camera.navigation_layout.xy) / scale;
        if id % 2u == 0u {
            color = mix(color, colors[id / 2u] * 0.7, segment(p, vec2<f32>(0.0), endpoint, 1.2, aa));
        }
    }
    for (var i = 0u; i < 6u; i++) {
        let tip = camera.tips[i];
        let id = u32(tip.w) - 1u;
        let endpoint = (tip.xy - camera.navigation_layout.xy) / scale;
        let q = p - endpoint;
        let d = length(q);
        let axis_color = colors[id / 2u];
        let positive = id % 2u == 0u;
        let fill = select(bg + axis_color * 0.10, axis_color, positive);
        let selected = ortho && tip.z > 0.9999;
        let ring = select(axis_color, light, hover == id + 1u || selected);
        color = mix(color, ring, 1.0 - smoothstep(11.0, 12.0, d));
        color = mix(color, fill, 1.0 - smoothstep(9.0, 9.0 + aa, d));
        let foreground = select(light, vec3<f32>(0.008, 0.010, 0.018), positive);
        let offset = select(vec2<f32>(-1.5, -4.55), vec2<f32>(-3.25, -4.55), positive);
        color = mix(color, foreground, glyph((q - offset) / 1.3, 7u + id / 2u, aa / 1.3));
        if !positive {
            color = mix(color, light, segment(q, vec2<f32>(-7.0, 0.0), vec2<f32>(-3.5, 0.0), 0.65, aa));
        }
    }
    return vec4<f32>(color, 1.0 - smoothstep(58.0, 59.0, length(p)));
}
