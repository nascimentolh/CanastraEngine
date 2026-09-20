//! Who is in the world and how to reach them: every player in it has an outbox the world drops messages
//! into, which that player's own task writes out.
//!
//! The outbox is bounded. A player whose messages pile up is not waited for: the world drops it, which ends
//! its session, so one slow connection never holds the world or grows without end.

use std::collections::HashMap;
use std::sync::RwLock;

use canastra_protocol::game::{CharacterId, GameServer};
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::error::TrySendError;

/// Messages a player may fall behind by before its session is ended.
pub(crate) const OUTBOX: usize = 128;

/// The players in the world, by the character each entered as.
#[derive(Default)]
pub(crate) struct Registry {
    players: RwLock<HashMap<CharacterId, Sender<GameServer>>>,
}

impl Registry {
    /// Puts a player in the world, reachable at `outbox`.
    pub(crate) fn join(&self, character: CharacterId, outbox: Sender<GameServer>) {
        self.write().insert(character, outbox);
    }

    /// Takes a player out of the world; what is left in its outbox is dropped with it.
    pub(crate) fn leave(&self, character: CharacterId) {
        self.write().remove(&character);
    }

    /// How many players are in the world.
    pub(crate) fn len(&self) -> usize {
        self.read().len()
    }

    /// Sends `message` to one player. A player that has left, or that is too far behind to take it, is
    /// dropped from the world and told nothing more.
    pub(crate) fn tell(&self, character: CharacterId, message: GameServer) {
        let outbox = self.read().get(&character).cloned();
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

    fn read(&self) -> std::sync::RwLockReadGuard<'_, HashMap<CharacterId, Sender<GameServer>>> {
        self.players.read().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, HashMap<CharacterId, Sender<GameServer>>> {
        self.players.write().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_player_hears_the_world_until_it_falls_behind() {
        let registry = Registry::default();
        let (outbox, mut heard) = tokio::sync::mpsc::channel(2);
        registry.join(CharacterId(1), outbox);
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
        let (outbox, mut heard) = tokio::sync::mpsc::channel(OUTBOX);
        registry.join(CharacterId(7), outbox);
        registry.leave(CharacterId(7));
        registry.tell(CharacterId(7), GameServer::Admitted);
        assert_eq!(heard.try_recv().ok(), None);
    }
}
