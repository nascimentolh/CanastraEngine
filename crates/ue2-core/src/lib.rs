//! Bounds-checked little-endian reader shared by every Unreal/Lineage 2 binary format.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    UnexpectedEof(usize),
    BadCompactIndex(usize),
    InvalidString(usize),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof(at) => write!(f, "unexpected end of data at {at:#x}"),
            Self::BadCompactIndex(at) => write!(f, "malformed compact index at {at:#x}"),
            Self::InvalidString(at) => write!(f, "invalid string at {at:#x}"),
        }
    }
}

impl std::error::Error for ReadError {}

pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn at(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> &'a [u8] {
        self.data.get(self.pos..).unwrap_or_default()
    }

    pub fn bytes(&mut self, len: usize) -> Result<&'a [u8], ReadError> {
        let slice = self
            .pos
            .checked_add(len)
            .and_then(|end| self.data.get(self.pos..end))
            .ok_or(ReadError::UnexpectedEof(self.pos))?;
        self.pos += len;
        Ok(slice)
    }

    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], ReadError> {
        let array = *self.remaining().first_chunk::<N>().ok_or(ReadError::UnexpectedEof(self.pos))?;
        self.pos += N;
        Ok(array)
    }

    pub fn u8(&mut self) -> Result<u8, ReadError> {
        Ok(self.array::<1>()?[0])
    }

    pub fn u16(&mut self) -> Result<u16, ReadError> {
        self.array().map(u16::from_le_bytes)
    }

    pub fn u32(&mut self) -> Result<u32, ReadError> {
        self.array().map(u32::from_le_bytes)
    }

    pub fn i32(&mut self) -> Result<i32, ReadError> {
        self.array().map(i32::from_le_bytes)
    }

    pub fn f32(&mut self) -> Result<f32, ReadError> {
        self.array().map(f32::from_le_bytes)
    }

    /// UE2 compact index: first byte is sign, continue flag and 6 bits;
    /// up to four more bytes carry a continue flag and 7 bits each.
    pub fn compact(&mut self) -> Result<i32, ReadError> {
        let start = self.pos;
        let first = self.u8()?;
        let mut value = u64::from(first & 0x3F);
        let mut more = first & 0x40 != 0;
        let mut shift = 6;
        while more {
            if shift > 27 {
                return Err(ReadError::BadCompactIndex(start));
            }
            let byte = self.u8()?;
            value |= u64::from(byte & 0x7F) << shift;
            more = byte & 0x80 != 0;
            shift += 7;
        }
        let value = i32::try_from(value).map_err(|_| ReadError::BadCompactIndex(start))?;
        Ok(if first & 0x80 != 0 { -value } else { value })
    }

    /// `FString`: compact length including the terminator; positive is Windows-1252,
    /// negative is `UTF-16LE`.
    pub fn string(&mut self) -> Result<String, ReadError> {
        let len = self.compact()?;
        let Ok(len) = usize::try_from(len) else {
            return self.utf16((len.unsigned_abs() as usize).saturating_mul(2));
        };
        let bytes = self.bytes(len)?;
        Ok(bytes.strip_suffix(&[0]).unwrap_or(bytes).iter().map(|&b| cp1252(b)).collect())
    }

    /// `len` bytes of `UTF-16LE`, with trailing terminators dropped.
    pub fn utf16(&mut self, len: usize) -> Result<String, ReadError> {
        let start = self.pos;
        let mut units: Vec<u16> =
            self.bytes(len)?.as_chunks::<2>().0.iter().map(|&unit| u16::from_le_bytes(unit)).collect();
        while units.last() == Some(&0) {
            units.pop();
        }
        String::from_utf16(&units).map_err(|_| ReadError::InvalidString(start))
    }
}

/// Windows-1252 differs from Latin-1 only in 0x80..=0x9F; its five undefined
/// bytes keep their Latin-1 code points, as Windows does.
fn cp1252(byte: u8) -> char {
    const HIGH: [char; 32] = [
        '\u{20AC}', '\u{81}', '\u{201A}', '\u{192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}', '\u{2C6}',
        '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{8D}', '\u{17D}', '\u{8F}', '\u{90}', '\u{2018}', '\u{2019}',
        '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}', '\u{2DC}', '\u{2122}', '\u{161}', '\u{203A}',
        '\u{153}', '\u{9D}', '\u{17E}', '\u{178}',
    ];
    byte.checked_sub(0x80).and_then(|i| HIGH.get(usize::from(i))).copied().unwrap_or(char::from(byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compact(bytes: &[u8]) -> Result<i32, ReadError> {
        Reader::at(bytes, 0).compact()
    }

    #[test]
    fn compact_index() {
        assert_eq!(compact(&[0x00]).unwrap(), 0);
        assert_eq!(compact(&[0x3F]).unwrap(), 63);
        assert_eq!(compact(&[0x40, 0x01]).unwrap(), 64);
        assert_eq!(compact(&[0x81]).unwrap(), -1);
        assert_eq!(compact(&[0x7F, 0xFF, 0xFF, 0xFF, 0x0F]).unwrap(), i32::MAX);
        assert!(compact(&[0x40, 0x80, 0x80, 0x80, 0x80, 0x01]).is_err());
        assert!(compact(&[0x40]).is_err());
    }

    #[test]
    fn strings() {
        assert_eq!(Reader::at(b"\x05None\0", 0).string().unwrap(), "None");
        assert_eq!(Reader::at(b"\x03\x85\x99\0", 0).string().unwrap(), "\u{2026}\u{2122}");
        assert_eq!(Reader::at(b"\x83A\0\xE9\0\0\0", 0).string().unwrap(), "A\u{E9}");
        assert_eq!(Reader::at(b"h\0i\0\0\0", 0).utf16(6).unwrap(), "hi");
    }
}
