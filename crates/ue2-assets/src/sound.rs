//! `Sound` objects: a format name, then the file itself, which the client stores as RIFF WAV.

use ue2_core::Reader;
use ue2_package::{Export, Package};

use crate::{Error, object_data};

/// The bytes of the sound `export`, as the file its format names; `None` where the object holds none.
pub fn read_sound<'a>(package: &'a Package, file: &'a [u8], export: &Export) -> Result<&'a [u8], Error> {
    let (_, data) = object_data(package, file, export)?;
    let mut reader = Reader::at(data, 0);
    reader.compact()?; // format name
    reader.i32()?; // where the sound ends in the file, which the bytes below give anyway
    let len = usize::try_from(reader.compact()?).unwrap_or(0);
    Ok(reader.bytes(len)?)
}
