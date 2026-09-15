use std::{fmt, io};

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    /// The handshake failed or a frame did not authenticate.
    Noise(snow::Error),
    /// A frame decrypted but did not hold the expected message.
    Decode(postcard::Error),
    /// A frame's length is over the limit.
    TooLarge(usize),
    /// The peer took longer than the handshake timeout.
    Timeout,
    /// A key is not 32 bytes of hex.
    BadKey,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "connection: {error}"),
            Self::Noise(error) => write!(f, "encryption: {error}"),
            Self::Decode(error) => write!(f, "unexpected message: {error}"),
            Self::TooLarge(len) => write!(f, "frame of {len} bytes is over the limit"),
            Self::Timeout => f.write_str("the handshake timed out"),
            Self::BadKey => f.write_str("a key must be 64 hex digits"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<snow::Error> for Error {
    fn from(error: snow::Error) -> Self {
        Self::Noise(error)
    }
}
