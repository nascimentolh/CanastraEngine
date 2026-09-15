//! Unreal Engine 2 objects read in place from a decrypted package (see `ue2-package`).
//!
//! Nothing is converted or rewritten: parsed objects borrow the original bytes, and
//! pixels are decoded to RGBA only when a caller asks for them.

mod properties;
mod static_mesh;
mod texture;

use std::fmt;

use ue2_core::ReadError;

pub use properties::{Property, find, object_properties};
pub use static_mesh::{Section, StaticMesh, read_static_mesh};
pub use texture::{Image, Mip, Texture, TextureFormat, decode_rgba, decode_texture, read_palette, read_texture};

#[derive(Debug)]
pub enum Error {
    Read(ReadError),
    Package(ue2_package::Error),
    ExportOutOfRange,
    UnknownPropertyType(u8),
    UnknownTextureFormat(u8),
    UnsupportedFormat(TextureFormat),
    MissingPalette,
    NoMipArray,
    /// Static mesh streams that do not fit together.
    BadMesh,
    MipTooShort {
        needed: usize,
        actual: usize,
    },
    TrailingBytes(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(f),
            Self::Package(error) => error.fmt(f),
            Self::ExportOutOfRange => f.write_str("export data lies outside the file"),
            Self::UnknownPropertyType(kind) => write!(f, "unknown property type {kind}"),
            Self::UnknownTextureFormat(format) => write!(f, "unknown texture format {format}"),
            Self::UnsupportedFormat(format) => write!(f, "decoding {format:?} is not supported"),
            Self::MissingPalette => f.write_str("paletted texture without a palette in this package"),
            Self::NoMipArray => f.write_str("no mip array ends at the end of the object"),
            Self::BadMesh => f.write_str("static mesh streams do not fit together"),
            Self::MipTooShort { needed, actual } => write!(f, "mip needs {needed} bytes, has {actual}"),
            Self::TrailingBytes(len) => write!(f, "{len} unread bytes at the end of the object"),
        }
    }
}

impl std::error::Error for Error {}

impl From<ReadError> for Error {
    fn from(error: ReadError) -> Self {
        Self::Read(error)
    }
}

impl From<ue2_package::Error> for Error {
    fn from(error: ue2_package::Error) -> Self {
        Self::Package(error)
    }
}
