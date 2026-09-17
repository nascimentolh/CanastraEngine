//! `FadeColor`: a tint the client swings between two colors over time, as torch glows pulse with it.

use std::f32::consts::TAU;

/// A tint that goes from `from` to `to` and back every `period` seconds, starting `phase` seconds in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fade {
    /// Each channel from 0 to 1, or past 1 where a combiner doubles or quadruples it.
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub period: f32,
    pub phase: f32,
    /// Eases in and out of each color (`ColorFadeType` 1) instead of moving between them at a steady rate.
    pub sinusoidal: bool,
}

impl Fade {
    /// The tint at `time` seconds.
    // ponytail: one full swing a period, from `Color1`; check against a slowed H5 capture if a glow pulses at the wrong pace.
    #[must_use]
    pub fn at(&self, time: f32) -> [f32; 3] {
        let turn = ((time + self.phase) / self.period.max(f32::EPSILON)).rem_euclid(1.0);
        let t = if self.sinusoidal { 0.5 - 0.5 * (turn * TAU).cos() } else { 1.0 - (2.0 * turn - 1.0).abs() };
        let mut color = self.from;
        for (channel, to) in color.iter_mut().zip(self.to) {
            *channel += (to - *channel) * t;
        }
        color
    }

    /// The same fade with both colors multiplied by `factor`.
    #[must_use]
    pub fn scaled(self, factor: f32) -> Self {
        Self { from: self.from.map(|channel| channel * factor), to: self.to.map(|channel| channel * factor), ..self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fade_swings_to_its_second_color_and_back_each_period() {
        let fade = Fade { from: [0.0; 3], to: [1.0, 0.5, 0.0], period: 0.4, phase: 0.0, sinusoidal: true };
        let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4);
        assert!(close(fade.at(0.0), [0.0; 3]));
        assert!(close(fade.at(0.2), [1.0, 0.5, 0.0]), "half a period in it reaches the second color");
        assert!(close(fade.at(0.4), [0.0; 3]), "and a full period brings it back");
        assert!(close(Fade { sinusoidal: false, ..fade }.at(0.1), [0.5, 0.25, 0.0]), "linear moves at a steady rate");
        assert!(close(fade.scaled(2.0).at(0.2), [2.0, 1.0, 0.0]));
    }
}
