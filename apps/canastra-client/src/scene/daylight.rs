//! Time-of-day lighting for world zones, whose meshes carry no precomputed colors: Fermata's model of a
//! hemisphere ambient and a wrapped sun diffuse, fed by the client's `TimeEnv` ramps for one hour.
//!
//! The lobby scenes hold still at the client's starting hour, so the light is worked out once per vertex when
//! the scene loads.
// ponytail: no sun shadows (each mesh instance's visibility bits), rim or specular yet; add them as the
// comparison with H5 shows they matter.

use l2_env::Environment;
use ue2_level::Actor;

use super::camera;

/// Sky light at one hour and where the sun shines from.
pub(super) struct Daylight {
    ambient: [f32; 3],
    sun: [f32; 3],
    /// Unit vector pointing toward the sun.
    toward_sun: [f32; 3],
}

impl Daylight {
    /// The static mesh light of `environment` at its starting hour, with the sun where the level's
    /// `NMovableSunLight` points.
    pub(super) fn static_mesh(environment: &Environment, actors: &[Actor]) -> Option<Self> {
        let sun = actors.iter().find(|actor| actor.class.eq_ignore_ascii_case("NMovableSunLight"))?;
        let hour = environment.start_hour();
        let unit = |color: [u8; 3]| color.map(|channel| f32::from(channel) / 255.0);
        let [forward, _, _] = camera::axes(sun.placement.rotation);
        Some(Self {
            ambient: unit(environment.color("StaticMeshAmbient", hour)?),
            sun: unit(environment.light("HSVStaticMeshLight", hour)?),
            toward_sun: forward.map(|axis| -axis),
        })
    }

    /// The light falling on a surface facing `normal`, in world space.
    pub(super) fn on(&self, normal: [f32; 3]) -> [f32; 3] {
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
            *light = ambient * (ground + (1.0 - ground) * sky_weight) + sun * diffuse;
        }
        light
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surfaces_facing_the_sun_get_it_and_facing_away_keep_the_ambient() {
        let daylight = Daylight { ambient: [0.5; 3], sun: [0.4; 3], toward_sun: [0.0, 0.0, 1.0] };
        let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-5);
        assert!(close(daylight.on([0.0, 0.0, 2.0]), [0.9; 3]));
        assert!(close(daylight.on([0.0, 0.0, -1.0]), [0.27, 0.235, 0.2]));
    }
}
