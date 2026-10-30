//! The world's HUD: what the character the player steers has left to fight with.

use super::Lobby;
use crate::screen::Screen;

impl Lobby {
    /// Shows how full each of the player's own pools is, which is how wide its bar is drawn, and the points
    /// behind it.
    pub(super) fn bind_vitals(&self, screen: &mut Screen) {
        let vitals = self.stands_at().map(|own| own.shown.vitals).unwrap_or_default();
        for (pool, name) in [(vitals.cp, "cp"), (vitals.hp, "hp"), (vitals.mp, "mp")] {
            screen.set(format!("world.{name}"), format!("{}%", pool.percent()));
            screen.set(format!("world.{name}-points"), format!("{} / {}", pool.now, pool.most));
        }
    }
}
