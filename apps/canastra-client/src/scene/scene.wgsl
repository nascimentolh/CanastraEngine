// Static scene geometry: world positions relative to the camera, drawn with a fixed-function style
// material of one or two texture stages. Textures hold gamma-space colors and are combined as
// Unreal's fixed-function pipeline did, then converted for the sRGB target.

struct Globals {
    view_projection: mat4x4<f32>,
    // Gamma-space RGB.
    fog_color: vec4<f32>,
    // Start and end distance; fog is linear in between.
    fog_range: vec4<f32>,
}

struct Material {
    // Rows of each stage's 2×3 texture coordinate transform.
    base_u: vec4<f32>,
    base_v: vec4<f32>,
    layer_u: vec4<f32>,
    layer_v: vec4<f32>,
    color: vec4<f32>,
    // Combine (0 none, 1 multiply, 2 add, 3 second red as alpha), combine factor, alpha cutoff, and
    // what fog blends towards (0 its color, 1 black for additive, 2 white for modulate).
    params: vec4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var<uniform> material: Material;
@group(1) @binding(1) var base: texture_2d<f32>;
@group(1) @binding(2) var layer: texture_2d<f32>;
@group(1) @binding(3) var tiling: sampler;

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
}

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // Distance along the view, for fog.
    @location(1) depth: f32,
}

@vertex
fn vs(vertex: Vertex) -> Varyings {
    var out: Varyings;
    out.position = globals.view_projection * vec4<f32>(vertex.position, 1.0);
    out.uv = vertex.uv;
    out.depth = out.position.w;
    return out;
}

fn to_linear(color: vec3<f32>) -> vec3<f32> {
    let low = color / 12.92;
    let high = pow((color + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, color <= vec3<f32>(0.04045));
}

@fragment
fn fs(in: Varyings) -> @location(0) vec4<f32> {
    let uv = vec3<f32>(in.uv, 1.0);
    let base_uv = vec2<f32>(dot(material.base_u.xyz, uv), dot(material.base_v.xyz, uv));
    let layer_uv = vec2<f32>(dot(material.layer_u.xyz, uv), dot(material.layer_v.xyz, uv));
    // Both stages sample unconditionally: texture sampling must run in uniform control flow.
    var color = textureSample(base, tiling, base_uv);
    let second = textureSample(layer, tiling, layer_uv);
    let factor = material.params.y;
    if material.params.x > 2.5 {
        color = vec4<f32>(color.rgb, color.a * second.r);
    } else if material.params.x > 1.5 {
        color = vec4<f32>((color.rgb + second.rgb) * factor, color.a);
    } else if material.params.x > 0.5 {
        color = vec4<f32>(color.rgb * second.rgb * factor, color.a * second.a);
    }
    color = min(color, vec4<f32>(1.0)) * material.color;
    if color.a < material.params.z {
        discard;
    }
    let range = globals.fog_range;
    let clear = clamp((range.y - in.depth) / max(range.y - range.x, 1.0), 0.0, 1.0);
    let neutral = select(vec3<f32>(0.0), vec3<f32>(1.0), material.params.w > 1.5);
    let target_color = select(globals.fog_color.rgb, neutral, material.params.w > 0.5);
    color = vec4<f32>(mix(target_color, color.rgb, clear), color.a);
    return vec4<f32>(to_linear(color.rgb), color.a);
}
