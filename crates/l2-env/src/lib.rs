//! The client's time-of-day environment: `system/Env.int` sets when the day starts, and
//! `system/TimeEnv0.int` holds hourly color ramps for the sky, clouds, ambient light and more.

use std::path::Path;

pub struct Environment {
    start_hour: f32,
    /// Game hours that pass in one real hour; zero when the clock stands still.
    time_ratio: f32,
    time_env: String,
}

impl Environment {
    /// Reads the environment under `client_root`, or `None` when its files are missing or unreadable.
    pub fn read(client_root: &Path) -> Option<Self> {
        let text = |name: &str| {
            let path = client_root.join("system").join(name);
            let bytes = std::fs::read(&path).ok()?;
            // Comments are Korean in EUC-KR; only ASCII keys and numbers are read.
            Some(String::from_utf8_lossy(&l2_crypto::decrypt(&bytes, &path).ok()?).into_owned())
        };
        let env = text("Env.int")?;
        let number = |key| value(&env, "EnvSetup", key).and_then(|number| number.parse::<f32>().ok());
        let start_hour = number("StartTime")?;
        let clock = value(&env, "EnvSetup", "IsClock").is_some_and(|clock| clock.eq_ignore_ascii_case("true"));
        let time_ratio = if clock { number("TimeRatio").unwrap_or(1.0) } else { 0.0 };
        // ponytail: only the normal environment, TimeEnv0; the Seven Signs variants come with the world clock.
        Some(Self { start_hour, time_ratio, time_env: text("TimeEnv0.int")? })
    }

    /// The hour the client's clock starts at.
    pub fn start_hour(&self) -> f32 {
        self.start_hour
    }

    /// The hour the client's clock shows `seconds` of real time after it started, from 0 to 24.
    pub fn hour_after(&self, seconds: f32) -> f32 {
        clock_hour(self.start_hour, self.time_ratio, seconds)
    }

    /// The RGB of the color ramp `section` at `hour`, linear between its stops and held past the ends.
    pub fn color(&self, section: &str, hour: f32) -> Option<[u8; 3]> {
        ramp(&self.time_env, section, "color", ["R", "G", "B"], hour).map(to_bytes)
    }

    /// The RGB of the light ramp `section` at `hour`, whose stops are hue, saturation and brightness
    /// from 0 to 255, interpolated as such.
    pub fn light(&self, section: &str, hour: f32) -> Option<[u8; 3]> {
        ramp(&self.time_env, section, "light", ["Hue", "Sat", "Bri"], hour).map(|hsv| to_bytes(rgb(hsv)))
    }
}

/// The value of a ramp of `key<n>=(T=.., a=.., b=.., c=..)` stops at `hour`.
fn ramp(text: &str, section: &str, key: &str, fields: [&str; 3], hour: f32) -> Option<[f32; 3]> {
    let mut stops: Vec<(f32, [f32; 3])> = lines(text, section)
        .filter_map(|(name, value)| {
            let parts = value.trim().strip_prefix('(')?.strip_suffix(')')?;
            let field = |wanted: &str| {
                parts.split(',').find_map(|part| {
                    let (name, value) = part.split_once('=')?;
                    name.trim().eq_ignore_ascii_case(wanted).then(|| value.trim().parse::<f32>().ok()).flatten()
                })
            };
            name.trim().to_ascii_lowercase().starts_with(key).then_some(())?;
            let [a, b, c] = fields;
            Some((field("T")?, [field(a)?, field(b)?, field(c)?]))
        })
        .collect();
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    let after = stops.iter().position(|&(time, _)| time >= hour);
    Some(match after {
        Some(0) => stops.first()?.1,
        None => stops.last()?.1,
        Some(index) => {
            let ((from_time, from), (to_time, to)) = (stops.get(index - 1)?, stops.get(index)?);
            let t = (hour - from_time) / (to_time - from_time).max(f32::EPSILON);
            let mut value = *from;
            for (channel, to) in value.iter_mut().zip(to) {
                *channel += (to - *channel) * t;
            }
            value
        }
    })
}

/// Unreal Engine 2's light color (`FGetHSV`) as RGB from 0 to 255. Its saturation runs backwards, 255
/// being white, and brightness follows a curve that lifts dim values.
fn rgb([hue, saturation, brightness]: [f32; 3]) -> [f32; 3] {
    let brightness = brightness * 1.4 / 255.0;
    let brightness = (brightness * 0.7 / (0.01 + brightness.sqrt())).clamp(0.0, 1.0);
    let pure = if hue < 86.0 {
        [(85.0 - hue) / 85.0, hue / 85.0, 0.0]
    } else if hue < 171.0 {
        [0.0, (170.0 - hue) / 85.0, (hue - 85.0) / 85.0]
    } else {
        [(hue - 170.0) / 85.0, 0.0, (255.0 - hue) / 84.0]
    };
    pure.map(|channel| (channel + saturation / 255.0 * (1.0 - channel)) * brightness * 255.0)
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to a byte first")]
fn to_bytes(value: [f32; 3]) -> [u8; 3] {
    value.map(|channel| channel.round().clamp(0.0, 255.0) as u8)
}

/// The `key=value` lines of `[section]`, comments skipped.
fn lines<'a>(text: &'a str, section: &'a str) -> impl Iterator<Item = (&'a str, &'a str)> {
    let mut inside = false;
    text.lines().filter_map(move |line| {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|line| line.strip_suffix(']')) {
            inside = name.eq_ignore_ascii_case(section);
            return None;
        }
        if !inside || line.starts_with(';') {
            return None;
        }
        line.split_once('=')
    })
}

/// `start` plus `ratio` game hours for every real hour in `seconds`, wrapped to a day.
fn clock_hour(start: f32, ratio: f32, seconds: f32) -> f32 {
    (start + seconds * ratio / 3600.0).rem_euclid(24.0)
}

fn value<'a>(text: &'a str, section: &'a str, key: &str) -> Option<&'a str> {
    lines(text, section).find_map(|(name, value)| name.trim().eq_ignore_ascii_case(key).then_some(value.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramps_interpolate_between_stops_and_hold_past_the_ends() {
        let text = "[SkyBoxColor]\nNUM=2\n;comment\nCOLOR1=(T=10,R=0,G=100,B=200)\nColor2=(T=20,R=100,G=100,B=0) \n\
                    [HazeringColor]\nCOLOR1=(T=0,R=9,G=9,B=9)\n";
        let color = |section, hour| ramp(text, section, "color", ["R", "G", "B"], hour).map(to_bytes);
        assert_eq!(color("skyboxcolor", 15.0), Some([50, 100, 100]));
        assert_eq!(color("SkyBoxColor", 2.0), Some([0, 100, 200]));
        assert_eq!(color("SkyBoxColor", 23.0), Some([100, 100, 0]));
        assert_eq!(color("Missing", 1.0), None);
        // Noon in TimeEnv0 is hue 1, saturation 255: white, dimmed by the brightness curve.
        assert_eq!(to_bytes(rgb([1.0, 255.0, 190.0])), [181, 181, 181]);
        assert_eq!(to_bytes(rgb([85.0, 0.0, 255.0])), [0, 209, 0]);
        assert_eq!(value("[EnvSetup]\nStartTime=22\n", "EnvSetup", "StartTime"), Some("22"));
    }

    #[test]
    fn the_clock_runs_from_its_start_at_its_ratio_and_wraps_at_midnight() {
        // H5's lobby: from 22:00 at six game hours a real hour, midnight comes after twenty real minutes.
        assert!((clock_hour(22.0, 6.0, 0.0) - 22.0).abs() < 1e-4);
        assert!((clock_hour(22.0, 6.0, 600.0) - 23.0).abs() < 1e-4);
        assert!((clock_hour(22.0, 6.0, 1500.0) - 0.5).abs() < 1e-4);
        assert!((clock_hour(22.0, 0.0, 9999.0) - 22.0).abs() < 1e-4, "a clock that stands still");
    }
}
