//! `Texture` and `Palette` objects, and on-demand decoding of their pixels.

use ue2_core::Reader;
use ue2_package::{Export, ObjectRef, Package};

use crate::{Error, Property, read_properties};

/// A 4096-pixel texture has 13 mips; anything above this is not a mip count.
const MAX_MIPS: i32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    P8,
    Rgba7,
    Rgb16,
    Dxt1,
    Rgb8,
    Rgba8,
    NoData,
    Dxt3,
    Dxt5,
    L8,
    G16,
    Rrrgggbbb,
}

impl TextureFormat {
    fn from_byte(byte: u8) -> Result<Self, Error> {
        use TextureFormat::{Dxt1, Dxt3, Dxt5, G16, L8, NoData, P8, Rgb8, Rgb16, Rgba7, Rgba8, Rrrgggbbb};
        const ALL: [TextureFormat; 12] = [P8, Rgba7, Rgb16, Dxt1, Rgb8, Rgba8, NoData, Dxt3, Dxt5, L8, G16, Rrrgggbbb];
        ALL.get(usize::from(byte)).copied().ok_or(Error::UnknownTextureFormat(byte))
    }
}

/// A texture as stored: every mip borrows the package bytes untouched.
#[derive(Debug, Clone)]
pub struct Texture<'a> {
    pub format: TextureFormat,
    pub palette: ObjectRef,
    pub mips: Vec<Mip<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct Mip<'a> {
    pub width: u32,
    pub height: u32,
    pub data: &'a [u8],
}

pub fn read_texture<'a>(package: &'a Package, file: &'a [u8], export: &Export) -> Result<Texture<'a>, Error> {
    let object = object_bytes(file, export)?;
    let mut reader = Reader::at(object, export.serial_offset);
    let properties = read_properties(&mut reader, package)?;
    let byte = |name: &str| find(&properties, name).and_then(|value| value.first().copied());
    let format = TextureFormat::from_byte(byte("Format").unwrap_or(0))?;
    let palette = match find(&properties, "Palette") {
        Some(value) => package.object_at(Reader::at(value, 0).compact()?)?,
        None => ObjectRef::Null,
    };

    // Lineage 2 puts a material block of its own between the properties and the mips, and its
    // layout changes between licensees. Its contents (object name, fixed-function shader) are not
    // needed, so instead of guessing its grammar the mip array is found by its own structure.
    let mips = (reader.pos()..object.len()).find_map(|start| mips_at(object, start).ok()).ok_or(Error::NoMipArray)?;
    Ok(Texture { format, palette, mips })
}

/// A mip array at `start`: every skip offset must point just past its data and the array must end
/// exactly where the object ends, which a wrong start cannot satisfy by chance.
fn mips_at(object: &[u8], start: usize) -> Result<Vec<Mip<'_>>, Error> {
    let mut reader = Reader::at(object, start);
    let count = reader.compact()?;
    if !(1..=MAX_MIPS).contains(&count) {
        return Err(Error::NoMipArray);
    }
    let mut mips = Vec::new();
    for _ in 0..count {
        let skip_to = reader.u32()? as usize;
        let len = usize::try_from(reader.compact()?).map_err(|_| Error::NoMipArray)?;
        let data = reader.bytes(len)?;
        if skip_to != reader.pos() {
            return Err(Error::NoMipArray);
        }
        let (width, height) = (reader.u32()?, reader.u32()?);
        reader.bytes(2)?; // log2 of width and height
        mips.push(Mip { width, height, data });
    }
    finish(&reader)?;
    Ok(mips)
}

/// A decoded image, RGBA8 rows from the top.
#[derive(Debug, Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Reads the texture export at `index`, resolves its palette and decodes the top mip.
pub fn decode_texture(package: &Package, file: &[u8], index: usize) -> Result<Image, Error> {
    let exports = package.exports();
    let texture = read_texture(package, file, exports.get(index).ok_or(Error::ExportOutOfRange)?)?;
    let palette = match texture.palette {
        ObjectRef::Null => None,
        ObjectRef::Export(palette) => {
            Some(read_palette(package, file, exports.get(palette).ok_or(Error::MissingPalette)?)?)
        }
        ObjectRef::Import(_) => return Err(Error::MissingPalette),
    };
    let mip = texture.mips.first().ok_or(Error::NoMipArray)?;
    let rgba = decode_rgba(texture.format, mip, palette.as_deref())?;
    Ok(Image { width: mip.width, height: mip.height, rgba })
}

/// Palette colors as RGBA.
pub fn read_palette(package: &Package, file: &[u8], export: &Export) -> Result<Vec<[u8; 4]>, Error> {
    let mut reader = Reader::at(object_bytes(file, export)?, export.serial_offset);
    read_properties(&mut reader, package)?;
    let count = usize::try_from(reader.compact()?).unwrap_or(0);
    let colors = reader.bytes(count.checked_mul(4).ok_or(Error::MissingPalette)?)?;
    finish(&reader)?;
    Ok(colors.as_chunks::<4>().0.iter().map(|&[b, g, r, a]| [r, g, b, a]).collect())
}

/// Decodes one mip to RGBA8, row by row from the top.
pub fn decode_rgba(format: TextureFormat, mip: &Mip<'_>, palette: Option<&[[u8; 4]]>) -> Result<Vec<u8>, Error> {
    let pixels = mip.width as usize * mip.height as usize;
    let exact = |bytes_per_pixel: usize| -> Result<&[u8], Error> {
        let needed = pixels * bytes_per_pixel;
        mip.data.get(..needed).ok_or(Error::MipTooShort { needed, actual: mip.data.len() })
    };
    Ok(match format {
        TextureFormat::Rgba8 => exact(4)?.as_chunks::<4>().0.iter().flat_map(|&[b, g, r, a]| [r, g, b, a]).collect(),
        TextureFormat::P8 => {
            let palette = palette.ok_or(Error::MissingPalette)?;
            exact(1)?.iter().flat_map(|&index| palette.get(usize::from(index)).copied().unwrap_or_default()).collect()
        }
        // Heightmaps: 16-bit gray, previewed through the high byte.
        TextureFormat::G16 => {
            exact(2)?.as_chunks::<2>().0.iter().flat_map(|&[_, high]| [high, high, high, 255]).collect()
        }
        TextureFormat::Dxt1 | TextureFormat::Dxt3 | TextureFormat::Dxt5 => decode_dxt(format, mip)?,
        other => return Err(Error::UnsupportedFormat(other)),
    })
}

/// The file up to the end of the export, so reads stay inside the object while positions stay
/// absolute (mip skip offsets are absolute).
fn object_bytes<'a>(file: &'a [u8], export: &Export) -> Result<&'a [u8], Error> {
    let end = export.serial_offset.checked_add(export.serial_size).ok_or(Error::ExportOutOfRange)?;
    file.get(..end).ok_or(Error::ExportOutOfRange)
}

fn find<'a>(properties: &[Property<'a>], name: &str) -> Option<&'a [u8]> {
    properties.iter().find(|property| property.name.eq_ignore_ascii_case(name)).map(|property| property.value)
}

fn finish(reader: &Reader<'_>) -> Result<(), Error> {
    match reader.remaining().len() {
        0 => Ok(()),
        len => Err(Error::TrailingBytes(len)),
    }
}

/// BC1/BC2/BC3 block decoding into RGBA8.
fn decode_dxt(format: TextureFormat, mip: &Mip<'_>) -> Result<Vec<u8>, Error> {
    let (width, height) = (mip.width as usize, mip.height as usize);
    let (blocks_wide, blocks_high) = (width.div_ceil(4), height.div_ceil(4));
    let block_len = if format == TextureFormat::Dxt1 { 8 } else { 16 };
    let needed = blocks_wide * blocks_high * block_len;
    let data = mip.data.get(..needed).ok_or(Error::MipTooShort { needed, actual: mip.data.len() })?;

    let mut out = vec![0u8; width * height * 4];
    for (index, block) in data.chunks_exact(block_len).enumerate() {
        let (alpha, color) = block.split_at(block_len - 8);
        let texels = decode_block(format, alpha, color);
        let (block_x, block_y) = (index % blocks_wide * 4, index / blocks_wide * 4);
        for (texel, rgba) in texels.iter().enumerate() {
            let (x, y) = (block_x + texel % 4, block_y + texel / 4);
            if x < width
                && let Some(pixel) = out.get_mut((y * width + x) * 4..(y * width + x) * 4 + 4)
            {
                pixel.copy_from_slice(rgba);
            }
        }
    }
    Ok(out)
}

fn decode_block(format: TextureFormat, alpha: &[u8], color: &[u8]) -> [[u8; 4]; 16] {
    let word = |bytes: &[u8], at: usize| u16::from_le_bytes([byte(bytes, at), byte(bytes, at + 1)]);
    let (c0, c1) = (word(color, 0), word(color, 2));
    let (p0, p1) = (rgb565(c0), rgb565(c1));
    let mix = |a: u8, b: u8, wa: u16, wb: u16| narrow((u16::from(a) * wa + u16::from(b) * wb) / (wa + wb));
    let blend =
        |wa: u16, wb: u16| [mix(p0[0], p1[0], wa, wb), mix(p0[1], p1[1], wa, wb), mix(p0[2], p1[2], wa, wb), 255];
    let palette = if format == TextureFormat::Dxt1 && c0 <= c1 {
        [p0, p1, blend(1, 1), [0, 0, 0, 0]]
    } else {
        [p0, p1, blend(2, 1), blend(1, 2)]
    };
    let indices = u32::from_le_bytes([byte(color, 4), byte(color, 5), byte(color, 6), byte(color, 7)]);

    let mut texels = [[0u8; 4]; 16];
    for (i, texel) in texels.iter_mut().enumerate() {
        *texel = palette.get(((indices >> (2 * i)) & 3) as usize).copied().unwrap_or_default();
        match format {
            TextureFormat::Dxt3 => texel[3] = (byte(alpha, i / 2) >> (4 * (i % 2)) & 0x0F) * 17,
            TextureFormat::Dxt5 => texel[3] = dxt5_alpha(alpha, i),
            _ => {}
        }
    }
    texels
}

fn dxt5_alpha(alpha: &[u8], texel: usize) -> u8 {
    let (a0, a1) = (u16::from(byte(alpha, 0)), u16::from(byte(alpha, 1)));
    let bits = alpha.get(2..8).map_or(0, |b| b.iter().rev().fold(0u64, |acc, &x| acc << 8 | u64::from(x)));
    let code = ((bits >> (3 * texel)) & 7) as u16;
    let value = match (code, a0 > a1) {
        (0, _) => a0,
        (1, _) => a1,
        (c, true) => ((8 - c) * a0 + (c - 1) * a1) / 7,
        (6, false) => 0,
        (7, false) => 255,
        (c, false) => ((6 - c) * a0 + (c - 1) * a1) / 5,
    };
    narrow(value)
}

fn rgb565(value: u16) -> [u8; 4] {
    let scale = |bits: u16, max: u16| narrow((bits * 255 + max / 2) / max);
    [scale(value >> 11 & 31, 31), scale(value >> 5 & 63, 63), scale(value & 31, 31), 255]
}

/// Weighted averages and scaled channels of bytes stay within a byte.
fn narrow(value: u16) -> u8 {
    u8::try_from(value).unwrap_or(u8::MAX)
}

fn byte(bytes: &[u8], at: usize) -> u8 {
    bytes.get(at).copied().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dxt1_block_with_transparency() {
        // c0 = pure red, c1 = pure blue, c0 <= c1 is false so four opaque colors.
        let mut block = [0u8; 8];
        block[0..2].copy_from_slice(&0xF800u16.to_le_bytes());
        block[2..4].copy_from_slice(&0x001Fu16.to_le_bytes());
        block[4..8].copy_from_slice(&0b01_00u32.to_le_bytes()); // texel 1 uses c1
        let mip = Mip { width: 2, height: 1, data: &block };
        assert_eq!(decode_rgba(TextureFormat::Dxt1, &mip, None).unwrap(), [255, 0, 0, 255, 0, 0, 255, 255]);

        // Swapped endpoints select the three-color mode with transparent index 3.
        let mut block = [0u8; 8];
        block[0..2].copy_from_slice(&0x001Fu16.to_le_bytes());
        block[2..4].copy_from_slice(&0xF800u16.to_le_bytes());
        block[4..8].copy_from_slice(&3u32.to_le_bytes());
        let mip = Mip { width: 1, height: 1, data: &block };
        assert_eq!(decode_rgba(TextureFormat::Dxt1, &mip, None).unwrap(), [0, 0, 0, 0]);
    }

    #[test]
    fn dxt5_interpolates_alpha() {
        let mut block = [0u8; 16];
        (block[0], block[1]) = (255, 0);
        block[2] = 2; // texel 0 uses code 2: (6 * 255 + 1 * 0) / 7
        let mip = Mip { width: 1, height: 1, data: &block };
        assert_eq!(decode_rgba(TextureFormat::Dxt5, &mip, None).unwrap()[3], 218);
    }

    #[test]
    fn swaps_bgra_and_applies_palettes() {
        let mip = Mip { width: 1, height: 1, data: &[1, 2, 3, 4] };
        assert_eq!(decode_rgba(TextureFormat::Rgba8, &mip, None).unwrap(), [3, 2, 1, 4]);
        let mip = Mip { width: 2, height: 1, data: &[1, 0] };
        let palette = [[9, 9, 9, 9], [1, 2, 3, 4]];
        assert_eq!(decode_rgba(TextureFormat::P8, &mip, Some(&palette)).unwrap(), [1, 2, 3, 4, 9, 9, 9, 9]);
        assert!(matches!(decode_rgba(TextureFormat::P8, &mip, None), Err(Error::MissingPalette)));
        assert!(matches!(decode_rgba(TextureFormat::Rgba8, &mip, None), Err(Error::MipTooShort { .. })));
    }
}
