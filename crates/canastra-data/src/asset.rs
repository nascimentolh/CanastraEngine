//! References to assets by package path, typed by what they point at.

use std::fmt;
use std::marker::PhantomData;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Marker types: a `MeshRef` can never be used where a `TextureRef` is expected.
#[derive(Debug)]
pub enum Mesh {}
#[derive(Debug)]
pub enum Texture {}
#[derive(Debug)]
pub enum Sound {}
#[derive(Debug)]
pub enum Effect {}
/// Script class that drives an actor, e.g. an NPC's animation set.
#[derive(Debug)]
pub enum Class {}

pub type ClassRef = AssetRef<Class>;
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

impl<K> Serialize for AssetRef<K> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.path)
    }
}

/// Data files can come from servers, so every path is validated again on load.
impl<'de, K> Deserialize<'de> for AssetRef<K> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl<K> fmt::Debug for AssetRef<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_paths() {
        let mesh = MeshRef::parse("LineageWeapons.small_sword_m00_wp").unwrap();
        assert!(TextureRef::parse("L2UI_CH3.Button.Btn1_normal").is_ok());
        assert_eq!(mesh, MeshRef::parse("lineageweapons.SMALL_SWORD_M00_WP").unwrap());
        for bad in ["", "NoPackage", "A..B", "A.B.C.D", "A.has space"] {
            assert!(TextureRef::parse(bad).is_err(), "{bad}");
        }
    }
}
