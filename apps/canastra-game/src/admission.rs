//! Player connections: a player is admitted once by a genuine, unexpired ticket for this server, while
//! there is room.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use canastra_net::ticket::{Checker, TicketError};
use canastra_net::{Connection, Keypair, Pattern};
use canastra_protocol::game::{GameClient, GameServer, Refusal};
use canastra_protocol::{ServerId, VERSION};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;

use crate::config::Result;

pub(crate) struct Admission {
    pub(crate) id: ServerId,
    pub(crate) capacity: u32,
    pub(crate) keys: Keypair,
    pub(crate) tickets: Checker,
    /// Players connected now, reported to the login server.
    pub(crate) online: AtomicU32,
    /// Nonces of tickets already used, with their expiry.
    used: Mutex<HashMap<[u8; 16], u64>>,
}

impl Admission {
    pub(crate) fn new(id: ServerId, capacity: u32, keys: Keypair, tickets: Checker) -> Self {
        Self { id, capacity, keys, tickets, online: AtomicU32::new(0), used: Mutex::new(HashMap::new()) }
    }

    /// Records a ticket's nonce; false when it was used before. Nonces of expired tickets are forgotten,
    /// since those tickets are refused anyway.
    fn first_use(&self, nonce: [u8; 16], expires_at: u64, now: u64) -> bool {
        let mut used = self.used.lock().unwrap_or_else(PoisonError::into_inner);
        used.retain(|_, &mut expiry| expiry > now);
        used.insert(nonce, expires_at).is_none()
    }
}

pub(crate) async fn listen(listener: TcpListener, admission: Arc<Admission>) {
    loop {
        let Ok((stream, address)) = listener.accept().await else { continue };
        let admission = admission.clone();
        tokio::spawn(async move {
            if let Err(error) = serve(stream, &admission).await {
                tracing::debug!(%address, %error, "player connection closed");
            }
        });
    }
}

async fn serve<S: AsyncRead + AsyncWrite + Unpin>(stream: S, admission: &Admission) -> Result {
    let mut connection = Connection::accept(stream, &admission.keys, Pattern::Player).await?;
    let GameClient::Hello { version, ticket } = connection.recv().await?;
    if version != VERSION {
        connection.send(&GameServer::UpdateRequired { version: VERSION }).await?;
        return Ok(());
    }
    let now = unix_now();
    let refusal = match admission.tickets.check(&ticket, admission.id, now) {
        Err(TicketError::Forged | TicketError::WrongServer) => Some(Refusal::InvalidTicket),
        Err(TicketError::Expired) => Some(Refusal::Expired),
        Ok(ticket) if !admission.first_use(ticket.nonce, ticket.expires_at, now) => Some(Refusal::Reused),
        Ok(_) if !claim(&admission.online, admission.capacity) => Some(Refusal::Full),
        Ok(ticket) => {
            tracing::info!(account = ticket.account.0, "player admitted");
            None
        }
    };
    if let Some(refusal) = refusal {
        connection.send(&GameServer::Refused(refusal)).await?;
        return Ok(());
    }
    let result = async {
        connection.send(&GameServer::Admitted).await?;
        // ponytail: admitted players only hold their connection until characters and the world arrive.
        let GameClient::Hello { .. } = connection.recv().await?;
        Err::<(), _>("a player said hello twice".into())
    }
    .await;
    admission.online.fetch_sub(1, Ordering::Relaxed);
    result
}

/// Takes a place under `capacity`; false when the server is full.
fn claim(online: &AtomicU32, capacity: u32) -> bool {
    online.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| (count < capacity).then_some(count + 1)).is_ok()
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use canastra_net::ticket::Issuer;
    use canastra_protocol::AccountId;
    use canastra_protocol::ticket::SignedTicket;
    use tokio::io::DuplexStream;

    /// The server's answer to a hello with `ticket`, with the client's connection so the caller can keep an
    /// admitted player connected.
    async fn hello(admission: &Arc<Admission>, ticket: SignedTicket) -> (GameServer, Connection<DuplexStream>) {
        let (client_end, server_end) = tokio::io::duplex(1 << 17);
        let server = admission.clone();
        tokio::spawn(async move { serve(server_end, &server).await });
        let mut client = Connection::connect(client_end, &admission.keys.public, None).await.unwrap();
        client.send(&GameClient::Hello { version: VERSION, ticket }).await.unwrap();
        (client.recv().await.unwrap(), client)
    }

    #[tokio::test]
    async fn tickets_admit_once_for_this_server_while_there_is_room() {
        let issuer = Issuer::generate().unwrap();
        let checker = Checker::from_hex(&issuer.public_hex()).unwrap();
        let admission = Arc::new(Admission::new(ServerId(1), 1, Keypair::generate().unwrap(), checker));
        let later = unix_now() + 30;

        let ticket = issuer.issue(AccountId(1), ServerId(1), later).unwrap();
        let (reply, _first) = hello(&admission, ticket.clone()).await;
        assert_eq!(reply, GameServer::Admitted);
        assert_eq!(hello(&admission, ticket).await.0, GameServer::Refused(Refusal::Reused));
        let elsewhere = issuer.issue(AccountId(1), ServerId(2), later).unwrap();
        assert_eq!(hello(&admission, elsewhere).await.0, GameServer::Refused(Refusal::InvalidTicket));
        let stale = issuer.issue(AccountId(1), ServerId(1), unix_now() - 1).unwrap();
        assert_eq!(hello(&admission, stale).await.0, GameServer::Refused(Refusal::Expired));
        // The first player, still connected, holds the only place.
        let another = issuer.issue(AccountId(2), ServerId(1), later).unwrap();
        assert_eq!(hello(&admission, another).await.0, GameServer::Refused(Refusal::Full));
    }
}
