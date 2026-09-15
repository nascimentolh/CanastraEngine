//! Texture inspection: decode in memory, write a PNG only when asked.

use std::fs;
use std::io::Write;
use std::path::Path;

use flate2::Crc;
use flate2::write::ZlibEncoder;
use ue2_package::{ObjectRef, Package};

use crate::{Result, read_decrypted};

/// Decodes every texture in a package; returns how many decoded and a message per failure.
pub(crate) fn export(package_path: &Path, object: &str, output: &Path) -> Result {
    let (_, file) = read_decrypted(package_path)?;
    let package = Package::parse(&file)?;
    let index = (0..package.exports().len())
        .find(|&index| package.object_path(ObjectRef::Export(index)).eq_ignore_ascii_case(object))
        .ok_or_else(|| format!("no object `{object}` in {}", package_path.display()))?;
    let image = ue2_assets::decode_texture(&package, &file, index)?;
    fs::write(output, png(image.width, image.height, &image.rgba)?)?;
    println!("wrote {} ({}x{})", output.display(), image.width, image.height);
    Ok(())
}

/// Minimal RGBA8 PNG: one IDAT, no filtering.
fn png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let row = width as usize * 4;
    if row == 0 || rgba.len() != row * height as usize {
        return Err("image size does not match its pixels".into());
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    for line in rgba.chunks_exact(row) {
        encoder.write_all(&[0])?;
        encoder.write_all(line)?;
    }
    let mut header = [0u8; 13];
    header[..4].copy_from_slice(&width.to_be_bytes());
    header[4..8].copy_from_slice(&height.to_be_bytes());
    header[8..10].copy_from_slice(&[8, 6]); // 8-bit RGBA

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, data) in [(b"IHDR", header.to_vec()), (b"IDAT", encoder.finish()?), (b"IEND", Vec::new())] {
        out.extend(u32::try_from(data.len())?.to_be_bytes());
        let mut crc = Crc::new();
        crc.update(kind);
        crc.update(&data);
        out.extend_from_slice(kind);
        out.extend(data);
        out.extend(crc.sum().to_be_bytes());
    }
    Ok(out)
}
