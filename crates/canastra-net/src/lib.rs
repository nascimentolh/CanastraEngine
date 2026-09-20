//! Carries Canastra protocol messages over any byte stream: a Noise handshake that authenticates the
//! server by its pinned static key, then length-prefixed frames sealed with ChaCha20-Poly1305. Also
//! signs and checks the login server's admission tickets.

mod connection;
mod error;
mod frame;
mod keys;
pub mod ticket;

pub use connection::{Connection, HANDSHAKE_TIMEOUT, Pattern, Reader, Writer};
pub use error::Error;
pub use keys::{Keypair, parse_key};
