use std::io::Read;

use flate2::read::ZlibDecoder;
use num_bigint::BigUint;

use crate::{Error, TRAILER_LEN};

const BLOCK_LEN: usize = 128;
const MAX_BLOCK_DATA: usize = 124;

/// Public half of a `Ver413` RSA key.
#[derive(Debug, Clone, Copy)]
pub struct RsaKey {
    pub name: &'static str,
    pub modulus_hex: &'static str,
    pub exponent: u32,
}

/// Tried in order; the first key that opens the first block wins.
pub const KNOWN_413_KEYS: [RsaKey; 2] = [
    RsaKey {
        name: "ncsoft",
        modulus_hex: "97df398472ddf737ef0a0cd17e8d172f0fef1661a38a8ae1d6e829bc1c6e4c3cfc19292dda9ef90175e46e7394a18850b6417d03be6eea274d3ed1dde5b5d7bde72cc0a0b71d03608655633881793a02c9a67d9ef2b45eb7c08d4be329083ce450e68f7867b6749314d40511d09bc5744551baa86a89dc38123dc1668fd72d83",
        exponent: 0x35,
    },
    RsaKey {
        name: "l2encdec",
        modulus_hex: "75b4d6de5c016544068a1acf125869f43d2e09fc55b8b1e289556daf9b8757635593446288b3653da1ce91c87bb1a5c18f16323495c55d7d72c0890a83f69bfd1fd9434eb1c02f3e4679edfa43309319070129c267c85604d87bb65bae205de3707af1d2108881abb567c3b3d069ae67c3a4c6a3aa93d26413d4c66094ae2039",
        exponent: 0x1d,
    },
];

/// Everything after the header: RSA blocks that concatenate to
/// `u32 decompressed size` + zlib stream, then an optional trailer.
pub(crate) fn decrypt(body: &[u8]) -> Result<Vec<u8>, Error> {
    let payload = match body.len() % BLOCK_LEN {
        0 => body,
        TRAILER_LEN => body.split_last_chunk::<TRAILER_LEN>().map_or(body, |(payload, _)| payload),
        _ => return Err(Error::MisalignedRsaPayload(body.len())),
    };
    let first = payload.get(..BLOCK_LEN).ok_or(Error::Truncated)?;
    let (modulus, exponent) = KNOWN_413_KEYS
        .iter()
        .map(|key| {
            let modulus = BigUint::parse_bytes(key.modulus_hex.as_bytes(), 16).expect("key modulus is valid hex");
            (modulus, BigUint::from(key.exponent))
        })
        .find(|(modulus, exponent)| open_block(first, modulus, exponent).is_some())
        .ok_or(Error::NoMatchingRsaKey)?;

    let mut compressed = Vec::with_capacity(payload.len());
    for (index, block) in payload.as_chunks::<BLOCK_LEN>().0.iter().enumerate() {
        let data = open_block(block, &modulus, &exponent).ok_or(Error::InvalidRsaBlock(index))?;
        compressed.extend_from_slice(&data);
    }

    let (size, stream) = compressed.split_first_chunk::<4>().ok_or(Error::Truncated)?;
    let declared = u32::from_le_bytes(*size) as usize;
    let mut plain = Vec::new();
    ZlibDecoder::new(stream).read_to_end(&mut plain).map_err(Error::Inflate)?;
    if plain.len() != declared {
        return Err(Error::SizeMismatch { declared, actual: plain.len() });
    }
    Ok(plain)
}

/// A block decrypts to `[0, 0, 0, size, ...]` with `size` data bytes right-aligned
/// to a 4-byte boundary inside the last 124 bytes.
fn open_block(block: &[u8], modulus: &BigUint, exponent: &BigUint) -> Option<Vec<u8>> {
    let value = BigUint::from_bytes_be(block).modpow(exponent, modulus).to_bytes_be();
    let mut plain = [0u8; BLOCK_LEN];
    plain.get_mut(BLOCK_LEN.checked_sub(value.len())?..)?.copy_from_slice(&value);
    let [0, 0, 0, size, ..] = plain else {
        return None;
    };
    let size = usize::from(size);
    let start = BLOCK_LEN.checked_sub(size + MAX_BLOCK_DATA.checked_sub(size)? % 4)?;
    plain.get(start..start + size).map(<[u8]>::to_vec)
}
