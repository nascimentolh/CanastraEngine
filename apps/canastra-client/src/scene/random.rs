//! Deterministic randomness, so a scene looks the same on every start.

use ue2_level::Range;

/// An xorshift generator; its state must not be zero.
pub(super) struct Random(pub(super) u64);

impl Random {
    /// Uniform in [0, 1).
    pub(super) fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    pub(super) fn range(&mut self, [min, max]: Range) -> f32 {
        min + (max - min) * self.unit()
    }
}
