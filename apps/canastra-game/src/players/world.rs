//! A player in the world: where its character stands, and the walks it asks for.
//!
//! Places are whole map units on the wire and floats while a character walks between them; the world is tens
//! of thousands of units across, so both hold every place exactly enough to stand on.
#![expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "map units, speeds and rotation units are small whole numbers beside the exact range of a float"
)]

use std::time::Instant;

use canastra_data::GameData;
use canastra_db::Database;
use canastra_net::Connection;
use canastra_protocol::game::{CharacterId, GameClient, GameServer, InWorld, Move};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::config::Result;

/// Runs the world for one player: it walks where it asks to until it leaves, and wherever it stands then is
/// where it stands again next time.
pub(crate) async fn run<S: AsyncRead + AsyncWrite + Unpin>(
    connection: &mut Connection<S>,
    database: &Database,
    data: &GameData,
    entered: InWorld,
) -> Result {
    let mut player = Player::new(&entered, speed_of(data, &entered));
    let result = steer(connection, &mut player).await;
    let (at, heading) = (player.at(), player.heading);
    database.place_character(player.character, at.map(round), heading).await?;
    result
}

/// Answers the player's asks until the connection ends.
async fn steer<S: AsyncRead + AsyncWrite + Unpin>(connection: &mut Connection<S>, player: &mut Player) -> Result {
    loop {
        match connection.recv().await? {
            // ponytail: every destination is granted as asked; geodata will say what the ground allows.
            GameClient::MoveTo(to) => {
                let walk = player.walk_to(to.map(|unit| unit as f32));
                connection.send(&GameServer::Moving(walk)).await?;
            }
            other => return Err(format!("a player in the world sent {other:?}").into()),
        }
    }
}

/// A character in the world, walking in a straight line from where it was to where it is bound.
struct Player {
    character: CharacterId,
    from: [f32; 3],
    to: [f32; 3],
    /// Which way it faces, in Unreal rotation units.
    heading: i32,
    /// Map units a second.
    speed: f32,
    /// When the walk from `from` began.
    since: Instant,
}

impl Player {
    fn new(entered: &InWorld, speed: f32) -> Self {
        let at = entered.position.map(|unit| unit as f32);
        Self {
            character: entered.character.id,
            from: at,
            to: at,
            heading: entered.heading,
            speed,
            since: Instant::now(),
        }
    }

    /// Where the character stands now, along the walk it is on.
    fn at(&self) -> [f32; 3] {
        let gone = self.speed * self.since.elapsed().as_secs_f32();
        let length = distance(self.from, self.to);
        let part = if length > 0.0 { (gone / length).min(1.0) } else { 1.0 };
        let mut at = self.from;
        for (at, to) in at.iter_mut().zip(self.to) {
            *at += (to - *at) * part;
        }
        at
    }

    /// Starts a walk from where the character stands now to `to`, facing that way.
    fn walk_to(&mut self, to: [f32; 3]) -> Move {
        self.from = self.at();
        self.to = to;
        self.since = Instant::now();
        if let Some(heading) = facing(self.from, self.to) {
            self.heading = heading;
        }
        Move { from: self.from.map(round), to: self.to.map(round), speed: self.speed }
    }
}

/// How fast the character runs, from its class template; characters of a class without one stand still.
fn speed_of(data: &GameData, entered: &InWorld) -> f32 {
    data.starting_class(entered.character.class).map_or(0.0, |start| start.template.move_speed.run as f32)
}

fn distance(from: [f32; 3], to: [f32; 3]) -> f32 {
    from.iter().zip(to).map(|(from, to)| (to - from) * (to - from)).sum::<f32>().sqrt()
}

/// Which way a walk from `from` to `to` faces, in Unreal rotation units; `None` when it goes nowhere.
fn facing(from: [f32; 3], to: [f32; 3]) -> Option<i32> {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    (dx != 0.0 || dy != 0.0).then(|| (dy.atan2(dx) / std::f32::consts::TAU * 65536.0) as i32)
}

fn round(value: f32) -> i32 {
    value.round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standing(speed: f32) -> Player {
        Player { character: CharacterId(1), from: [0.0; 3], to: [0.0; 3], heading: 0, speed, since: Instant::now() }
    }

    #[test]
    fn a_walk_starts_where_the_character_stands_and_faces_where_it_goes() {
        let mut player = standing(100.0);
        let walk = player.walk_to([1000.0, 0.0, 0.0]);
        assert_eq!(walk.from, [0, 0, 0]);
        assert_eq!(walk.to, [1000, 0, 0]);
        assert_eq!(player.heading, 0, "walking along +X faces along +X");
        // A quarter turn is 16384 units; walking along +Y faces that way.
        player.walk_to([1000.0, 1000.0, 0.0]);
        assert_eq!(player.heading, 8192, "walking between +X and +Y faces between them");
        // A character that never moved stands where it started.
        assert!(standing(0.0).at().iter().all(|at| at.abs() < f32::EPSILON));
    }
}
