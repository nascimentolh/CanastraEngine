//! Sound: the lobby's music, played at the volumes the player sets and kept in a settings file beside the
//! client. The client's music files are Ogg Vorbis whose first four bytes read `L2SD` in place of `OggS`.

use std::io::Cursor;
use std::path::Path;
use std::time::{Duration, Instant};

use rodio::stream::DeviceSinkBuilder;
use rodio::{Decoder, MixerDeviceSink, Player, Source};

/// Where the player's volumes are kept, beside the client.
const SETTINGS: &str = "canastra-settings.toml";
/// Steps a volume moves in, out of 100.
const STEP: u8 = 10;

/// What the player hears and how loudly.
pub(crate) struct Audio {
    /// The device everything plays through; dropping it closes it.
    device: MixerDeviceSink,
    music: Player,
    /// Music and effect volumes, each from 0 to 100.
    volumes: [u8; 2],
    /// Whether everything is silenced, whatever the volumes say.
    muted: bool,
    /// The ambient sounds heard where the camera stands, each looping on its own.
    ambient: Vec<Ambient>,
    /// The track playing and its bytes, which start again each time it ends.
    // ponytail: the track is decoded again every time round; rodio 0.22's `repeat_infinite` ends a decoder after a
    // fraction of a second, so it cannot loop one.
    track: Option<(String, Vec<u8>)>,
}

/// A sound the scene hands over: its file, how loudly it is heard, how often it is called, and how fast it plays.
pub(crate) struct Clip {
    pub(crate) bytes: Vec<u8>,
    pub(crate) gain: f32,
    /// Seconds between calls for a sound heard now and then; `None` for one that never stops.
    pub(crate) interval: Option<f32>,
    pub(crate) pitch: f32,
}

/// One ambient sound playing: what plays it, the clip, and when it is called again.
struct Ambient {
    player: Player,
    clip: Clip,
    /// When the sound is next called, for one heard now and then.
    next: Option<Instant>,
}

/// Which volume a call means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Music = 0,
    Effects = 1,
}

impl Audio {
    /// Opens the sound device with the settings last saved, or `None` when no device will open.
    pub(crate) fn open() -> Option<Self> {
        let device = DeviceSinkBuilder::open_default_sink().ok()?;
        let music = Player::connect_new(device.mixer());
        let (volumes, muted) = read_settings();
        let audio = Self { device, music, volumes, muted, ambient: Vec::new(), track: None };
        audio.apply();
        Some(audio)
    }

    /// Plays the track named `track` from the client's `music` folder, unless it is already playing.
    pub(crate) fn play_music(&mut self, client_root: &Path, track: &str) {
        if self.track.as_ref().is_some_and(|(playing, _)| playing == track) {
            return;
        }
        let path = client_root.join("music").join(format!("{track}.ogg"));
        let Some(bytes) = read_ogg(&path) else {
            eprintln!("music: {} could not be read", path.display());
            return;
        };
        self.music.clear();
        self.track = Some((track.to_owned(), bytes));
        self.start();
        self.music.play();
        println!("music: {track} at {}%", self.volume(Kind::Music));
    }

    /// Stops the music, leaving the ambient sounds playing.
    pub(crate) fn stop_music(&mut self) {
        if self.track.take().is_some() {
            self.music.clear();
            println!("music: stopped");
        }
    }

    /// Plays `sounds` in place of the ones playing now: those with no interval never stop, and the rest are
    /// called again after a wait of up to their interval.
    pub(crate) fn play_ambient(&mut self, sounds: Vec<Clip>) {
        self.ambient.clear();
        for clip in sounds {
            let player = Player::connect_new(self.device.mixer());
            player.set_volume(self.effects_gain(clip.gain));
            self.ambient.push(Ambient { player, clip, next: None });
        }
        self.tick();
    }

    /// Keeps the music playing and calls each ambient sound again when its turn comes.
    pub(crate) fn tick(&mut self) {
        if self.music.empty() {
            self.start();
        }
        let now = Instant::now();
        for ambient in &mut self.ambient {
            if !ambient.player.empty() {
                continue;
            }
            // A sound heard now and then waits, once it has played out, for a while of its own.
            if let Some(seconds) = ambient.clip.interval {
                match ambient.next {
                    Some(next) if next > now => continue,
                    Some(_) => ambient.next = None,
                    None => {
                        ambient.next = Some(now + Duration::from_secs_f32(seconds * wait()));
                        continue;
                    }
                }
            }
            match Decoder::new(Cursor::new(ambient.clip.bytes.clone())) {
                Ok(source) => ambient.player.append(source.speed(ambient.clip.pitch)),
                Err(error) => eprintln!("sound: {error}"),
            }
        }
    }

    /// The volume of `kind`, from 0 to 100.
    pub(crate) fn volume(&self, kind: Kind) -> u8 {
        self.volumes.get(kind as usize).copied().unwrap_or_default()
    }

    /// Whether everything is silenced.
    pub(crate) fn muted(&self) -> bool {
        self.muted
    }

    /// Silences everything, or lets it be heard again, and saves it.
    pub(crate) fn mute(&mut self, muted: bool) {
        self.muted = muted;
        self.apply();
        self.save();
    }

    /// Moves the volume of `kind` one step up or down and saves it.
    pub(crate) fn turn(&mut self, kind: Kind, up: bool) {
        let Some(volume) = self.volumes.get_mut(kind as usize) else { return };
        *volume = if up { volume.saturating_add(STEP).min(100) } else { volume.saturating_sub(STEP) };
        self.apply();
        self.save();
    }

    /// Queues the track from its beginning.
    fn start(&mut self) {
        let Some((name, bytes)) = &self.track else { return };
        match Decoder::new(Cursor::new(bytes.clone())) {
            Ok(source) => self.music.append(source),
            Err(error) => {
                eprintln!("music: {name} could not be decoded: {error}");
                self.track = None;
            }
        }
    }

    /// Sets what plays to the volumes now in force.
    fn apply(&self) {
        self.music.set_volume(gain(if self.muted { 0 } else { self.volume(Kind::Music) }));
        for ambient in &self.ambient {
            ambient.player.set_volume(self.effects_gain(ambient.clip.gain));
        }
    }

    /// How loudly a sound heard at `reach` of its full loudness plays.
    fn effects_gain(&self, reach: f32) -> f32 {
        if self.muted { 0.0 } else { gain(self.volume(Kind::Effects)) * reach }
    }

    fn save(&self) {
        let settings = format!(
            "music = {}\neffects = {}\nmuted = {}\n",
            self.volume(Kind::Music),
            self.volume(Kind::Effects),
            self.muted
        );
        if let Err(error) = std::fs::write(SETTINGS, settings) {
            eprintln!("settings: {error}");
        }
    }
}

/// The track in `path`, ready to decode; the client stores Ogg Vorbis with `L2SD` where `OggS` belongs.
fn read_ogg(path: &Path) -> Option<Vec<u8>> {
    let mut bytes = std::fs::read(path).ok()?;
    bytes.get_mut(..4)?.copy_from_slice(b"OggS");
    Some(bytes)
}

/// The volumes last saved and whether sound was silenced; half of full and heard where the file says nothing.
fn read_settings() -> ([u8; 2], bool) {
    let text = std::fs::read_to_string(SETTINGS).unwrap_or_default();
    let (mut volumes, mut muted) = ([50, 50], false);
    for (key, value) in text.lines().filter_map(|line| line.split_once('=')) {
        let value = value.trim();
        let index = match key.trim() {
            "music" => Kind::Music as usize,
            "effects" => Kind::Effects as usize,
            "muted" => {
                muted = value == "true";
                continue;
            }
            _ => continue,
        };
        if let (Some(volume), Ok(value)) = (volumes.get_mut(index), value.parse::<u8>()) {
            *volume = value.min(100);
        }
    }
    (volumes, muted)
}

/// A share of an interval to wait, from a quarter of it to all of it, so calls of the same sound spread out.
fn wait() -> f32 {
    let nanos = std::time::SystemTime::UNIX_EPOCH.elapsed().map_or(0, |since| since.subsec_nanos());
    0.25 + 0.75 * (f32::from(u16::try_from(nanos % 1000).unwrap_or(0)) / 1000.0)
}

/// A volume out of 100 as a gain, by loudness rather than amplitude: halfway sounds half as loud.
fn gain(volume: u8) -> f32 {
    (f32::from(volume) / 100.0).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volumes_move_in_steps_and_stop_at_the_ends() {
        let mut volumes = [100, 0];
        let turn = |volume: &mut u8, up: bool| {
            *volume = if up { volume.saturating_add(STEP).min(100) } else { volume.saturating_sub(STEP) };
        };
        turn(&mut volumes[0], true);
        turn(&mut volumes[1], false);
        assert_eq!(volumes, [100, 0], "full and silent stay where they are");
        turn(&mut volumes[0], false);
        assert_eq!(volumes[0], 90);
        assert!(gain(0) == 0.0 && (gain(100) - 1.0).abs() < f32::EPSILON && gain(50) < 0.5);
    }

    #[test]
    fn a_sound_heard_now_and_then_waits_a_part_of_its_interval() {
        // Waits shorter than a quarter would bunch the calls up; longer than the interval would drop them.
        assert!((0..50).map(|_| wait()).all(|wait| (0.25..=1.0).contains(&wait)));
    }
}
