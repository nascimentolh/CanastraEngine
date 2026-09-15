//! The login server end to end: a game server registers, a player authenticates, lists servers and
//! receives a ticket that checks out.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use canastra_db::Database;
use canastra_net::ticket::{Checker, Issuer};
use canastra_net::{Connection, Keypair};
use canastra_protocol::login::{AuthFailure, LoginClient, LoginServer};
use canastra_protocol::registry::{GameToLogin, LoginToGame};
use canastra_protocol::{ServerId, VERSION};
use tokio::net::{TcpListener, TcpStream};

use crate::limits::Attempts;
use crate::players::{self, Login};
use crate::registry::{self, Servers};

/// Needs PostgreSQL: `CANASTRA_TEST_DATABASE_URL=postgres://... cargo test -p canastra-login -- --ignored`.
#[tokio::test]
#[ignore = "needs a PostgreSQL database in CANASTRA_TEST_DATABASE_URL"]
async fn players_authenticate_and_receive_tickets_for_registered_servers() {
    let url = std::env::var("CANASTRA_TEST_DATABASE_URL").expect("CANASTRA_TEST_DATABASE_URL is set");
    let database = Database::connect(&url).await.unwrap();
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos();
    let name = format!("login_{}_{nanos}", std::process::id());
    let account = database.create_account(&name, "secret").await.unwrap();

    let (login_keys, game_keys, tickets) =
        (Keypair::generate().unwrap(), Keypair::generate().unwrap(), Issuer::generate().unwrap());
    let checker = Checker::from_hex(&tickets.public_hex()).unwrap();
    let servers = Servers::default();
    let (player_listener, game_listener) =
        (TcpListener::bind("127.0.0.1:0").await.unwrap(), TcpListener::bind("127.0.0.1:0").await.unwrap());
    let (player_address, game_address) = (player_listener.local_addr().unwrap(), game_listener.local_addr().unwrap());
    let authorized = Arc::new(HashMap::from([(game_keys.public, ServerId(1))]));
    tokio::spawn(registry::listen(game_listener, Arc::new(login_keys.clone()), authorized, servers.clone()));
    let window = Duration::from_secs(60);
    let login = Arc::new(Login {
        database,
        keys: login_keys.clone(),
        tickets,
        ticket_lifetime: Duration::from_secs(30),
        servers,
        by_address: Attempts::new(5, window),
        by_account: Attempts::new(5, window),
    });
    tokio::spawn(players::listen(player_listener, login));

    let game_stream = TcpStream::connect(game_address).await.unwrap();
    let mut game = Connection::connect(game_stream, &login_keys.public, Some(&game_keys)).await.unwrap();
    let register = GameToLogin::Register {
        version: VERSION,
        id: ServerId(1),
        name: "Test".into(),
        address: "127.0.0.1:7777".into(),
        capacity: 10,
    };
    game.send(&register).await.unwrap();
    assert_eq!(game.recv::<LoginToGame>().await.unwrap(), LoginToGame::Registered);

    let player_stream = TcpStream::connect(player_address).await.unwrap();
    let mut player = Connection::connect(player_stream, &login_keys.public, None).await.unwrap();
    player.send(&LoginClient::Hello { version: VERSION }).await.unwrap();
    assert_eq!(player.recv::<LoginServer>().await.unwrap(), LoginServer::Welcome);
    player.send(&LoginClient::Authenticate { account: name.clone(), password: "wrong".into() }).await.unwrap();
    assert_eq!(player.recv::<LoginServer>().await.unwrap(), LoginServer::AuthFailed(AuthFailure::WrongCredentials));
    player.send(&LoginClient::Authenticate { account: name.to_uppercase(), password: "secret".into() }).await.unwrap();
    let LoginServer::Servers(list) = player.recv().await.unwrap() else { panic!("expected the server list") };
    assert_eq!(list.iter().map(|entry| entry.id).collect::<Vec<_>>(), [ServerId(1)]);

    player.send(&LoginClient::RequestTicket { server: ServerId(1) }).await.unwrap();
    let LoginServer::Ticket(signed) = player.recv().await.unwrap() else { panic!("expected a ticket") };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    assert_eq!(checker.check(&signed, ServerId(1), now).unwrap().account, account);
}
