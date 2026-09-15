// Static scene geometry: world positions relative to the camera, one texture per draw.

struct Globals {
    view_projection: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var color: texture_2d<f32>;
@group(1) @binding(1) var color_sampler: sampler;

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
}

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs(vertex: Vertex) -> Varyings {
    var out: Varyings;
    out.position = globals.view_projection * vec4<f32>(vertex.position, 1.0);
    out.uv = vertex.uv;
    return out;
}

@fragment
fn fs(in: Varyings) -> @location(0) vec4<f32> {
    let texel = textureSample(color, color_sampler, in.uv);
    // Masked materials (leaves, grass) cut out; blending comes with material support.
    if texel.a < 0.5 {
        discard;
    }
    return vec4<f32>(texel.rgb, 1.0);
}
