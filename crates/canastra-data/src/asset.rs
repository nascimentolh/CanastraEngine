//! References to assets by package path, typed by what they point at.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// Marker types: a `MeshRef` can never be used where a `TextureRef` is expected.
#[derive(Debug)]
pub enum Mesh {}
#[derive(Debug)]
pub enum Texture {}
#[derive(Debug)]
pub enum Sound {}
#[derive(Debug)]
pub enum Effect {}

pub type MeshRef = AssetRef<Mesh>;
pub type TextureRef = AssetRef<Texture>;
pub type SoundRef = AssetRef<Sound>;
pub type EffectRef = AssetRef<Effect>;

/// `Package.Object` or `Package.Group.Object`. Paths compare case-insensitively,
/// like the engine that produced them.
pub struct AssetRef<K> {
    path: String,
    kind: PhantomData<fn() -> K>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidAssetPath(pub String);

impl fmt::Display for InvalidAssetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` is not a Package.Object asset path", self.0)
    }
}

impl std::error::Error for InvalidAssetPath {}

impl<K> AssetRef<K> {
    pub fn parse(path: &str) -> Result<Self, InvalidAssetPath> {
        let segments: Vec<&str> = path.split('.').collect();
        let valid_segment = |s: &&str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_graphic());
        if !(2..=3).contains(&segments.len()) || !segments.iter().all(valid_segment) {
            return Err(InvalidAssetPath(path.to_owned()));
        }
        Ok(Self { path: path.to_owned(), kind: PhantomData })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn package(&self) -> &str {
        self.path.split('.').next().unwrap_or_default()
    }
}

impl<K> Clone for AssetRef<K> {
    fn clone(&self) -> Self {
        Self { path: self.path.clone(), kind: PhantomData }
    }
}

impl<K> PartialEq for AssetRef<K> {
    fn eq(&self, other: &Self) -> bool {
        self.path.eq_ignore_ascii_case(&other.path)
    }
}

impl<K> Eq for AssetRef<K> {}

impl<K> Hash for AssetRef<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for byte in self.path.bytes() {
            state.write_u8(byte.to_ascii_lowercase());
        }
    }
}

impl<K> fmt::Debug for AssetRef<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.path)
    }
}

impl<K> fmt::Display for AssetRef<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_paths() {
        let mesh = MeshRef::parse("LineageWeapons.small_sword_m00_wp").unwrap();
        assert_eq!(mesh.package(), "LineageWeapons");
        assert!(TextureRef::parse("L2UI_CH3.Button.Btn1_normal").is_ok());
        assert_eq!(mesh, MeshRef::parse("lineageweapons.SMALL_SWORD_M00_WP").unwrap());
        for bad in ["", "NoPackage", "A..B", "A.B.C.D", "A.has space"] {
            assert!(TextureRef::parse(bad).is_err(), "{bad}");
        }
    }
}
