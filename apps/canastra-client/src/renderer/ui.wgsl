// One quad per UI rectangle or image piece: rounded corners, border and vertical gradient
// come from a signed distance, so edges stay smooth at any scale.

struct Globals {
    viewport: vec2<f32>,
    _padding: vec2<f32>,
}

struct Quad {
    rect: vec4<f32>,    // x, y, width, height in physical pixels
    uv: vec4<f32>,      // u0, v0, u1, v1
    top: vec4<f32>,     // linear RGBA at the top edge
    bottom: vec4<f32>,  // linear RGBA at the bottom edge
    border: vec4<f32>,  // linear RGBA
    params: vec4<f32>,  // radius, border width, textured (0 or 1), unused
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

@fragment
fn fs(in: Varyings) -> @location(0) vec4<f32> {
    let quad = quads[in.index];
    let size = quad.rect.zw;
    let radius = min(quad.params.x, min(size.x, size.y) * 0.5);
    let border_width = quad.params.y;

    let half_size = size * 0.5;
    let q = abs(in.local - half_size) - (half_size - vec2<f32>(radius));
    let distance = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
    let coverage = clamp(0.5 - distance, 0.0, 1.0);

    // Sampled unconditionally: texture sampling must run in uniform control flow.
    let sampled = textureSample(image, image_sampler, in.uv);
    let gradient = mix(quad.top, quad.bottom, clamp(in.local.y / max(size.y, 1.0), 0.0, 1.0));
    var color = select(gradient, sampled, quad.params.z > 0.5);
    if border_width > 0.0 {
        let inside = clamp(0.5 - (distance + border_width), 0.0, 1.0);
        color = mix(quad.border, color, inside);
    }
    return vec4<f32>(color.rgb, color.a * coverage);
}
