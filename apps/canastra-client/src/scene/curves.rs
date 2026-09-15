//! Time curves of sprite particles: size and color over a life, repeating, and fading in and out.

use ue2_level::SpriteEmitter;

pub(super) fn size_at(sprite: &SpriteEmitter, life: f32) -> f32 {
    let points: Vec<(f32, [f32; 1])> = sprite.size_scale.iter().map(|&[time, size]| (time, [size])).collect();
    sample(&points, repeat(life, sprite.size_scale_repeats)).map_or(1.0, |[size]| size)
}

pub(super) fn color_at(sprite: &SpriteEmitter, life: f32) -> [f32; 4] {
    let points: Vec<(f32, [f32; 4])> = sprite
        .color_scale
        .iter()
        .map(|&(time, color)| (time, color.map(|channel| f32::from(channel) / 255.0)))
        .collect();
    sample(&points, repeat(life, sprite.color_scale_repeats)).unwrap_or([1.0; 4])
}

/// The fraction of a life that repeats `repeats` times over it.
fn repeat(life: f32, repeats: f32) -> f32 {
    let scaled = life * repeats;
    if scaled >= repeats { 1.0 } else { scaled.fract() }
}

/// Linear interpolation between curve points sorted by time; `None` for an empty curve.
fn sample<const N: usize>(points: &[(f32, [f32; N])], time: f32) -> Option<[f32; N]> {
    let (first, rest) = points.split_first()?;
    let mut previous = *first;
    if time <= previous.0 {
        return Some(previous.1);
    }
    for &point in rest {
        if time <= point.0 {
            let span = (point.0 - previous.0).max(1e-6);
            let t = (time - previous.0) / span;
            let mut value = previous.1;
            for (value, next) in value.iter_mut().zip(point.1) {
                *value += (next - *value) * t;
            }
            return Some(value);
        }
        previous = point;
    }
    Some(previous.1)
}

/// Opacity multiplier from fading in after birth and out towards death.
pub(super) fn fade(sprite: &SpriteEmitter, age: f32, lifetime: f32) -> f32 {
    let fade_in = sprite.fade_in_end.filter(|&end| end > 0.0).map_or(1.0, |end| (age / end).min(1.0));
    let fade_out = sprite
        .fade_out_start
        .filter(|&start| start < lifetime)
        .map_or(1.0, |start| if age <= start { 1.0 } else { ((lifetime - age) / (lifetime - start)).max(0.0) });
    fade_in * fade_out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ue2_level::DrawStyle;

    #[test]
    #[expect(clippy::float_cmp, reason = "the compared values are exact")]
    fn curves_and_fades_follow_unreal() {
        let points = [(0.0, [0.0]), (0.5, [1.0]), (1.0, [0.0])];
        assert_eq!(sample(&points, 0.25), Some([0.5]));
        assert_eq!(sample(&points, 2.0), Some([0.0]));
        assert_eq!(repeat(0.75, 2.0), 0.5);

        let mut sprite = SpriteEmitter {
            max_particles: 1,
            lifetime: [10.0; 2],
            start_offset: [0.0; 3],
            start_location: [[0.0; 2]; 3],
            start_velocity: [[0.0; 2]; 3],
            acceleration: [0.0; 3],
            start_size: [100.0; 2],
            size_scale: Vec::new(),
            size_scale_repeats: 1.0,
            color_scale: Vec::new(),
            color_scale_repeats: 1.0,
            color_multiplier: [[1.0; 2]; 3],
            opacity: 1.0,
            fade_in_end: Some(2.0),
            fade_out_start: Some(6.0),
            spin: None,
            texture: None,
            subdivisions: [1, 1],
            subdivision_range: [0, 0],
            draw_style: DrawStyle::Translucent,
            z_test: true,
            fogged: true,
            cloud_color: false,
            projection_normal: None,
        };
        assert_eq!(fade(&sprite, 1.0, 10.0), 0.5);
        assert_eq!(fade(&sprite, 4.0, 10.0), 1.0);
        assert_eq!(fade(&sprite, 8.0, 10.0), 0.5);
        sprite.fade_in_end = None;
        assert_eq!(fade(&sprite, 0.0, 10.0), 1.0);
    }
}
