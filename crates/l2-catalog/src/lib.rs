//! Assets of an installed client found by path (`Package.Name` or `Package.Group.Name`) and read in
//! place: textures, static and skeletal meshes, animations and the texture a material draws with. Packages load on first use
//! and stay loaded.

mod fade;
mod material;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use ue2_assets::{Image, MeshAnimation, SkeletalMesh, StaticMesh};
use ue2_package::{ObjectRef, Package};

pub use fade::Fade;
pub use material::{Blend, Combine, IDENTITY, Material, Stage, UvMatrix, UvModifier};

/// Folders of the client that hold asset packages, with the extension used there.
const FOLDERS: [(&str, &str); 5] =
    [("SysTextures", "utx"), ("Textures", "utx"), ("StaticMeshes", "usx"), ("Animations", "ukx"), ("Sounds", "uax")];

/// Deeper material chains than this are treated as cycles.
const MAX_MATERIAL_DEPTH: usize = 8;
/// Frames of one animated texture to follow before giving up on a chain that never closes.
const MAX_TEXTURE_FRAMES: usize = 64;

#[derive(Default)]
pub struct Catalog {
    /// Lower-case package name to file.
    files: HashMap<String, PathBuf>,
    /// Lower-case package name to its contents, `None` when it failed to load.
    packages: HashMap<String, Option<Loaded>>,
}

struct Loaded {
    file: Vec<u8>,
    package: Package,
    /// Lower-case `Group.Name` and bare `Name` to export index.
    objects: HashMap<String, usize>,
}

/// 16-bit terrain heights, row by row.
#[derive(Debug, Clone)]
pub struct Heightmap {
    pub width: usize,
    pub height: usize,
    pub samples: Vec<u16>,
}

/// A texture ready for a GPU: the client's own blocks with the mip chain it stores, or plain pixels for the
/// formats a GPU cannot read as they are.
#[derive(Debug, Clone)]
pub struct Pixels {
    pub layout: Layout,
    pub width: u32,
    pub height: u32,
    /// Each mip level's bytes, largest first.
    pub mips: Vec<Vec<u8>>,
}

/// How a texture's bytes are laid out: block compressed, four by four texels a block, or plain RGBA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Colour with one bit of transparency, half a byte a texel.
    Bc1,
    /// Colour with four bits of transparency, one byte a texel.
    Bc2,
    /// Colour with interpolated transparency, one byte a texel.
    Bc3,
    Rgba,
}

/// A skeletal mesh with its default animation and each section's material as client-wide paths.
#[derive(Debug, Clone)]
pub struct Skinned {
    pub mesh: SkeletalMesh,
    pub animation: Option<String>,
    pub materials: Vec<Option<String>>,
}

/// A static mesh with each section's material as a client-wide path.
#[derive(Debug, Clone)]
pub struct Mesh {
    pub mesh: StaticMesh,
    /// Material of each section, `None` when unset.
    pub materials: Vec<Option<String>>,
}

impl Layout {
    /// Bytes one four by four block of texels takes.
    fn block_bytes(self) -> usize {
        match self {
            Self::Bc1 => 8,
            Self::Bc2 | Self::Bc3 => 16,
            Self::Rgba => 64,
        }
    }
}

/// How many blocks a mip level of a texture holds.
fn blocks_of(width: u32, height: u32, level: usize) -> usize {
    let side = |size: u32| {
        let shrunk = (size >> u32::try_from(level).unwrap_or(0)).max(1);
        (shrunk as usize).div_ceil(4)
    };
    side(width) * side(height)
}

impl Catalog {
    /// Indexes the asset packages under a client root; missing folders are skipped.
    pub fn open(client_root: &Path) -> Self {
        let mut files = HashMap::new();
        for (folder, extension) in FOLDERS {
            for entry in fs::read_dir(client_root.join(folder)).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
                    && let Some(stem) = path.file_stem()
                {
                    files.insert(stem.to_string_lossy().to_lowercase(), path);
                }
            }
        }
        Self { files, packages: HashMap::new() }
    }

    /// Decodes the texture at `path`, or `None` when it does not exist or cannot be read.
    pub fn texture(&mut self, path: &str) -> Option<Image> {
        let (loaded, index) = self.object(path, "Texture")?;
        ue2_assets::decode_texture(&loaded.package, &loaded.file, index).ok()
    }

    /// The texture at `path` as a GPU takes it. With `blocks` the client's own compressed blocks and mips
    /// are kept, which a GPU reads directly; without it, and for formats that are not blocks, the top mip is
    /// decoded to RGBA.
    pub fn pixels(&mut self, path: &str, blocks: bool) -> Option<Pixels> {
        let (loaded, index) = self.object(path, "Texture")?;
        let export = loaded.package.exports().get(index)?;
        let texture = ue2_assets::read_texture(&loaded.package, &loaded.file, export).ok()?;
        let layout = match texture.format {
            ue2_assets::TextureFormat::Dxt1 => Layout::Bc1,
            ue2_assets::TextureFormat::Dxt3 => Layout::Bc2,
            ue2_assets::TextureFormat::Dxt5 => Layout::Bc3,
            _ => Layout::Rgba,
        };
        if !blocks || layout == Layout::Rgba {
            let image = ue2_assets::decode_texture(&loaded.package, &loaded.file, index).ok()?;
            return Some(Pixels {
                layout: Layout::Rgba,
                width: image.width,
                height: image.height,
                mips: vec![image.rgba],
            });
        }
        let top = texture.mips.first()?;
        let (width, height) = (top.width, top.height);
        // Mips run to the smallest one the client stored; any that does not hold whole blocks ends the chain.
        let mut mips = Vec::new();
        for (level, mip) in texture.mips.iter().enumerate() {
            let expected = blocks_of(width, height, level) * layout.block_bytes();
            if mip.data.len() < expected || expected == 0 {
                break;
            }
            mips.push(mip.data.get(..expected)?.to_vec());
        }
        (!mips.is_empty()).then_some(Pixels { layout, width, height, mips })
    }

    /// The raw samples of the G16 texture at `path`.
    /// The sound at `path`, as the file the client stores: RIFF WAV.
    pub fn sound(&mut self, path: &str) -> Option<Vec<u8>> {
        let (loaded, index) = self.object(path, "sound")?;
        let export = loaded.package.exports().get(index)?;
        ue2_assets::read_sound(&loaded.package, &loaded.file, export).ok().map(<[u8]>::to_vec)
    }

    pub fn heightmap(&mut self, path: &str) -> Option<Heightmap> {
        let (loaded, index) = self.object(path, "Texture")?;
        let texture =
            ue2_assets::read_texture(&loaded.package, &loaded.file, loaded.package.exports().get(index)?).ok()?;
        let mip = texture.mips.first().filter(|_| texture.format == ue2_assets::TextureFormat::G16)?;
        let (width, height) = (mip.width as usize, mip.height as usize);
        let samples: Vec<u16> = mip.data.as_chunks::<2>().0.iter().map(|&bytes| u16::from_le_bytes(bytes)).collect();
        (samples.len() >= width * height).then_some(Heightmap { width, height, samples })
    }

    pub fn static_mesh(&mut self, path: &str) -> Option<Mesh> {
        let package_name = package_name(path)?;
        let (loaded, index) = self.object(path, "StaticMesh")?;
        let export = loaded.package.exports().get(index)?;
        let mesh = ue2_assets::read_static_mesh(&loaded.package, &loaded.file, export).ok()?;
        let materials =
            mesh.materials.iter().map(|&material| full_path(&package_name, &loaded.package, material)).collect();
        Some(Mesh { mesh, materials })
    }

    /// The skeletal mesh at `path`, with its default animation and each section's material as client paths.
    pub fn skeletal_mesh(&mut self, path: &str) -> Option<Skinned> {
        let package_name = package_name(path)?;
        let (loaded, index) = self.object(path, "SkeletalMesh")?;
        let export = loaded.package.exports().get(index)?;
        let mesh = ue2_assets::read_skeletal_mesh(&loaded.package, &loaded.file, export).ok()?;
        let animation = full_path(&package_name, &loaded.package, mesh.animation);
        let materials =
            mesh.materials.iter().map(|&material| full_path(&package_name, &loaded.package, material)).collect();
        Some(Skinned { mesh, animation, materials })
    }

    pub fn mesh_animation(&mut self, path: &str) -> Option<MeshAnimation> {
        let (loaded, index) = self.object(path, "MeshAnimation")?;
        let export = loaded.package.exports().get(index)?;
        ue2_assets::read_mesh_animation(&loaded.package, &loaded.file, export).ok()
    }

    /// The loaded package and export index of `path`, when its class is `class` (any when empty).
    fn object(&mut self, path: &str, class: &str) -> Option<(&Loaded, usize)> {
        let path = path.to_lowercase();
        let (package_name, object) = path.split_once('.')?;
        let loaded = self.package(package_name)?;
        let index = *loaded.objects.get(object)?;
        let export = loaded.package.exports().get(index)?;
        (class.is_empty() || loaded.package.class_name(export).eq_ignore_ascii_case(class)).then_some((loaded, index))
    }

    fn package(&mut self, name: &str) -> Option<&Loaded> {
        if !self.packages.contains_key(name) {
            let loaded = self.files.get(name).and_then(|path| load(path));
            self.packages.insert(name.to_owned(), loaded);
        }
        self.packages.get(name)?.as_ref()
    }
}

fn package_name(path: &str) -> Option<String> {
    path.split_once('.').map(|(package, _)| package.to_lowercase())
}

/// A reference read inside `package` as a client-wide path: imports already start with their
/// package, exports get the package's own name in front.
fn full_path(package_name: &str, package: &Package, object: ObjectRef) -> Option<String> {
    match object {
        ObjectRef::Null => None,
        ObjectRef::Import(_) => Some(package.object_path(object)),
        ObjectRef::Export(_) => Some(format!("{package_name}.{}", package.object_path(object))),
    }
}

fn load(path: &Path) -> Option<Loaded> {
    let bytes = fs::read(path).ok()?;
    let file = l2_crypto::decrypt(&bytes, path).ok()?.into_owned();
    let package = Package::parse(&file).ok()?;
    let mut objects = HashMap::new();
    for (index, export) in package.exports().iter().enumerate() {
        // Two-part references name the object without its group, so index both forms.
        objects.insert(package.object_path(ObjectRef::Export(index)).to_lowercase(), index);
        objects.entry(package.name(export.name).to_lowercase()).or_insert(index);
    }
    Some(Loaded { file, package, objects })
}
