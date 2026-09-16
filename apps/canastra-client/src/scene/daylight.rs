//! Time-of-day lighting for world zones, whose meshes carry no precomputed colors: Fermata's model of a
//! hemisphere ambient and a wrapped sun diffuse, fed by the client's `TimeEnv` ramps for one hour.
//!
//! The lobby scenes hold still at one hour, so the light is worked out once per vertex when the scene loads.
// ponytail: no sun shadows on static meshes (each instance's visibility bits), rim or specular yet; add them as the
// comparison with H5 shows they matter.

use l2_env::Environment;
use ue2_level::Actor;

use super::camera;

/// Sky light at one hour and where the sun shines from.
pub(super) struct Daylight {
    hour: f32,
    ambient: [f32; 3],
    sun: [f32; 3],
    /// Unit vector pointing toward the sun.
    toward_sun: [f32; 3],
}

impl Daylight {
    /// The light of one kind of surface at `hour`, from its `ambient` color ramp and `light` HSV ramp, with the
    /// sun where the level's `NMovableSunLight` points.
    pub(super) fn new(
        environment: &Environment,
        hour: f32,
        actors: &[Actor],
        [ambient, light]: [&str; 2],
    ) -> Option<Self> {
        let sun = actors.iter().find(|actor| actor.class.eq_ignore_ascii_case("NMovableSunLight"))?;
        let unit = |color: [u8; 3]| color.map(|channel| f32::from(channel) / 255.0);
        let [forward, _, _] = camera::axes(sun.placement.rotation);
        Some(Self {
            hour,
            ambient: unit(environment.color(ambient, hour)?),
            sun: unit(environment.light(light, hour)?),
            toward_sun: forward.map(|axis| -axis),
        })
    }

    /// Which of the eight time-of-day states a level stores, three hours each, holds this hour.
    // ponytail: states assumed to start at midnight, in order; check them against more H5 screenshots.
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to 0..8")]
    pub(super) fn time_slot(&self) -> u8 {
        (self.hour / 3.0).clamp(0.0, 7.0) as u8
    }

    /// The light on a surface facing `normal`, in world space, that receives `sunlit` of the sun and `skylit` of
    /// the sky, each from 0 to 1.
    pub(super) fn on_shaded(&self, normal: [f32; 3], sunlit: f32, skylit: f32) -> [f32; 3] {
        let length = normal.iter().map(|axis| axis * axis).sum::<f32>().sqrt().max(f32::EPSILON);
        let normal = normal.map(|axis| axis / length);
        // Surfaces facing down see the ground, which returns a dimmer, warmer share of the sky.
        let sky_weight = (normal[2] * 0.5 + 0.5).clamp(0.0, 1.0);
        let ground = [0.54, 0.47, 0.40];
        let incidence: f32 = normal.iter().zip(self.toward_sun).map(|(normal, sun)| normal * sun).sum();
        let wrapped = ((incidence + 0.08) / 1.08).clamp(0.0, 1.0);
        let diffuse = incidence.max(0.0) + (wrapped - incidence.max(0.0)) * 0.35;
        let mut light = [0.0; 3];
        for (((light, ambient), sun), ground) in light.iter_mut().zip(self.ambient).zip(self.sun).zip(ground) {
            *light = ambient * (ground + (1.0 - ground) * sky_weight) * skylit + sun * diffuse * sunlit;
        }
        light
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surfaces_facing_the_sun_get_it_and_facing_away_keep_the_ambient() {
        let daylight = Daylight { hour: 21.0, ambient: [0.5; 3], sun: [0.4; 3], toward_sun: [0.0, 0.0, 1.0] };
        let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-5);
        assert!(close(daylight.on_shaded([0.0, 0.0, 2.0], 1.0, 1.0), [0.9; 3]));
        assert!(close(daylight.on_shaded([0.0, 0.0, -1.0], 1.0, 1.0), [0.27, 0.235, 0.2]));
        assert!(close(daylight.on_shaded([0.0, 0.0, 1.0], 0.5, 1.0), [0.7; 3]));
        assert!(close(daylight.on_shaded([0.0, 0.0, 1.0], 1.0, 0.5), [0.65; 3]), "half the sky, all of the sun");
        assert_eq!(daylight.time_slot(), 7);
    }
}
