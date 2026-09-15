//! The client's time-of-day environment: `system/Env.int` sets when the day starts, and
//! `system/TimeEnv0.int` holds hourly color ramps for the sky, clouds, ambient light and more.

use std::path::Path;

pub struct Environment {
    start_hour: f32,
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
        let start_hour = value(&text("Env.int")?, "EnvSetup", "StartTime").and_then(|hour| hour.parse().ok())?;
        // ponytail: only the normal environment, TimeEnv0; the Seven Signs variants come with the world clock.
        Some(Self { start_hour, time_env: text("TimeEnv0.int")? })
    }

    /// The hour the client's clock starts at.
    pub fn start_hour(&self) -> f32 {
        self.start_hour
    }

    /// The RGB of the color ramp `section` at `hour`, linear between its stops and held past the ends.
    pub fn color(&self, section: &str, hour: f32) -> Option<[u8; 3]> {
        color_at(&self.time_env, section, hour)
    }
}

fn color_at(text: &str, section: &str, hour: f32) -> Option<[u8; 3]> {
    let mut stops: Vec<(f32, [f32; 3])> = lines(text, section)
        .filter_map(|(key, value)| {
            let fields = value.trim().strip_prefix('(')?.strip_suffix(')')?;
            let field = |name: &str| {
                fields.split(',').find_map(|field| {
                    let (key, value) = field.split_once('=')?;
                    key.trim().eq_ignore_ascii_case(name).then(|| value.trim().parse::<f32>().ok()).flatten()
                })
            };
            key.to_ascii_lowercase().starts_with("color").then_some(())?;
            Some((field("T")?, [field("R")?, field("G")?, field("B")?]))
        })
        .collect();
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    let after = stops.iter().position(|&(time, _)| time >= hour);
    let color = match after {
        Some(0) => stops.first()?.1,
        None => stops.last()?.1,
        Some(index) => {
            let ((from_time, from), (to_time, to)) = (stops.get(index - 1)?, stops.get(index)?);
            let t = (hour - from_time) / (to_time - from_time).max(f32::EPSILON);
            let mut color = *from;
            for (channel, to) in color.iter_mut().zip(to) {
                *channel += (to - *channel) * t;
            }
            color
        }
    };
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to a byte first")]
    Some(color.map(|channel| channel.round().clamp(0.0, 255.0) as u8))
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
        assert_eq!(color_at(text, "skyboxcolor", 15.0), Some([50, 100, 100]));
        assert_eq!(color_at(text, "SkyBoxColor", 2.0), Some([0, 100, 200]));
        assert_eq!(color_at(text, "SkyBoxColor", 23.0), Some([100, 100, 0]));
        assert_eq!(color_at(text, "Missing", 1.0), None);
        assert_eq!(value("[EnvSetup]\nStartTime=22\n", "EnvSetup", "StartTime"), Some("22"));
    }
}
