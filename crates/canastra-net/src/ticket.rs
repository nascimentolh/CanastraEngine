//! Admission tickets: the login server signs them with its Ed25519 key, and game servers check them
//! with the matching public key, without asking the login server.

use std::fmt;

use canastra_protocol::ticket::{SignedTicket, Ticket};
use canastra_protocol::{AccountId, ServerId};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::{Error, parse_key};

/// Why a ticket is not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketError {
    /// Not signed by this login server, or altered.
    Forged,
    /// For another game server.
    WrongServer,
    Expired,
}

impl fmt::Display for TicketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Forged => "the ticket's signature does not match",
            Self::WrongServer => "the ticket is for another server",
            Self::Expired => "the ticket has expired",
        })
    }
}

impl std::error::Error for TicketError {}

/// The login server's ticket key.
pub struct Issuer(SigningKey);

impl Issuer {
    /// A new random signing key.
    pub fn generate() -> Result<Self, Error> {
        Ok(Self(SigningKey::from_bytes(&random()?)))
    }

    /// The signing key whose 32-byte secret is `hex`.
    pub fn from_hex(hex: &str) -> Result<Self, Error> {
        Ok(Self(SigningKey::from_bytes(&parse_key(hex)?)))
    }

    pub fn secret_hex(&self) -> String {
        hex::encode(self.0.to_bytes())
    }

    /// The public key game servers check tickets with.
    pub fn public_hex(&self) -> String {
        hex::encode(self.0.verifying_key().to_bytes())
    }

    /// A single-use ticket for `account` on `server`, valid until `expires_at` (Unix seconds).
    pub fn issue(&self, account: AccountId, server: ServerId, expires_at: u64) -> Result<SignedTicket, Error> {
        let ticket = canastra_protocol::encode(&Ticket { account, server, expires_at, nonce: random()? });
        let signature = self.0.sign(&ticket).to_bytes().to_vec();
        Ok(SignedTicket { ticket, signature })
    }
}

/// A game server's copy of the login server's public ticket key.
pub struct Checker(VerifyingKey);

impl Checker {
    pub fn from_hex(hex: &str) -> Result<Self, Error> {
        VerifyingKey::from_bytes(&parse_key(hex)?).map(Self).map_err(|_| Error::BadKey)
    }

    /// The ticket, when it is genuine, for `server` and not expired at `now` (Unix seconds). Callers
    /// still refuse a nonce they have seen, which makes each ticket single use.
    pub fn check(&self, signed: &SignedTicket, server: ServerId, now: u64) -> Result<Ticket, TicketError> {
        let signature = Signature::from_slice(&signed.signature).map_err(|_| TicketError::Forged)?;
        self.0.verify_strict(&signed.ticket, &signature).map_err(|_| TicketError::Forged)?;
        let ticket: Ticket = canastra_protocol::decode(&signed.ticket).map_err(|_| TicketError::Forged)?;
        if ticket.server != server {
            Err(TicketError::WrongServer)
        } else if now >= ticket.expires_at {
            Err(TicketError::Expired)
        } else {
            Ok(ticket)
        }
    }
}

fn random<const N: usize>() -> Result<[u8; N], Error> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).map_err(|_| Error::Io(std::io::Error::other("no system randomness")))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tickets_admit_only_their_server_until_they_expire() {
        let issuer = Issuer::generate().unwrap();
        let checker = Checker::from_hex(&issuer.public_hex()).unwrap();
        let signed = issuer.issue(AccountId(42), ServerId(1), 100).unwrap();

        assert_eq!(checker.check(&signed, ServerId(1), 99).unwrap().account, AccountId(42));
        assert_eq!(checker.check(&signed, ServerId(2), 99), Err(TicketError::WrongServer));
        assert_eq!(checker.check(&signed, ServerId(1), 100), Err(TicketError::Expired));

        let mut altered = signed.clone();
        if let Some(byte) = altered.ticket.first_mut() {
            *byte ^= 1;
        }
        assert_eq!(checker.check(&altered, ServerId(1), 99), Err(TicketError::Forged));
        let stranger = Checker::from_hex(&Issuer::generate().unwrap().public_hex()).unwrap();
        assert_eq!(stranger.check(&signed, ServerId(1), 99), Err(TicketError::Forged));
        assert_eq!(Issuer::from_hex(&issuer.secret_hex()).unwrap().public_hex(), issuer.public_hex());
    }
}
