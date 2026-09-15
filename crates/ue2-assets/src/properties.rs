//! The tagged property list that starts every serialized object.

use ue2_core::Reader;
use ue2_package::{Export, ObjectRef, Package};

use crate::Error;

const BYTE: u8 = 1;
const INT: u8 = 2;
const BOOL: u8 = 3;
const FLOAT: u8 = 4;
const OBJECT: u8 = 5;
const NAME: u8 = 6;
const CLASS: u8 = 8;
const ARRAY: u8 = 9;
const STRUCT: u8 = 10;

/// Objects that carry a script state frame before their properties.
const HAS_STACK: u32 = 0x0200_0000;

/// One tagged property. The value stays raw; accessors read it when its type matches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Property<'a> {
    pub name: &'a str,
    kind: u8,
    struct_name: Option<&'a str>,
    value: &'a [u8],
    /// Booleans keep their value in the tag, not in `value`.
    flag: bool,
}

impl<'a> Property<'a> {
    pub fn byte(&self) -> Option<u8> {
        (self.kind == BYTE).then(|| self.value.first().copied()).flatten()
    }

    pub fn int(&self) -> Option<i32> {
        (self.kind == INT).then(|| Reader::at(self.value, 0).i32().ok()).flatten()
    }

    pub fn bool(&self) -> Option<bool> {
        (self.kind == BOOL).then_some(self.flag)
    }

    pub fn float(&self) -> Option<f32> {
        (self.kind == FLOAT).then(|| Reader::at(self.value, 0).f32().ok()).flatten()
    }

    /// An object or class reference.
    pub fn object(&self, package: &Package) -> Option<ObjectRef> {
        matches!(self.kind, OBJECT | CLASS).then(|| object(package, &mut Reader::at(self.value, 0))).flatten()
    }

    pub fn name_value(&self, package: &'a Package) -> Option<&'a str> {
        (self.kind == NAME).then(|| package.name_at(Reader::at(self.value, 0).compact().ok()?).ok()).flatten()
    }

    pub fn vector(&self) -> Option<[f32; 3]> {
        let mut reader = self.struct_value("Vector")?;
        Some([reader.f32().ok()?, reader.f32().ok()?, reader.f32().ok()?])
    }

    /// A `Color` struct as RGBA; Unreal stores it blue first.
    pub fn color(&self) -> Option<[u8; 4]> {
        match (self.kind, self.struct_name, self.value) {
            (STRUCT, Some("Color"), &[b, g, r, a]) => Some([r, g, b, a]),
            _ => None,
        }
    }

    /// Pitch, yaw and roll in Unreal units (65536 to a full turn).
    pub fn rotator(&self) -> Option<[i32; 3]> {
        let mut reader = self.struct_value("Rotator")?;
        Some([reader.i32().ok()?, reader.i32().ok()?, reader.i32().ok()?])
    }

    /// A dynamic array of structs, each a property list of its own.
    pub fn structs(&self, package: &'a Package) -> Option<Vec<Vec<Property<'a>>>> {
        if self.kind != ARRAY {
            return None;
        }
        let mut reader = Reader::at(self.value, 0);
        let count = usize::try_from(reader.compact().ok()?).ok()?;
        let structs = (0..count).map(|_| read_properties(&mut reader, package).ok()).collect::<Option<Vec<_>>>()?;
        reader.remaining().is_empty().then_some(structs)
    }

    /// A dynamic array of object references; `None` when the bytes are not exactly that.
    pub fn objects(&self, package: &Package) -> Option<Vec<ObjectRef>> {
        if self.kind != ARRAY {
            return None;
        }
        let mut reader = Reader::at(self.value, 0);
        let count = usize::try_from(reader.compact().ok()?).ok()?;
        let objects = (0..count).map(|_| object(package, &mut reader)).collect::<Option<Vec<_>>>()?;
        reader.remaining().is_empty().then_some(objects)
    }

    fn struct_value(&self, name: &str) -> Option<Reader<'a>> {
        (self.kind == STRUCT && self.struct_name == Some(name) && self.value.len() == 12)
            .then(|| Reader::at(self.value, 0))
    }
}

fn object(package: &Package, reader: &mut Reader<'_>) -> Option<ObjectRef> {
    package.object_at(reader.compact().ok()?).ok()
}

/// Properties of `export`, past the state frame objects with script state carry.
pub fn object_properties<'a>(
    package: &'a Package,
    file: &'a [u8],
    export: &Export,
) -> Result<Vec<Property<'a>>, Error> {
    object_properties_reader(package, file, export).map(|(properties, _)| properties)
}

/// Properties of `export` and a reader just past them, bounded by the end of the object.
pub(crate) fn object_properties_reader<'a>(
    package: &'a Package,
    file: &'a [u8],
    export: &Export,
) -> Result<(Vec<Property<'a>>, Reader<'a>), Error> {
    let end = export.serial_offset.checked_add(export.serial_size).ok_or(Error::ExportOutOfRange)?;
    let object = file.get(..end).ok_or(Error::ExportOutOfRange)?;
    let mut reader = Reader::at(object, export.serial_offset);
    if export.flags & HAS_STACK != 0 {
        let node = reader.compact()?;
        reader.compact()?; // state node
        reader.bytes(8)?; // probe mask
        reader.u32()?; // latent action
        if node != 0 {
            reader.compact()?; // code offset
        }
    }
    let properties = read_properties(&mut reader, package)?;
    Ok((properties, reader))
}

/// Reads the tagged property list at the reader, up to `None`.
pub(crate) fn read_properties<'a>(reader: &mut Reader<'a>, package: &'a Package) -> Result<Vec<Property<'a>>, Error> {
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
        let struct_name = if kind == STRUCT { Some(package.name_at(reader.compact()?)?) } else { None };
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
        // ponytail: fixed-array element indices are skipped; keep them when a reader needs one.
        let flag = info & 0x80 != 0;
        if flag && kind != BOOL {
            skip_array_index(reader)?;
        }
        let value = if kind == BOOL { &[][..] } else { reader.bytes(size)? };
        properties.push(Property { name, kind, struct_name, value, flag });
    }
}

/// One byte, or two when the top bit is set, or four when the top two bits are set.
fn skip_array_index(reader: &mut Reader<'_>) -> Result<(), Error> {
    let extra = match reader.u8()? {
        0..0x80 => 0,
        0x80..0xC0 => 1,
        _ => 3,
    };
    reader.bytes(extra)?;
    Ok(())
}

/// The first property called `name`, ignoring case as Unreal names do.
pub fn find<'p, 'a>(properties: &'p [Property<'a>], name: &str) -> Option<&'p Property<'a>> {
    properties.iter().find(|property| property.name.eq_ignore_ascii_case(name))
}
