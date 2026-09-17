// Static scene geometry: world positions relative to the camera, drawn with a fixed-function style
// material of one or two texture stages. Textures hold gamma-space colors that are combined, written
// and blended as they are, as Unreal's fixed-function pipeline did.

struct Globals {
    view_projection: mat4x4<f32>,
    // Gamma-space RGB.
    fog_color: vec4<f32>,
    // Start and end distance, fog linear in between, then the near plane's distance.
    fog_range: vec4<f32>,
    // The hour's light in world zones: toward the sun, then for each ramp (static mesh, terrain, BSP, actor) its
    // ambient, sun and ground bounce colors.
    toward_sun: vec4<f32>,
    ramps: array<vec4<f32>, 12>,
}

struct Material {
    // Rows of each stage's 2×3 texture coordinate transform.
    base_u: vec4<f32>,
    base_v: vec4<f32>,
    layer_u: vec4<f32>,
    layer_v: vec4<f32>,
    color: vec4<f32>,
    // Combine (0 none, 1 multiply, 2 add, 3 second red as alpha, 4 add where the base is opaque),
    // combine factor, alpha cutoff, and how the batch blends for fog and fading (0 alpha or opaque,
    // 1 translucent or brighten, 2 modulate, 3 darken; ten more when fog is off).
    params: vec4<f32>,
    // Distance over which a soft sprite fades out in front of the geometry behind it, zero when hard; then 1
    // for masked batches, whose alpha becomes sample coverage.
    soft: vec4<f32>,
    // Color added unlit where the base texture's alpha marks it, black when nothing glows.
    glow: vec4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;
// The depth level geometry wrote; a placeholder while level geometry itself draws.
@group(0) @binding(1) var scene_depth: texture_depth_multisampled_2d;
@group(1) @binding(0) var<uniform> material: Material;
@group(1) @binding(1) var base: texture_2d<f32>;
@group(1) @binding(2) var layer: texture_2d<f32>;
@group(1) @binding(3) var tiling: sampler;

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    // Light the level stored, or the color itself where the hour lights nothing.
    @location(2) color: vec4<f32>,
    @location(3) normal: vec3<f32>,
    // The ramp that lights the vertex (zero for none), and how much of the sun and of the sky reach it.
    @location(4) light: vec3<f32>,
}

// A hemisphere ambient, dimmer and ground-tinted facing down, and a slightly wrapped sun.
fn daylight(normal: vec3<f32>, ramp: i32, sunlit: f32, skylit: f32) -> vec3<f32> {
    let n = normalize(normal);
    let row = (ramp - 1) * 3;
    let ambient = globals.ramps[row].rgb;
    let sun = globals.ramps[row + 1].rgb;
    let ground = globals.ramps[row + 2].rgb;
    let sky_weight = clamp(n.z * 0.5 + 0.5, 0.0, 1.0);
    let incidence = dot(n, globals.toward_sun.xyz);
    let wrapped = clamp((incidence + 0.08) / 1.08, 0.0, 1.0);
    let diffuse = max(incidence, 0.0) + (wrapped - max(incidence, 0.0)) * 0.35;
    return ambient * (ground + (vec3<f32>(1.0) - ground) * sky_weight) * skylit + sun * diffuse * sunlit;
}

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // Distance along the view, for fog.
    @location(1) depth: f32,
    @location(2) color: vec4<f32>,
}

@vertex
fn vs(vertex: Vertex) -> Varyings {
    var out: Varyings;
    out.position = globals.view_projection * vec4<f32>(vertex.position, 1.0);
    out.uv = vertex.uv;
    out.depth = out.position.w;
    let ramp = i32(round(vertex.light.x));
    if ramp > 0 {
        out.color = vec4<f32>(vertex.color.rgb + daylight(vertex.normal, ramp, vertex.light.y, vertex.light.z), vertex.color.a);
    } else {
        out.color = vertex.color;
    }
    return out;
}

@fragment
fn fs(in: Varyings) -> @location(0) vec4<f32> {
    let uv = vec3<f32>(in.uv, 1.0);
    let base_uv = vec2<f32>(dot(material.base_u.xyz, uv), dot(material.base_v.xyz, uv));
    let layer_uv = vec2<f32>(dot(material.layer_u.xyz, uv), dot(material.layer_v.xyz, uv));
    // Both stages sample unconditionally: texture sampling must run in uniform control flow.
    var color = textureSample(base, tiling, base_uv);
    let glow = material.glow.rgb * color.a;
    let second = textureSample(layer, tiling, layer_uv);
    let factor = material.params.y;
    if material.params.x > 3.5 {
        color = vec4<f32>(color.rgb + second.rgb * color.a * factor, color.a);
    } else if material.params.x > 2.5 {
        color = vec4<f32>(color.rgb, color.a * second.r);
    } else if material.params.x > 1.5 {
        color = vec4<f32>((color.rgb + second.rgb) * factor, color.a);
    } else if material.params.x > 0.5 {
        color = vec4<f32>(color.rgb * second.rgb * factor, color.a * second.a);
    }
    color = min(color, vec4<f32>(1.0)) * material.color * in.color;
    color = vec4<f32>(color.rgb + glow, color.a);
    // Taken before any discard, while derivatives are still defined.
    let alpha_width = max(fwidth(color.a), 0.0001);
    let unfogged = material.params.w > 9.5;
    let neutral_kind = material.params.w - select(0.0, 10.0, unfogged);
    // Blends that ignore alpha fade through the vertex color instead: translucent, brighten and darken
    // towards black, modulate towards mid gray, which its doubling leaves unchanged. Level geometry has
    // opaque vertex colors, so only particles change.
    var fade = in.color.a;
    if material.soft.x > 0.0 {
        // Reverse Z with no far plane stores near / distance, zero where nothing was drawn.
        let stored = textureLoad(scene_depth, vec2<i32>(in.position.xy), 0);
        let behind = select(1.0e30, globals.fog_range.z / stored, stored > 0.0) - in.depth;
        let soft = clamp(behind / material.soft.x, 0.0, 1.0);
        fade *= soft;
        color = vec4<f32>(color.rgb, color.a * soft);
    }
    let modulate = neutral_kind > 1.5 && neutral_kind < 2.5;
    if modulate {
        color = vec4<f32>(mix(vec3<f32>(0.5), color.rgb, fade), color.a);
    } else if neutral_kind > 0.5 {
        color = vec4<f32>(color.rgb * fade, color.a);
    }
    if material.soft.y > 0.5 {
        // Fermata's cut-out: only fully clear texels discard; the rest cover as many samples as their alpha
        // stands above the reference, sharpened to about a pixel.
        if color.a <= 0.0001 {
            discard;
        }
        color = vec4<f32>(color.rgb, clamp((color.a - material.params.z) / alpha_width + 0.5, 0.0, 1.0));
    } else if color.a < material.params.z {
        discard;
    }
    let range = globals.fog_range;
    let fogged = clamp((range.y - in.depth) / max(range.y - range.x, 1.0), 0.0, 1.0);
    let clear = select(fogged, 1.0, unfogged);
    // Fog as Fermata applies it per blend: opaque and alpha surfaces towards the fog color, modulate
    // towards its neutral gray, darken towards black, and translucent and brighten tinted by the fog so
    // black stays transparent and white lands on the fog color.
    if neutral_kind < 0.5 {
        color = vec4<f32>(mix(globals.fog_color.rgb, color.rgb, clear), color.a);
    } else if neutral_kind < 1.5 {
        color = vec4<f32>(color.rgb * mix(globals.fog_color.rgb, vec3<f32>(1.0), clear), color.a);
    } else if modulate {
        color = vec4<f32>(mix(vec3<f32>(0.5), color.rgb, clear), color.a);
    } else {
        color = vec4<f32>(color.rgb * clear, color.a);
    }
    return color;
}
