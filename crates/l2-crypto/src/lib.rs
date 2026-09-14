//! Decryption of Lineage 2 client files protected by a `Lineage2VerXXX` header.
//!
//! High Five ships three schemes: `Ver111` and `Ver121` (single-byte XOR) and
//! `Ver413` (RSA-1024 blocks over a zlib stream). Pure: bytes in, bytes out.

mod ver413;

use std::borrow::Cow;
use std::fmt;
use std::path::Path;

/// `Lineage2VerXXX` as UTF-16LE.
const HEADER_LEN: usize = 28;
/// Optional footer `[0, ?, ?, crc32, 0]` as little-endian u32s. On XOR files the CRC
/// covers header + payload; on `Ver413` files it does not reliably, so block alignment decides.
const TRAILER_LEN: usize = 20;
const MAGIC: &str = "Lineage2Ver";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scheme {
    /// No `Lineage2Ver` header; the file is used as-is.
    Plain,
    Ver111,
    Ver121,
    Ver413,
}

#[derive(Debug)]
pub enum Error {
    UnsupportedVersion(u16),
    Truncated,
    MisalignedRsaPayload(usize),
    NoMatchingRsaKey,
    InvalidRsaBlock(usize),
    Inflate(std::io::Error),
    SizeMismatch { declared: usize, actual: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(v) => write!(f, "unsupported Lineage2Ver{v}"),
            Self::Truncated => f.write_str("file is truncated"),
            Self::MisalignedRsaPayload(len) => write!(f, "RSA payload of {len} bytes is not a multiple of 128"),
            Self::NoMatchingRsaKey => f.write_str("no known Ver413 RSA key opens this file"),
            Self::InvalidRsaBlock(i) => write!(f, "RSA block {i} is malformed"),
            Self::Inflate(e) => write!(f, "zlib stream is corrupt: {e}"),
            Self::SizeMismatch { declared, actual } => {
                write!(f, "declared {declared} decompressed bytes, got {actual}")
            }
        }
    }
}

impl std::error::Error for Error {}

/// Identifies the protection scheme from the file header.
pub fn detect(data: &[u8]) -> Result<Scheme, Error> {
    match header_version(data) {
        None => Ok(Scheme::Plain),
        Some(111) => Ok(Scheme::Ver111),
        Some(121) => Ok(Scheme::Ver121),
        Some(413) => Ok(Scheme::Ver413),
        Some(other) => Err(Error::UnsupportedVersion(other)),
    }
}

/// Decrypts a client file. `path` is only used for its file name, which keys `Ver121`.
pub fn decrypt<'a>(data: &'a [u8], path: &Path) -> Result<Cow<'a, [u8]>, Error> {
    Ok(Cow::Owned(match detect(data)? {
        Scheme::Plain => return Ok(Cow::Borrowed(data)),
        Scheme::Ver111 => xor(xor_payload(data)?, 0xAC),
        Scheme::Ver121 => xor(xor_payload(data)?, ver121_key(path)),
        Scheme::Ver413 => ver413::decrypt(data.get(HEADER_LEN..).ok_or(Error::Truncated)?)?,
    }))
}

/// Official XOR files end with a CRC-signed trailer; some custom encoders omit it,
/// so it is only stripped when its checksum matches.
fn xor_payload(data: &[u8]) -> Result<&[u8], Error> {
    let body = data.get(HEADER_LEN..).ok_or(Error::Truncated)?;
    let Some((signed, trailer)) = data.split_last_chunk::<TRAILER_LEN>() else {
        return Ok(body);
    };
    let [.., c0, c1, c2, c3, _, _, _, _] = *trailer;
    let crc = u32::from_le_bytes([c0, c1, c2, c3]);
    let mut hasher = flate2::Crc::new();
    hasher.update(signed);
    match signed.get(HEADER_LEN..) {
        Some(payload) if hasher.sum() == crc => Ok(payload),
        _ => Ok(body),
    }
}

fn header_version(data: &[u8]) -> Option<u16> {
    let units = data.get(..HEADER_LEN)?.as_chunks::<2>().0.iter().map(|&unit| u16::from_le_bytes(unit));
    let text: String = char::decode_utf16(units).collect::<Result<_, _>>().ok()?;
    text.strip_prefix(MAGIC)?.parse().ok()
}

/// Low byte of the sum of the lower-cased file name's UTF-16 units.
fn ver121_key(path: &Path) -> u8 {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let [low, ..] = name.encode_utf16().fold(0u32, |sum, unit| sum.wrapping_add(unit.into())).to_le_bytes();
    low
}

fn xor(payload: &[u8], key: u8) -> Vec<u8> {
    payload.iter().map(|b| b ^ key).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(version: u16, key: u8, plain: &[u8], with_trailer: bool) -> Vec<u8> {
        let mut file: Vec<u8> = format!("{MAGIC}{version}").encode_utf16().flat_map(u16::to_le_bytes).collect();
        file.extend(xor(plain, key));
        if with_trailer {
            let mut hasher = flate2::Crc::new();
            hasher.update(&file);
            file.extend([0; 12]);
            file.extend(hasher.sum().to_le_bytes());
            file.extend([0; 4]);
        }
        file
    }

    #[test]
    fn ver121_keys_match_real_client_files() {
        assert_eq!(ver121_key(Path::new("SysTextures/BaseballTex.utx")), 0x16);
        assert_eq!(ver121_key(Path::new("ui.ugx")), 0x60);
    }

    #[test]
    fn xor_schemes_round_trip() {
        let plain = b"\xC1\x83\x2A\x9E package with enough bytes to span a trailer";
        let path = Path::new("BaseballTex.utx");
        for with_trailer in [true, false] {
            assert_eq!(*decrypt(&encode(111, 0xAC, plain, with_trailer), path).unwrap(), *plain);
            assert_eq!(*decrypt(&encode(121, 0x16, plain, with_trailer), path).unwrap(), *plain);
        }
    }

    #[test]
    fn headerless_files_are_borrowed() {
        assert!(matches!(decrypt(b"OggS", Path::new("a.ogg")).unwrap(), Cow::Borrowed(_)));
        assert!(matches!(detect(&encode(212, 0, b"", true)), Err(Error::UnsupportedVersion(212))));
    }
}
