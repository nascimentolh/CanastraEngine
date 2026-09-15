//! Player connections: version check, rate-limited authentication, the server list and tickets.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use canastra_db::{Authentication, Database};
use canastra_net::ticket::Issuer;
use canastra_net::{Connection, Keypair, Pattern};
use canastra_protocol::login::{AuthFailure, LoginClient, LoginServer, TicketRefusal};
use canastra_protocol::{AccountId, VERSION};
use tokio::net::{TcpListener, TcpStream};

use crate::config::Result;
use crate::limits::Attempts;
use crate::registry::Servers;

pub(crate) struct Login {
    pub(crate) database: Database,
    pub(crate) keys: Keypair,
    pub(crate) tickets: Issuer,
    pub(crate) ticket_lifetime: Duration,
    pub(crate) servers: Servers,
    pub(crate) by_address: Attempts,
    pub(crate) by_account: Attempts,
}

pub(crate) async fn listen(listener: TcpListener, login: Arc<Login>) {
    loop {
        let Ok((stream, address)) = listener.accept().await else { continue };
        let login = login.clone();
        tokio::spawn(async move {
            if let Err(error) = serve(stream, address, &login).await {
                tracing::debug!(%address, %error, "player connection closed");
            }
        });
    }
}

async fn serve(stream: TcpStream, address: SocketAddr, login: &Login) -> Result {
    let mut connection = Connection::accept(stream, &login.keys, Pattern::Player).await?;
    let LoginClient::Hello { version } = connection.recv().await? else {
        return Err("the client did not say hello first".into());
    };
    if version != VERSION {
        connection.send(&LoginServer::UpdateRequired { version: VERSION }).await?;
        return Ok(());
    }
    connection.send(&LoginServer::Welcome).await?;
    let account = authenticate(&mut connection, address, login).await?;
    tracing::info!(%address, account = account.0, "player authenticated");
    connection.send(&LoginServer::Servers(login.servers.list())).await?;
    loop {
        let LoginClient::RequestTicket { server } = connection.recv().await? else {
            return Err("unexpected message after authentication".into());
        };
        let reply = match login.servers.get(server) {
            None => LoginServer::TicketRefused(TicketRefusal::Offline),
            Some(entry) if entry.population >= entry.capacity => LoginServer::TicketRefused(TicketRefusal::Full),
            Some(_) => LoginServer::Ticket(login.tickets.issue(
                account,
                server,
                unix_now() + login.ticket_lifetime.as_secs(),
            )?),
        };
        connection.send(&reply).await?;
    }
}

/// Answers authentication attempts until one is accepted.
async fn authenticate(connection: &mut Connection<TcpStream>, address: SocketAddr, login: &Login) -> Result<AccountId> {
    loop {
        let LoginClient::Authenticate { account, password } = connection.recv().await? else {
            return Err("unexpected message before authentication".into());
        };
        let now = Instant::now();
        let allowed = login.by_address.allow(&address.ip().to_string(), now)
            && login.by_account.allow(&account.to_lowercase(), now);
        let failure = if allowed {
            match login.database.authenticate(&account, &password).await? {
                Authentication::Accepted(id) => return Ok(id),
                Authentication::WrongCredentials => AuthFailure::WrongCredentials,
                Authentication::Banned => AuthFailure::Banned,
            }
        } else {
            AuthFailure::TooManyAttempts
        };
        connection.send(&LoginServer::AuthFailed(failure)).await?;
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs())
}
