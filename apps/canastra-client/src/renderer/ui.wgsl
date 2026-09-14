// One quad per UI rectangle, shadow or image piece: rounded corners, borders, gradients and
// shadow falloff come from a signed distance, so edges stay smooth at any scale.

struct Globals {
    viewport: vec2<f32>,
    _padding: vec2<f32>,
}

struct Quad {
    rect: vec4<f32>,    // x, y, width, height in physical pixels
    uv: vec4<f32>,      // u0, v0, u1, v1; offset x, y for inset shadows
    top: vec4<f32>,     // linear RGBA at the top edge
    bottom: vec4<f32>,  // linear RGBA at the bottom edge
    border: vec4<f32>,  // linear RGBA
    params: vec4<f32>,  // radius, border width, mode (0 vertical, 1 textured, 2 radial), blur
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var<storage, read> quads: array<Quad>;
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) index: u32,
}

@vertex
fn vs(@builtin(vertex_index) vertex: u32, @builtin(instance_index) index: u32) -> Varyings {
    let quad = quads[index];
    let corner = vec2<f32>(f32(vertex & 1u), f32((vertex >> 1u) & 1u));
    let point = quad.rect.xy + corner * quad.rect.zw;
    var out: Varyings;
    out.position = vec4<f32>(point.x / globals.viewport.x * 2.0 - 1.0, 1.0 - point.y / globals.viewport.y * 2.0, 0.0, 1.0);
    out.local = corner * quad.rect.zw;
    out.uv = mix(quad.uv.xy, quad.uv.zw, corner);
    out.index = index;
    return out;
}

/// Signed distance from `point` to a rounded box centered at `center`.
fn rounded_box(point: vec2<f32>, center: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(point - center) - (half_size - vec2<f32>(radius));
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

@fragment
fn fs(in: Varyings) -> @location(0) vec4<f32> {
    let quad = quads[in.index];
    let size = quad.rect.zw;
    let border_width = quad.params.y;
    let mode = quad.params.z;
    let blur = quad.params.w;
    let half_size = size * 0.5;

    // An outer shadow's quad is larger than its shape by `blur` on every side.
    let outer_shadow = blur > 0.0 && mode < 0.5;
    let shape_half = max(half_size - select(vec2<f32>(0.0), vec2<f32>(blur), outer_shadow), vec2<f32>(0.0));
    let radius = min(quad.params.x, min(shape_half.x, shape_half.y));
    let distance = rounded_box(in.local, half_size, shape_half, radius);
    var coverage = clamp(0.5 - distance, 0.0, 1.0);
    if outer_shadow {
        coverage = 1.0 - smoothstep(-blur, blur, distance);
    }
    if mode > 2.5 {
        // Inset: strongest at the offset shape's edge, gone `blur` pixels inside it, clipped to the shape.
        let inner = rounded_box(in.local - quad.uv.xy, half_size, shape_half, radius);
        coverage *= smoothstep(-max(blur, 0.5), 0.5, inner);
    }

    // Sampled unconditionally: texture sampling must run in uniform control flow.
    let sampled = textureSample(image, image_sampler, in.uv);
    let vertical = mix(quad.top, quad.bottom, clamp(in.local.y / max(size.y, 1.0), 0.0, 1.0));
    let radial = mix(quad.top, quad.bottom, clamp(length(in.local - half_size) / max(length(half_size), 1.0), 0.0, 1.0));
    var color = select(vertical, radial, mode > 1.5 && mode < 2.5);
    color = select(color, sampled, mode > 0.5 && mode < 1.5);
    if border_width > 0.0 {
        let inside = clamp(0.5 - (distance + border_width), 0.0, 1.0);
        color = mix(quad.border, color, inside);
    }
    return vec4<f32>(color.rgb, color.a * coverage);
}
