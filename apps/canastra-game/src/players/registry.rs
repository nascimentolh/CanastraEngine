//! Who is in the world, where each one stands, and how to reach them: every player in it has an outbox the
//! world drops messages into, which that player's own task writes out.
//!
//! The outbox is bounded. A player whose messages pile up is not waited for: the world drops it, which ends
//! its session, so one slow connection never holds the world or grows without end.
//!
//! Players are also kept in a grid of square cells, so telling the ones around a place costs the few cells
//! it spans instead of a walk over everyone in the world.

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use canastra_protocol::game::{CharacterId, GameServer, InWorld, Move};
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::error::TrySendError;

/// Messages a player may fall behind by before its session is ended.
pub(crate) const OUTBOX: usize = 128;
/// How far a player sees others, in world units.
pub(crate) const VIEW: i32 = 4000;
/// World units a cell of the grid covers on each axis; a view spans a few of them.
const CELL: i32 = 2048;

/// A player in the world: where its character stands, who it can see, and how to reach it.
struct Player {
    outbox: Sender<GameServer>,
    /// The character as others see it, with where it stands and the walk it is on.
    presence: InWorld,
    walk: Option<Move>,
    /// The characters this player has been told about and not yet told have gone.
    seen: HashSet<CharacterId>,
}

/// The players in the world, by the character each entered as.
#[derive(Default)]
pub(crate) struct Registry {
    players: RwLock<HashMap<CharacterId, Player>>,
    /// Which characters stand in each cell of the grid.
    cells: RwLock<HashMap<[i32; 2], HashSet<CharacterId>>>,
}

impl Registry {
    /// Puts a player in the world where `presence` says, reachable at `outbox`.
    pub(crate) fn join(&self, presence: InWorld, outbox: Sender<GameServer>) {
        let character = presence.character.id;
        let cell = cell_of(presence.position);
        let player = Player { outbox, presence, walk: None, seen: HashSet::new() };
        self.write().insert(character, player);
        self.cells_mut().entry(cell).or_default().insert(character);
    }

    /// Takes a player out of the world, telling everyone who could see it that it is gone.
    pub(crate) fn leave(&self, character: CharacterId) {
        let gone = self.write().remove(&character);
        let Some(gone) = gone else { return };
        self.cells_mut().entry(cell_of(gone.presence.position)).or_default().remove(&character);
        for watcher in self.watchers(character) {
            self.tell(watcher, GameServer::Vanishes(character));
            if let Some(player) = self.write().get_mut(&watcher) {
                player.seen.remove(&character);
            }
        }
    }

    /// How many players are in the world.
    pub(crate) fn len(&self) -> usize {
        self.read().len()
    }

    /// Takes note of a walk a player was granted and tells everyone who can see it.
    pub(crate) fn walks(&self, character: CharacterId, walk: Move) {
        let moved = {
            let mut players = self.write();
            let Some(player) = players.get_mut(&character) else { return };
            let was = cell_of(player.presence.position);
            player.presence.position = walk.from;
            player.walk = Some(walk);
            (was, cell_of(walk.from))
        };
        if moved.0 != moved.1 {
            self.cells_mut().entry(moved.0).or_default().remove(&character);
            self.cells_mut().entry(moved.1).or_default().insert(character);
        }
        for watcher in self.around(walk.from) {
            self.tell(watcher, GameServer::Moving { character, walk });
        }
    }

    /// Brings every player's view up to date: characters that came within sight are shown to it, and those
    /// that went out of sight are taken away.
    pub(crate) fn follow(&self) {
        let players: Vec<CharacterId> = self.read().keys().copied().collect();
        for watcher in players {
            let Some(at) = self.read().get(&watcher).map(|player| player.presence.position) else { continue };
            let near: HashSet<CharacterId> =
                self.around(at).into_iter().filter(|character| *character != watcher).collect();
            let seen = self.read().get(&watcher).map(|player| player.seen.clone()).unwrap_or_default();
            for gone in seen.difference(&near) {
                self.tell(watcher, GameServer::Vanishes(*gone));
            }
            for came in near.difference(&seen) {
                let Some((presence, walk)) = self.read().get(came).map(|player| (player.presence.clone(), player.walk))
                else {
                    continue;
                };
                self.tell(watcher, GameServer::Appears(presence));
                if let Some(walk) = walk {
                    self.tell(watcher, GameServer::Moving { character: *came, walk });
                }
            }
            if let Some(player) = self.write().get_mut(&watcher) {
                player.seen = near;
            }
        }
    }

    /// Sends `message` to one player. A player that has left, or that is too far behind to take it, is
    /// dropped from the world and told nothing more.
    pub(crate) fn tell(&self, character: CharacterId, message: GameServer) {
        let outbox = self.read().get(&character).map(|player| player.outbox.clone());
        let Some(outbox) = outbox else { return };
        match outbox.try_send(message) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                tracing::info!(character = character.0, "player too far behind to follow the world");
                self.leave(character);
            }
            Err(TrySendError::Closed(_)) => self.leave(character),
        }
    }

    /// The players close enough to `at` to see what happens there.
    fn around(&self, at: [i32; 3]) -> Vec<CharacterId> {
        let [cell_x, cell_y] = cell_of(at);
        let reach = VIEW.div_euclid(CELL) + 1;
        let cells = self.cells();
        let mut near = Vec::new();
        for x in cell_x - reach..=cell_x + reach {
            for y in cell_y - reach..=cell_y + reach {
                near.extend(cells.get(&[x, y]).into_iter().flatten().copied());
            }
        }
        drop(cells);
        let players = self.read();
        near.retain(|character| players.get(character).is_some_and(|player| within_view(player.presence.position, at)));
        near
    }

    /// The players that have been told about `character`.
    fn watchers(&self, character: CharacterId) -> Vec<CharacterId> {
        self.read().iter().filter(|(_, player)| player.seen.contains(&character)).map(|(watcher, _)| *watcher).collect()
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, HashMap<CharacterId, Player>> {
        self.players.read().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, HashMap<CharacterId, Player>> {
        self.players.write().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn cells(&self) -> std::sync::RwLockReadGuard<'_, HashMap<[i32; 2], HashSet<CharacterId>>> {
        self.cells.read().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn cells_mut(&self) -> std::sync::RwLockWriteGuard<'_, HashMap<[i32; 2], HashSet<CharacterId>>> {
        self.cells.write().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The cell of the grid a place falls in.
fn cell_of([x, y, _]: [i32; 3]) -> [i32; 2] {
    [x.div_euclid(CELL), y.div_euclid(CELL)]
}

/// Whether two places are close enough for one to see the other.
fn within_view(one: [i32; 3], other: [i32; 3]) -> bool {
    let (dx, dy) = (i64::from(one[0] - other[0]), i64::from(one[1] - other[1]));
    dx * dx + dy * dy <= i64::from(VIEW) * i64::from(VIEW)
}

#[cfg(test)]
mod tests {
    use canastra_data::id::ClassId;
    use canastra_protocol::game::{Appearance, CharacterSummary, Sex};
    use tokio::sync::mpsc::Receiver;

    use super::*;

    fn presence(id: i64, at: [i32; 3]) -> InWorld {
        let character = CharacterSummary {
            id: CharacterId(id),
            name: format!("Hero{id}"),
            class: ClassId(0),
            sex: Sex::Male,
            appearance: Appearance::default(),
            level: 1,
            gear: Vec::new(),
        };
        InWorld { character, position: at, heading: 0 }
    }

    fn enter(registry: &Registry, id: i64, at: [i32; 3]) -> Receiver<GameServer> {
        let (outbox, heard) = tokio::sync::mpsc::channel(OUTBOX);
        registry.join(presence(id, at), outbox);
        heard
    }

    #[tokio::test]
    async fn players_see_each_other_when_near_and_lose_sight_when_far() {
        let registry = Registry::default();
        let mut first = enter(&registry, 1, [0, 0, 0]);
        let mut second = enter(&registry, 2, [1000, 0, 0]);
        registry.follow();
        assert!(matches!(first.try_recv(), Ok(GameServer::Appears(seen)) if seen.character.id == CharacterId(2)));
        assert!(matches!(second.try_recv(), Ok(GameServer::Appears(seen)) if seen.character.id == CharacterId(1)));

        // The second walks out of sight: both are told the other is gone, once.
        let away = [VIEW * 2, 0, 0];
        let walk = Move { from: away, to: away, speed: 100.0 };
        registry.walks(CharacterId(2), walk);
        registry.follow();
        assert_eq!(first.try_recv().ok(), Some(GameServer::Vanishes(CharacterId(2))));
        // The one walking hears its own walk before it is told the other is out of sight.
        assert_eq!(second.try_recv().ok(), Some(GameServer::Moving { character: CharacterId(2), walk }));
        assert_eq!(second.try_recv().ok(), Some(GameServer::Vanishes(CharacterId(1))));
        registry.follow();
        assert!(first.try_recv().is_err(), "nothing more is said about a character already gone");
    }

    #[tokio::test]
    async fn a_walk_reaches_those_who_can_see_it_and_no_one_else() {
        let registry = Registry::default();
        let mut near = enter(&registry, 1, [0, 0, 0]);
        let mut far = enter(&registry, 2, [VIEW * 3, 0, 0]);
        registry.follow();
        let walk = Move { from: [100, 0, 0], to: [200, 0, 0], speed: 120.0 };
        registry.walks(CharacterId(1), walk);
        let heard: Vec<GameServer> = std::iter::from_fn(|| near.try_recv().ok()).collect();
        assert!(heard.contains(&GameServer::Moving { character: CharacterId(1), walk }));
        assert!(far.try_recv().is_err(), "a walk out of sight is not heard");
        // Whoever arrives next is shown where the walk left it, and the walk it is on.
        let mut arrived = enter(&registry, 3, [0, 0, 0]);
        registry.follow();
        let heard: Vec<GameServer> = std::iter::from_fn(|| arrived.try_recv().ok()).collect();
        assert!(heard.iter().any(|told| matches!(told, GameServer::Appears(seen) if seen.position == [100, 0, 0])));
        assert!(heard.contains(&GameServer::Moving { character: CharacterId(1), walk }));
    }

    #[tokio::test]
    async fn a_player_hears_the_world_until_it_falls_behind() {
        let registry = Registry::default();
        let (outbox, mut heard) = tokio::sync::mpsc::channel(2);
        registry.join(presence(1, [0, 0, 0]), outbox);
        assert_eq!(registry.len(), 1);
        registry.tell(CharacterId(1), GameServer::Admitted);
        assert_eq!(heard.recv().await, Some(GameServer::Admitted));

        // Nothing is read from here on, so the outbox fills and the player is dropped from the world.
        for _ in 0..4 {
            registry.tell(CharacterId(1), GameServer::Admitted);
        }
        assert_eq!(registry.len(), 0, "a player that cannot keep up leaves the world");
        registry.tell(CharacterId(1), GameServer::Admitted);
    }

    #[tokio::test]
    async fn a_player_that_left_is_told_nothing() {
        let registry = Registry::default();
        let mut heard = enter(&registry, 7, [0, 0, 0]);
        registry.leave(CharacterId(7));
        registry.tell(CharacterId(7), GameServer::Admitted);
        assert!(heard.try_recv().is_err());
    }
}
