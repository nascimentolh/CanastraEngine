//! Frames on the wire: a little-endian `u32` length, then that many bytes.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::Error;

/// The largest frame: a Noise message is at most 65535 bytes.
pub(crate) const MAX_FRAME: usize = 65535;

pub(crate) async fn write<S: AsyncWrite + Unpin>(stream: &mut S, bytes: &[u8]) -> Result<(), Error> {
    let len =
        u32::try_from(bytes.len()).ok().filter(|&len| len as usize <= MAX_FRAME).ok_or(Error::TooLarge(bytes.len()))?;
    stream.write_all(&len.to_le_bytes()).await?;
    stream.write_all(bytes).await?;
    stream.flush().await?;
    Ok(())
}

/// Reads one frame; a length over the limit fails before anything is allocated for it.
pub(crate) async fn read<S: AsyncRead + Unpin>(stream: &mut S) -> Result<Vec<u8>, Error> {
    let len = stream.read_u32_le().await? as usize;
    if len > MAX_FRAME {
        return Err(Error::TooLarge(len));
    }
    let mut bytes = vec![0; len];
    stream.read_exact(&mut bytes).await?;
    Ok(bytes)
}
