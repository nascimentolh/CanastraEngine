//! Admission: a player gets in once by a genuine, unexpired ticket for this server, while there is room.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use canastra_net::Connection;
use canastra_net::ticket::{Checker, TicketError};
use canastra_protocol::game::{GameClient, GameServer, Refusal};
use canastra_protocol::{AccountId, ServerId, VERSION};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::config::Result;

pub(crate) struct Admission {
    id: ServerId,
    capacity: u32,
    tickets: Checker,
    /// Players admitted and still connected, reported to the login server.
    pub(crate) online: AtomicU32,
    /// Nonces of tickets already used, with their expiry.
    used: Mutex<HashMap<[u8; 16], u64>>,
}

impl Admission {
    pub(crate) fn new(id: ServerId, capacity: u32, tickets: Checker) -> Self {
        Self { id, capacity, tickets, online: AtomicU32::new(0), used: Mutex::new(HashMap::new()) }
    }

    /// Reads the player's hello and answers it: the admitted account, or `None` when the player was told
    /// to update or refused. An admitted player holds a place until [`Admission::release`].
    pub(crate) async fn admit<S: AsyncRead + AsyncWrite + Unpin>(
        &self,
        connection: &mut Connection<S>,
    ) -> Result<Option<AccountId>> {
        let GameClient::Hello { version, ticket } = connection.recv().await? else {
            return Err("the client did not say hello first".into());
        };
        if version != VERSION {
            connection.send(&GameServer::UpdateRequired { version: VERSION }).await?;
            return Ok(None);
        }
        let now = unix_now();
        let refusal = match self.tickets.check(&ticket, self.id, now) {
            Err(TicketError::Forged | TicketError::WrongServer) => Refusal::InvalidTicket,
            Err(TicketError::Expired) => Refusal::Expired,
            Ok(ticket) if !self.first_use(ticket.nonce, ticket.expires_at, now) => Refusal::Reused,
            Ok(_) if !self.claim() => Refusal::Full,
            Ok(ticket) => {
                connection.send(&GameServer::Admitted).await?;
                tracing::info!(account = ticket.account.0, "player admitted");
                return Ok(Some(ticket.account));
            }
        };
        connection.send(&GameServer::Refused(refusal)).await?;
        Ok(None)
    }

    /// Frees the place an admitted player held.
    pub(crate) fn release(&self) {
        self.online.fetch_sub(1, Ordering::Relaxed);
    }

    /// Records a ticket's nonce; false when it was used before. Nonces of expired tickets are forgotten,
    /// since those tickets are refused anyway.
    fn first_use(&self, nonce: [u8; 16], expires_at: u64, now: u64) -> bool {
        let mut used = self.used.lock().unwrap_or_else(PoisonError::into_inner);
        used.retain(|_, &mut expiry| expiry > now);
        used.insert(nonce, expires_at).is_none()
    }

    /// Takes a place under the capacity; false when the server is full.
    fn claim(&self) -> bool {
        let capacity = self.capacity;
        self.online
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| (count < capacity).then_some(count + 1))
            .is_ok()
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use canastra_net::ticket::Issuer;
    use canastra_net::{Keypair, Pattern};
    use canastra_protocol::ticket::SignedTicket;
    use tokio::io::DuplexStream;

    use super::*;

    /// The server's answer to a hello with `ticket`, with the client's connection. An admitted player's server
    /// side stays connected, holding its place, until the returned connection drops.
    async fn hello(
        admission: &Arc<Admission>,
        keys: &Keypair,
        ticket: SignedTicket,
    ) -> (GameServer, Connection<DuplexStream>) {
        let (client_end, server_end) = tokio::io::duplex(1 << 17);
        let (server, server_keys) = (admission.clone(), keys.clone());
        tokio::spawn(async move {
            let mut connection = Connection::accept(server_end, &server_keys, Pattern::Player).await?;
            if server.admit(&mut connection).await?.is_some() {
                let _ = connection.recv::<GameClient>().await;
                server.release();
            }
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
        });
        let mut client = Connection::connect(client_end, &keys.public, None).await.unwrap();
        client.send(&GameClient::Hello { version: VERSION, ticket }).await.unwrap();
        (client.recv().await.unwrap(), client)
    }

    #[tokio::test]
    async fn tickets_admit_once_for_this_server_while_there_is_room() {
        let (issuer, keys) = (Issuer::generate().unwrap(), Keypair::generate().unwrap());
        let checker = Checker::from_hex(&issuer.public_hex()).unwrap();
        let admission = Arc::new(Admission::new(ServerId(1), 1, checker));
        let later = unix_now() + 30;

        let ticket = issuer.issue(AccountId(1), ServerId(1), later).unwrap();
        let (reply, _first) = hello(&admission, &keys, ticket.clone()).await;
        assert_eq!(reply, GameServer::Admitted);
        assert_eq!(hello(&admission, &keys, ticket).await.0, GameServer::Refused(Refusal::Reused));
        let elsewhere = issuer.issue(AccountId(1), ServerId(2), later).unwrap();
        assert_eq!(hello(&admission, &keys, elsewhere).await.0, GameServer::Refused(Refusal::InvalidTicket));
        let stale = issuer.issue(AccountId(1), ServerId(1), unix_now() - 1).unwrap();
        assert_eq!(hello(&admission, &keys, stale).await.0, GameServer::Refused(Refusal::Expired));
        // The first player, still connected, holds the only place.
        let another = issuer.issue(AccountId(2), ServerId(1), later).unwrap();
        assert_eq!(hello(&admission, &keys, another).await.0, GameServer::Refused(Refusal::Full));
    }
}
