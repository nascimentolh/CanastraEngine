//! An authenticated, encrypted connection sending and receiving typed messages.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncRead, AsyncWrite, ReadHalf, WriteHalf};

use crate::{Error, Keypair, frame};

/// How long either side waits for the other to complete the handshake.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Room ChaCha20-Poly1305 adds to every sealed frame.
const TAG: usize = 16;

/// Who authenticates whom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    /// A player's client: it knows the server's static key and has none of its own (Noise NK).
    Player,
    /// A peer server: both sides have static keys and the connecting side knows the other's (Noise IK),
    /// so the accepting side learns who connected.
    Peer,
}

impl Pattern {
    fn name(self) -> &'static str {
        match self {
            Self::Player => "Noise_NK_25519_ChaChaPoly_BLAKE2s",
            Self::Peer => "Noise_IK_25519_ChaChaPoly_BLAKE2s",
        }
    }

    pub(crate) fn params(self) -> snow::params::NoiseParams {
        self.name().parse().unwrap_or_else(|_| unreachable!("both pattern names are valid Noise parameters"))
    }
}

pub struct Connection<S> {
    stream: S,
    noise: snow::TransportState,
}

impl<S: AsyncRead + AsyncWrite + Unpin> Connection<S> {
    /// Connects to a server whose static public key is `server`. With `keys`, the peer pattern also
    /// proves this side's identity.
    pub async fn connect(mut stream: S, server: &[u8; 32], keys: Option<&Keypair>) -> Result<Self, Error> {
        let pattern = if keys.is_some() { Pattern::Peer } else { Pattern::Player };
        let mut builder = snow::Builder::new(pattern.params()).remote_public_key(server)?;
        if let Some(keys) = keys {
            builder = builder.local_private_key(&keys.private)?;
        }
        let mut noise = builder.build_initiator()?;
        timed(async {
            let mut buffer = vec![0; frame::MAX_FRAME];
            let len = noise.write_message(&[], &mut buffer)?;
            frame::write(&mut stream, buffer.get(..len).unwrap_or_default()).await?;
            noise.read_message(&frame::read(&mut stream).await?, &mut buffer)?;
            Ok(())
        })
        .await?;
        Ok(Self { stream, noise: noise.into_transport_mode()? })
    }

    /// Accepts a client with this server's `keys`, under `pattern`.
    pub async fn accept(mut stream: S, keys: &Keypair, pattern: Pattern) -> Result<Self, Error> {
        let mut noise = snow::Builder::new(pattern.params()).local_private_key(&keys.private)?.build_responder()?;
        timed(async {
            let mut buffer = vec![0; frame::MAX_FRAME];
            noise.read_message(&frame::read(&mut stream).await?, &mut buffer)?;
            let len = noise.write_message(&[], &mut buffer)?;
            frame::write(&mut stream, buffer.get(..len).unwrap_or_default()).await?;
            Ok(())
        })
        .await?;
        Ok(Self { stream, noise: noise.into_transport_mode()? })
    }

    /// The other side's static public key, under the peer pattern.
    pub fn remote_key(&self) -> Option<[u8; 32]> {
        self.noise.get_remote_static().and_then(|key| key.try_into().ok())
    }

    pub async fn send<T: Serialize>(&mut self, message: &T) -> Result<(), Error> {
        let plain = canastra_protocol::encode(message);
        if plain.len() + TAG > frame::MAX_FRAME {
            return Err(Error::TooLarge(plain.len()));
        }
        let mut sealed = vec![0; plain.len() + TAG];
        let len = self.noise.write_message(&plain, &mut sealed)?;
        frame::write(&mut self.stream, sealed.get(..len).unwrap_or_default()).await
    }

    pub async fn recv<T: DeserializeOwned>(&mut self) -> Result<T, Error> {
        let sealed = frame::read(&mut self.stream).await?;
        let mut plain = vec![0; sealed.len()];
        let len = self.noise.read_message(&sealed, &mut plain)?;
        canastra_protocol::decode(plain.get(..len).unwrap_or_default()).map_err(Error::Decode)
    }

    /// Splits the connection into an end that reads and an end that writes, so one task can wait for
    /// messages while another sends them. Each frame is sealed or opened on its own, and the two ends never
    /// hold the seal across a read or a write.
    pub fn split(self) -> (Reader<S>, Writer<S>) {
        let (reader, writer) = tokio::io::split(self.stream);
        let seal = Arc::new(Mutex::new(self.noise));
        (Reader { stream: reader, seal: seal.clone() }, Writer { stream: writer, seal })
    }
}

/// The end of a connection that reads messages.
pub struct Reader<S> {
    stream: ReadHalf<S>,
    seal: Arc<Mutex<snow::TransportState>>,
}

/// The end of a connection that writes messages.
pub struct Writer<S> {
    stream: WriteHalf<S>,
    seal: Arc<Mutex<snow::TransportState>>,
}

impl<S: AsyncRead + AsyncWrite + Unpin> Reader<S> {
    pub async fn recv<T: DeserializeOwned>(&mut self) -> Result<T, Error> {
        let sealed = frame::read(&mut self.stream).await?;
        let mut plain = vec![0; sealed.len()];
        let len = {
            let mut seal = self.seal.lock().unwrap_or_else(PoisonError::into_inner);
            seal.read_message(&sealed, &mut plain)?
        };
        canastra_protocol::decode(plain.get(..len).unwrap_or_default()).map_err(Error::Decode)
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> Writer<S> {
    pub async fn send<T: Serialize>(&mut self, message: &T) -> Result<(), Error> {
        let plain = canastra_protocol::encode(message);
        if plain.len() + TAG > frame::MAX_FRAME {
            return Err(Error::TooLarge(plain.len()));
        }
        let mut sealed = vec![0; plain.len() + TAG];
        let len = {
            let mut seal = self.seal.lock().unwrap_or_else(PoisonError::into_inner);
            seal.write_message(&plain, &mut sealed)?
        };
        frame::write(&mut self.stream, sealed.get(..len).unwrap_or_default()).await
    }
}

async fn timed(handshake: impl Future<Output = Result<(), Error>>) -> Result<(), Error> {
    tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake).await.map_err(|_| Error::Timeout)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use canastra_protocol::login::LoginClient;

    #[tokio::test]
    async fn players_reach_the_pinned_server_and_exchange_messages() {
        let server_keys = Keypair::generate().unwrap();
        let (client_end, server_end) = tokio::io::duplex(1 << 17);
        let keys = server_keys.clone();
        let server = tokio::spawn(async move {
            let mut connection = Connection::accept(server_end, &keys, Pattern::Player).await.unwrap();
            assert_eq!(connection.remote_key(), None);
            let hello: LoginClient = connection.recv().await.unwrap();
            connection.send(&hello).await.unwrap();
        });
        let mut client = Connection::connect(client_end, &server_keys.public, None).await.unwrap();
        let hello = LoginClient::Hello { version: 1 };
        client.send(&hello).await.unwrap();
        assert_eq!(client.recv::<LoginClient>().await.unwrap(), hello);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn split_ends_send_and_receive_while_the_other_waits() {
        let server_keys = Keypair::generate().unwrap();
        let (client_end, server_end) = tokio::io::duplex(1 << 17);
        let keys = server_keys.clone();
        let server = tokio::spawn(async move {
            let connection = Connection::accept(server_end, &keys, Pattern::Player).await.unwrap();
            let (mut reader, mut writer) = connection.split();
            // The server sends without being asked, then answers what the client sends after that.
            writer.send(&LoginClient::Hello { version: 7 }).await.unwrap();
            let asked: LoginClient = reader.recv().await.unwrap();
            writer.send(&asked).await.unwrap();
        });
        let mut client = Connection::connect(client_end, &server_keys.public, None).await.unwrap();
        assert_eq!(client.recv::<LoginClient>().await.unwrap(), LoginClient::Hello { version: 7 });
        let asked = LoginClient::Authenticate { account: "ana".into(), password: "pw".into() };
        client.send(&asked).await.unwrap();
        assert_eq!(client.recv::<LoginClient>().await.unwrap(), asked);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn a_wrong_pinned_key_fails_the_handshake() {
        let (server_keys, impostor) = (Keypair::generate().unwrap(), Keypair::generate().unwrap());
        let (client_end, server_end) = tokio::io::duplex(1 << 17);
        let server = tokio::spawn(async move { Connection::accept(server_end, &impostor, Pattern::Player).await });
        let client = Connection::connect(client_end, &server_keys.public, None).await;
        let server = server.await.unwrap();
        assert!(client.is_err() || server.is_err());
    }

    #[tokio::test]
    async fn peers_learn_who_connected() {
        let (login, game) = (Keypair::generate().unwrap(), Keypair::generate().unwrap());
        let (game_end, login_end) = tokio::io::duplex(1 << 17);
        let login_keys = login.clone();
        let accepted = tokio::spawn(async move { Connection::accept(login_end, &login_keys, Pattern::Peer).await });
        let _game = Connection::connect(game_end, &login.public, Some(&game)).await.unwrap();
        assert_eq!(accepted.await.unwrap().unwrap().remote_key(), Some(game.public));
    }

    #[tokio::test]
    async fn oversized_frames_are_refused_before_reading_them() {
        let (mut writer, mut reader) = tokio::io::duplex(64);
        tokio::io::AsyncWriteExt::write_all(&mut writer, &u32::MAX.to_le_bytes()).await.unwrap();
        assert!(matches!(frame::read(&mut reader).await, Err(Error::TooLarge(_))));
    }
}
