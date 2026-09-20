//! Canastra's network protocol: typed messages, one enum per connection phase and direction, encoded
//! with postcard. Nothing here does IO; `canastra-net` carries these messages over encrypted frames.

pub mod game;
pub mod login;
pub mod registry;
pub mod ticket;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Bump on any change to a message type; the `layout_is_frozen` tests fail until you do. Peers on other
/// versions are refused, never adapted to.
pub const VERSION: u32 = 5;

/// A game server, as the login server and its game servers know it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ServerId(pub u16);

/// An account, as stored by the login server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AccountId(pub i64);

/// # Panics
///
/// Never: every message serializes into memory.
pub fn encode<T: Serialize>(message: &T) -> Vec<u8> {
    postcard::to_allocvec(message).expect("serializing a message into memory cannot fail")
}

/// Decodes one whole message; trailing bytes are an error.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, postcard::Error> {
    let (message, rest) = postcard::take_from_bytes(bytes)?;
    if rest.is_empty() { Ok(message) } else { Err(postcard::Error::DeserializeBadEncoding) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use login::{LoginClient, LoginServer, ServerEntry};
    use registry::GameToLogin;

    #[test]
    fn layout_is_frozen() {
        // These bytes change only with VERSION: an edited message fails here until it is bumped.
        let authenticate = LoginClient::Authenticate { account: "ana".into(), password: "pw".into() };
        assert_eq!(encode(&authenticate), [1, 3, b'a', b'n', b'a', 2, b'p', b'w']);
        let servers = LoginServer::Servers(vec![ServerEntry {
            id: ServerId(1),
            name: "A".into(),
            address: "h:1".into(),
            key: [9; 32],
            population: 2,
            capacity: 300,
        }]);
        let mut expected = vec![3, 1, 1, 1, b'A', 3, b'h', b':', b'1'];
        expected.extend([9; 32]);
        expected.extend([2, 0xAC, 0x02]);
        assert_eq!(encode(&servers), expected);
        let register = GameToLogin::Register {
            version: VERSION,
            id: ServerId(7),
            name: String::new(),
            address: String::new(),
            capacity: 1,
        };
        assert_eq!(encode(&register), [0, 5, 7, 0, 0, 1]);
        assert_eq!(encode(&game::GameClient::EnterWorld(game::CharacterId(5))), [3, 10]);
        let refused = game::GameServer::Refused(game::Refusal::Reused);
        assert_eq!(encode(&refused), [2, 2]);
        let summary = game::CharacterSummary {
            id: game::CharacterId(5),
            name: "Ana".into(),
            class: canastra_data::id::ClassId(124),
            sex: game::Sex::Female,
            appearance: game::Appearance { hair_style: 1, hair_color: 2, face: 0 },
            level: 1,
            gear: vec![canastra_data::id::ItemId(57)],
        };
        assert_eq!(
            encode(&game::GameServer::Characters(vec![summary])),
            [3, 1, 10, 3, b'A', b'n', b'a', 124, 1, 1, 2, 0, 1, 1, 57]
        );
    }

    #[test]
    fn decoding_rejects_trailing_bytes() {
        let mut bytes = encode(&LoginClient::Hello { version: VERSION });
        assert_eq!(decode::<LoginClient>(&bytes).unwrap(), LoginClient::Hello { version: VERSION });
        bytes.push(0);
        assert!(decode::<LoginClient>(&bytes).is_err());
    }
}
