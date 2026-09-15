//! Unreal Engine 2 materials reduced to what a fixed-function pass draws: a base texture stage, an
//! optional second stage combined with it, a tint, a blend mode and animated texture coordinates.

use ue2_assets::{Property, find, object_properties};

use crate::{Catalog, MAX_MATERIAL_DEPTH, full_path, package_name};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Blend {
    Opaque,
    /// Cut out where alpha is below the material's alpha reference.
    Masked,
    Alpha,
    /// Unreal's screen blend: adds, scaled down where what is behind is already bright.
    Translucent,
    /// Multiplies what is behind, doubled as Unreal does.
    Modulate,
    /// Adds, scaled down where what is behind is already bright.
    Brighten,
    /// Darkens what is behind where the source is bright.
    Darken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combine {
    Multiply,
    Add,
    /// The second stage's red channel becomes the alpha, as terrain alpha maps weigh their layers.
    Mask,
}

/// A texture and how its coordinates move, outermost modifier first.
#[derive(Debug, Clone, PartialEq)]
pub struct Stage {
    pub texture: String,
    pub uv: Vec<UvModifier>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UvModifier {
    /// Texture widths per second.
    Pan([f32; 2]),
    /// Divides coordinates by `scale`, then adds `offset` (in texture widths).
    Scale { scale: [f32; 2], offset: [f32; 2] },
    /// Turns `angle` radians plus `rate` radians per second about `center` (in texture widths).
    Rotate { center: [f32; 2], angle: f32, rate: f32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    pub base: Stage,
    /// A second stage combined with the base, with its factor (1, 2 or 4).
    pub layer: Option<(Stage, Combine, f32)>,
    pub blend: Blend,
    /// RGBA multiplier, 255 when untinted.
    pub color: [u8; 4],
    /// Alpha out of 255 below which a masked material is cut out; `None` cuts below half.
    pub alpha_ref: Option<u8>,
}

/// Rows of a 2×3 affine transform of texture coordinates.
pub type UvMatrix = [[f32; 3]; 2];

pub const IDENTITY: UvMatrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];

impl Stage {
    /// The coordinate transform at `time` seconds: outer modifiers apply to coordinates first.
    pub fn matrix(&self, time: f32) -> UvMatrix {
        self.uv.iter().fold(IDENTITY, |matrix, modifier| then(matrix, modifier.matrix(time)))
    }
}

impl UvModifier {
    fn matrix(self, time: f32) -> UvMatrix {
        match self {
            Self::Pan([u, v]) => [[1.0, 0.0, u * time], [0.0, 1.0, v * time]],
            Self::Scale { scale: [su, sv], offset: [ou, ov] } => [[1.0 / su, 0.0, ou], [0.0, 1.0 / sv, ov]],
            Self::Rotate { center: [cu, cv], angle, rate } => {
                let (sin, cos) = (angle + rate * time).sin_cos();
                [[cos, -sin, cu - cos * cu + sin * cv], [sin, cos, cv - sin * cu - cos * cv]]
            }
        }
    }
}

/// `first` followed by `second`.
fn then(first: UvMatrix, second: UvMatrix) -> UvMatrix {
    let [first_u, first_v] = first;
    second.map(|[from_u, from_v, offset]| {
        let mut row = [0.0, 0.0, offset];
        for ((out, u), v) in row.iter_mut().zip(first_u).zip(first_v) {
            *out += from_u * u + from_v * v;
        }
        row
    })
}

impl Catalog {
    /// Resolves the material at `path`; `None` when it is missing, invisible or of an unsupported kind.
    pub fn material(&mut self, path: &str) -> Option<Material> {
        self.resolve(path, 0)
    }

    fn resolve(&mut self, path: &str, depth: usize) -> Option<Material> {
        if depth > MAX_MATERIAL_DEPTH {
            return None;
        }
        let node = self.node(path)?;
        let inner = |catalog: &mut Self, reference: &Option<String>| catalog.resolve(reference.as_deref()?, depth + 1);
        Some(match node.class.as_str() {
            "texture" => Material {
                base: Stage { texture: path.to_owned(), uv: Vec::new() },
                layer: None,
                blend: if node.alpha_texture {
                    Blend::Alpha
                } else if node.masked {
                    Blend::Masked
                } else {
                    Blend::Opaque
                },
                color: [255; 4],
                alpha_ref: None,
            },
            "shader" => {
                let mut material = inner(self, &node.diffuse).or_else(|| inner(self, &node.self_illumination))?;
                material.blend = match node.output_blending {
                    // ponytail: an Opacity map is taken to be the diffuse texture's own alpha, as foliage shaders set it.
                    0 if node.alpha_test.is_some() => Blend::Masked,
                    0 if node.opacity => Blend::Alpha,
                    0 => material.blend,
                    1 => Blend::Masked,
                    2 => Blend::Modulate,
                    6 => Blend::Darken,
                    3 => Blend::Translucent,
                    5 => Blend::Brighten,
                    _ => return None,
                };
                material.alpha_ref = node.alpha_test.or(material.alpha_ref);
                material
            }
            "finalblend" => {
                let mut material = inner(self, &node.material)?;
                material.blend = match node.frame_buffer_blending {
                    0 if node.alpha_test.is_some() => Blend::Masked,
                    0 => Blend::Opaque,
                    1 => Blend::Modulate,
                    5 => Blend::Darken,
                    2 | 3 => Blend::Alpha,
                    4 | 8 => Blend::Translucent,
                    6 => Blend::Brighten,
                    _ => return None,
                };
                material.alpha_ref = node.alpha_test.or(material.alpha_ref);
                material
            }
            "colormodifier" => {
                let mut material = inner(self, &node.material)?;
                for (channel, tint) in material.color.iter_mut().zip(node.color.unwrap_or([255; 4])) {
                    *channel = u8::try_from(u16::from(*channel) * u16::from(tint) / 255).unwrap_or(u8::MAX);
                }
                material
            }
            "combiner" => {
                let first = inner(self, &node.material1);
                let second = inner(self, &node.material2);
                // ponytail: only material selection, multiply and add; masked operations draw Material1.
                match (node.combine_operation, first, second) {
                    (1, _, Some(second)) => second,
                    (operation @ (2 | 3), Some(mut first), Some(second)) => {
                        let combine = if operation == 2 { Combine::Multiply } else { Combine::Add };
                        first.layer = Some((second.base, combine, node.modulate));
                        first
                    }
                    (_, first, second) => first.or(second)?,
                }
            }
            class if class.starts_with("tex") => {
                let mut material = inner(self, &node.material)?;
                let size = self.texture_size(&material.base.texture).unwrap_or([1.0; 2]);
                // ponytail: oscillators and other modifiers leave coordinates untouched.
                if let Some(modifier) = node.modifier.modifier(class, size) {
                    material.base.uv.insert(0, modifier);
                    if let Some((stage, _, _)) = &mut material.layer {
                        stage.uv.insert(0, modifier);
                    }
                }
                material
            }
            _ => return None,
        })
    }

    /// What resolving reads from the object at `path`, copied out so the catalog can load more.
    fn node(&mut self, path: &str) -> Option<Node> {
        let package_name = package_name(path)?;
        let (loaded, index) = self.object(path, "")?;
        let package = &loaded.package;
        let export = package.exports().get(index)?;
        let properties = object_properties(package, &loaded.file, export).ok()?;
        let get = |name| find(&properties, name);
        let flag = |name| get(name).and_then(Property::bool).unwrap_or(false);
        let byte = |name| get(name).and_then(Property::byte).unwrap_or(0);
        let float = |name, default| get(name).and_then(Property::float).unwrap_or(default);
        let alpha_ref = get("AlphaRef").and_then(|value| value.byte().or_else(|| u8::try_from(value.int()?).ok()));
        let reference = |name| full_path(&package_name, package, get(name)?.object(package)?);
        Some(Node {
            class: package.class_name(export).to_ascii_lowercase(),
            diffuse: reference("Diffuse"),
            self_illumination: reference("SelfIllumination"),
            material: reference("Material"),
            material1: reference("Material1"),
            material2: reference("Material2"),
            alpha_texture: flag("bAlphaTexture"),
            masked: flag("bMasked"),
            alpha_test: flag("AlphaTest").then(|| alpha_ref.unwrap_or(0)),
            opacity: get("Opacity")
                .and_then(|opacity| opacity.object(package))
                .is_some_and(|opacity| !matches!(opacity, ue2_package::ObjectRef::Null)),
            modulate: if flag("Modulate4X") {
                4.0
            } else if flag("Modulate2X") {
                2.0
            } else {
                1.0
            },
            output_blending: byte("OutputBlending"),
            frame_buffer_blending: byte("FrameBufferBlending"),
            combine_operation: byte("CombineOperation"),
            color: get("Color").and_then(Property::color),
            modifier: Modifier {
                pan_direction: get("PanDirection").and_then(Property::rotator).unwrap_or_default(),
                pan_rate: float("PanRate", 0.0),
                scale: [float("UScale", 1.0), float("VScale", 1.0)],
                offset: [float("UOffset", 0.0), float("VOffset", 0.0)],
                rotation: get("Rotation").and_then(Property::rotator).unwrap_or_default(),
                rotation_type: byte("TexRotationType"),
            },
        })
    }

    /// A texture's size in texels, from its `USize` and `VSize`.
    #[expect(clippy::cast_precision_loss, reason = "texture sizes are at most a few thousand texels")]
    fn texture_size(&mut self, path: &str) -> Option<[f32; 2]> {
        let (loaded, index) = self.object(path, "Texture")?;
        let export = loaded.package.exports().get(index)?;
        let properties = object_properties(&loaded.package, &loaded.file, export).ok()?;
        let size = |name| find(&properties, name).and_then(Property::int).map(|size| size as f32);
        Some([size("USize")?, size("VSize")?])
    }
}

/// The properties of one material object that resolving it needs.
struct Node {
    /// Lower-case class name.
    class: String,
    diffuse: Option<String>,
    self_illumination: Option<String>,
    material: Option<String>,
    material1: Option<String>,
    material2: Option<String>,
    alpha_texture: bool,
    masked: bool,
    /// The alpha reference when the material cuts out by alpha.
    alpha_test: Option<u8>,
    /// Whether a Shader sets an Opacity map.
    opacity: bool,
    /// Combiner factor: 1, 2 or 4.
    modulate: f32,
    output_blending: u8,
    frame_buffer_blending: u8,
    combine_operation: u8,
    color: Option<[u8; 4]>,
    modifier: Modifier,
}

/// The values a texture modifier stores.
struct Modifier {
    pan_direction: [i32; 3],
    pan_rate: f32,
    scale: [f32; 2],
    offset: [f32; 2],
    rotation: [i32; 3],
    rotation_type: u8,
}

impl Modifier {
    #[expect(clippy::cast_precision_loss, reason = "rotator units fit a float exactly below 2^24")]
    fn modifier(&self, class: &str, [width, height]: [f32; 2]) -> Option<UvModifier> {
        let turns = |units: i32| units as f32 * std::f32::consts::TAU / 65536.0;
        let texels = [self.offset[0] / width, self.offset[1] / height];
        match class {
            "texpanner" => {
                let [pitch, yaw, _] = self.pan_direction.map(turns);
                let direction = [yaw.cos() * pitch.cos(), yaw.sin() * pitch.cos()];
                Some(UvModifier::Pan(direction.map(|axis| axis * self.pan_rate)))
            }
            "texscaler" => Some(UvModifier::Scale { scale: self.scale, offset: texels }),
            "texrotator" => {
                let yaw = turns(self.rotation[1]);
                // 0 is a fixed angle, 1 turns by Rotation every second; oscillation is not modeled.
                let (angle, rate) = if self.rotation_type == 1 { (0.0, yaw) } else { (yaw, 0.0) };
                Some(UvModifier::Rotate { center: texels, angle, rate })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(matrix: UvMatrix, [u, v]: [f32; 2]) -> [f32; 2] {
        matrix.map(|[a, b, c]| a * u + b * v + c)
    }

    fn close(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5
    }

    #[test]
    fn outer_modifiers_move_coordinates_before_inner_ones() {
        // A panner around a scaler, like the lobby sky: pan in mesh space, then scale and offset.
        let stage = Stage {
            texture: String::new(),
            uv: vec![UvModifier::Pan([-0.5, 0.0]), UvModifier::Scale { scale: [2.0, 2.0], offset: [0.0, 0.25] }],
        };
        assert!(close(apply(stage.matrix(2.0), [1.0, 1.0]), [0.0, 0.75]));

        // A quarter turn about the middle sends the right edge to the bottom.
        let rotate = UvModifier::Rotate { center: [0.5, 0.5], angle: 0.0, rate: std::f32::consts::FRAC_PI_2 };
        assert!(close(apply(rotate.matrix(1.0), [1.0, 0.5]), [0.5, 1.0]));
    }
}
