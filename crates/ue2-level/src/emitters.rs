//! `Emitter` actors and their `SpriteEmitter`s and `MeshEmitter`s: what a particle simulation needs to spawn,
//! move, size, color and draw sprites or meshes. Unset values take the Unreal Engine 2 defaults.

use ue2_assets::{Property, find, object_properties};
use ue2_package::{ObjectRef, Package};

use crate::Error;

/// How sprites blend, as `EParticleDrawStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawStyle {
    Regular,
    AlphaBlend,
    Modulated,
    Translucent,
    AlphaModulate,
    Darken,
    Brighten,
}

/// A value picked uniformly between `min` and `max`.
pub type Range = [f32; 2];

#[derive(Debug, Clone, PartialEq)]
#[expect(clippy::struct_excessive_bools, reason = "mirrors the emitter's independent Unreal flags")]
pub struct SpriteEmitter {
    pub max_particles: u32,
    pub lifetime: Range,
    /// Added to the owner's location, then a random point of `start_location` on each axis.
    pub start_offset: [f32; 3],
    pub start_location: [Range; 3],
    pub start_velocity: [Range; 3],
    pub acceleration: [f32; 3],
    /// Sprite size in world units; only X is used for uniform sizes.
    pub start_size: Range,
    /// (relative time, size multiplier) points, when set.
    pub size_scale: Vec<[f32; 2]>,
    pub size_scale_repeats: f32,
    /// (relative time, RGBA) points, when set.
    pub color_scale: Vec<(f32, [u8; 4])>,
    pub color_scale_repeats: f32,
    pub color_multiplier: [Range; 3],
    pub opacity: f32,
    /// Seconds of fading in from the start, when fading in.
    pub fade_in_end: Option<f32>,
    /// Second after which the sprite fades out, when fading out.
    pub fade_out_start: Option<f32>,
    /// Starting turn and turns per second, when spinning.
    pub spin: Option<(Range, Range)>,
    /// Center offset and turns per second about X, Y and Z, when particles revolve about the emitter.
    pub revolution: Option<([Range; 3], [Range; 3])>,
    pub texture: Option<String>,
    /// Texture cells across and down, and the cells picked from (`end` 0 means all).
    pub subdivisions: [u32; 2],
    pub subdivision_range: [u32; 2],
    pub draw_style: DrawStyle,
    pub z_test: bool,
    pub fogged: bool,
    /// Tinted by the time of day's sky color.
    pub cloud_color: bool,
    /// Fades out where it nears the geometry behind it instead of cutting into it.
    pub soft: bool,
    /// The plane sprites lie in instead of facing the camera.
    pub projection_normal: Option<[f32; 3]>,
    /// For a `MeshEmitter`, the mesh each particle draws in place of a sprite.
    pub mesh: Option<MeshShape>,
}

/// What a `MeshEmitter` draws for each particle.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshShape {
    /// Object path of the static mesh.
    pub mesh: String,
    /// Material overrides by section (`CustomMaterials`), `None` where the mesh's own applies.
    pub materials: Vec<Option<String>>,
    /// Whether the mesh's materials say how it blends (`UseMeshBlendMode`) rather than the draw style.
    pub own_blend: bool,
    /// Scale on each axis at spawn, one where unset; a uniformly sized emitter scales all three by X's.
    pub scale: [Range; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Emitter {
    pub location: [f32; 3],
    /// The actor's rotation in Unreal units, which turns the meshes its mesh emitters draw.
    pub rotation: [i32; 3],
    /// Name of the zone the emitter stands in.
    pub zone: Option<String>,
    pub sprites: Vec<SpriteEmitter>,
}

pub(crate) fn read(
    package: &Package,
    file: &[u8],
    properties: &[Property<'_>],
    zone: Option<String>,
) -> Result<Emitter, Error> {
    let mut sprites = Vec::new();
    for object in find(properties, "Emitters").and_then(|emitters| emitters.objects(package)).unwrap_or_default() {
        let ObjectRef::Export(index) = object else { continue };
        let Some(export) = package.exports().get(index) else { continue };
        let class = package.class_name(export);
        let meshes = class.eq_ignore_ascii_case("MeshEmitter");
        if !meshes && !class.eq_ignore_ascii_case("SpriteEmitter") {
            continue;
        }
        let properties = object_properties(package, file, export)?;
        // Disabled emitters spawn nothing.
        if !find(&properties, "Disabled").and_then(Property::bool).unwrap_or(false) {
            let mut sprite = sprite(package, &properties);
            if meshes {
                sprite.mesh = mesh_shape(package, &properties);
            }
            sprites.push(sprite);
        }
    }
    let location = find(properties, "Location").and_then(Property::vector).unwrap_or_default();
    let rotation = find(properties, "Rotation").and_then(Property::rotator).unwrap_or_default();
    Ok(Emitter { location, rotation, zone, sprites })
}

fn sprite(package: &Package, properties: &[Property<'_>]) -> SpriteEmitter {
    let get = |name| find(properties, name);
    let float = |name, default| get(name).and_then(Property::float).unwrap_or(default);
    let int = |name, default| get(name).and_then(Property::int).unwrap_or(default);
    let flag = |name, default| get(name).and_then(Property::bool).unwrap_or(default);
    let range = |name, default| get(name).map_or([default; 2], |range| range_of(package, range));
    let ranges = |name, default| get(name).map_or([[default; 2]; 3], |ranges| range_vector(package, ranges));
    let vector = |name| get(name).and_then(Property::vector).unwrap_or_default();
    let unsigned = |name, default| u32::try_from(int(name, default)).unwrap_or(0);
    let styles = [
        DrawStyle::Regular,
        DrawStyle::AlphaBlend,
        DrawStyle::Modulated,
        DrawStyle::Translucent,
        DrawStyle::AlphaModulate,
        DrawStyle::Darken,
        DrawStyle::Brighten,
    ];
    SpriteEmitter {
        max_particles: unsigned("MaxParticles", 10),
        lifetime: range("LifetimeRange", 4.0),
        start_offset: vector("StartLocationOffset"),
        start_location: ranges("StartLocationRange", 0.0),
        start_velocity: ranges("StartVelocityRange", 0.0),
        acceleration: vector("Acceleration"),
        start_size: ranges("StartSizeRange", 100.0)[0],
        size_scale: if flag("UseSizeScale", false) {
            curve(package, get("SizeScale"), "RelativeSize")
                .into_iter()
                .map(|(time, size)| [time, size.and_then(|size| size.float()).unwrap_or(1.0)])
                .collect()
        } else {
            Vec::new()
        },
        size_scale_repeats: float("SizeScaleRepeats", 0.0).max(1.0),
        color_scale: if flag("UseColorScale", false) {
            curve(package, get("ColorScale"), "Color")
                .into_iter()
                .map(|(time, color)| (time, color.and_then(|color| color.color()).unwrap_or([255; 4])))
                .collect()
        } else {
            Vec::new()
        },
        color_scale_repeats: float("ColorScaleRepeats", 0.0).max(1.0),
        color_multiplier: ranges("ColorMultiplierRange", 1.0),
        opacity: float("Opacity", 1.0),
        fade_in_end: flag("FadeIn", false).then(|| float("FadeInEndTime", 0.0)),
        fade_out_start: flag("FadeOut", false).then(|| float("FadeOutStartTime", 0.0)),
        revolution: flag("UseRevolution", false)
            .then(|| (ranges("RevolutionCenterOffsetRange", 0.0), ranges("RevolutionsPerSecondRange", 0.0))),
        spin: flag("SpinParticles", false)
            .then(|| (ranges("StartSpinRange", 0.0)[0], ranges("SpinsPerSecondRange", 0.0)[0])),
        texture: get("Texture").and_then(|texture| texture.object(package)).and_then(|texture| match texture {
            ObjectRef::Null => None,
            texture => Some(package.object_path(texture)),
        }),
        subdivisions: [unsigned("TextureUSubdivisions", 1).max(1), unsigned("TextureVSubdivisions", 1).max(1)],
        subdivision_range: [unsigned("SubdivisionStart", 0), unsigned("SubdivisionEnd", 0)],
        draw_style: styles
            .get(usize::from(get("DrawStyle").and_then(Property::byte).unwrap_or(3)))
            .copied()
            .unwrap_or(DrawStyle::Translucent),
        z_test: flag("ZTest", true),
        fogged: !flag("DisableFogging", false),
        cloud_color: flag("UseCloudColor", false),
        soft: flag("UseSoftParticle", false),
        // PTDU_Normal and the modes built on it lay sprites in ProjectionNormal's plane.
        projection_normal: matches!(get("UseDirectionAs").and_then(Property::byte), Some(4..=6))
            .then(|| get("ProjectionNormal").and_then(Property::vector).unwrap_or([0.0, 0.0, 1.0])),
        mesh: None,
    }
}

/// A `MeshEmitter`'s mesh and material overrides, or `None` without a mesh.
fn mesh_shape(package: &Package, properties: &[Property<'_>]) -> Option<MeshShape> {
    let path = |object| match object {
        ObjectRef::Null => None,
        object => Some(package.object_path(object)),
    };
    let mesh = find(properties, "StaticMesh").and_then(|mesh| mesh.object(package)).and_then(path)?;
    let materials = find(properties, "CustomMaterials")
        .and_then(|materials| materials.objects(package))
        .unwrap_or_default()
        .into_iter()
        .map(path)
        .collect();
    let own_blend = find(properties, "UseMeshBlendMode").and_then(Property::bool).unwrap_or(true);
    // A mesh particle's size is a scale, which starts at one rather than a sprite's hundred units.
    let mut scale = find(properties, "StartSizeRange").map_or([[1.0; 2]; 3], |ranges| range_vector(package, ranges));
    if find(properties, "UniformSize").and_then(Property::bool).unwrap_or(false) {
        let [x, ..] = scale;
        scale = [x; 3];
    }
    Some(MeshShape { mesh, materials, own_blend, scale })
}

/// The points of a time curve: each point's `RelativeTime` and its `value` field.
fn curve<'a>(package: &'a Package, points: Option<&Property<'a>>, value: &str) -> Vec<(f32, Option<Property<'a>>)> {
    let points = points.and_then(|points| points.structs(package)).unwrap_or_default();
    points
        .iter()
        .map(|point| {
            (find(point, "RelativeTime").and_then(Property::float).unwrap_or(0.0), find(point, value).copied())
        })
        .collect()
}

/// A `Range` struct: `Min` and `Max`.
pub(crate) fn range_of(package: &Package, range: &Property<'_>) -> Range {
    let fields = range.fields(package).unwrap_or_default();
    let value = |name| find(&fields, name).and_then(Property::float).unwrap_or(0.0);
    [value("Min"), value("Max")]
}

/// A `RangeVector` struct: a `Range` on each of `X`, `Y` and `Z`.
pub(crate) fn range_vector(package: &Package, ranges: &Property<'_>) -> [Range; 3] {
    let fields = ranges.fields(package).unwrap_or_default();
    ["X", "Y", "Z"].map(|axis| find(&fields, axis).map_or([0.0; 2], |range| range_of(package, range)))
}
