//! Unreal Engine 2 objects read in place from a decrypted package (see `ue2-package`).
//!
//! Nothing is converted or rewritten: parsed objects borrow the original bytes, and
//! pixels are decoded to RGBA only when a caller asks for them.

mod texture;

use std::fmt;

use ue2_core::{ReadError, Reader};
use ue2_package::Package;

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
    MipTooShort { needed: usize, actual: usize },
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

const BOOL: u8 = 3;
const STRUCT: u8 = 10;

/// One tagged property: its name and raw value bytes (empty for booleans).
struct Property<'a> {
    name: &'a str,
    value: &'a [u8],
}

/// Reads the tagged property list that starts every object, up to `None`.
fn read_properties<'a>(reader: &mut Reader<'a>, package: &'a Package) -> Result<Vec<Property<'a>>, Error> {
    let mut properties = Vec::new();
    loop {
        let name = package.name_at(reader.compact()?)?;
        if name.eq_ignore_ascii_case("None") {
            return Ok(properties);
        }
        let info = reader.u8()?;
        let kind = info & 0x0F;
        if kind == 0 {
            return Err(Error::UnknownPropertyType(kind));
        }
        if kind == STRUCT {
            package.name_at(reader.compact()?)?;
        }
        let size = match (info >> 4) & 0x07 {
            0 => 1,
            1 => 2,
            2 => 4,
            3 => 12,
            4 => 16,
            5 => usize::from(reader.u8()?),
            6 => usize::from(reader.u16()?),
            _ => reader.u32()? as usize,
        };
        // For booleans the high bit is the value, not an array flag.
        if info & 0x80 != 0 && kind != BOOL {
            skip_array_index(reader)?;
        }
        let value = if kind == BOOL { &[][..] } else { reader.bytes(size)? };
        properties.push(Property { name, value });
    }
}

/// One byte, or two when the top bit is set, or four when the top two bits are set.
fn skip_array_index(reader: &mut Reader<'_>) -> Result<(), ReadError> {
    let first = reader.u8()?;
    let extra = match first {
        0..0x80 => 0,
        0x80..0xC0 => 1,
        _ => 3,
    };
    reader.bytes(extra).map(drop)
}
