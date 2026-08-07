use std::f64::consts::TAU;
use std::num::{NonZeroU16, NonZeroU32};
use std::time::Duration;

use anyhow::{Context, Result};
use rodio::Player;
use rodio::source::Source;
use rodio::stream::{DeviceSinkBuilder, MixerDeviceSink};

use crate::config::{MusicSettings, MusicSource, ToneSettings};

mod lofi;

use lofi::{LofiSource, MusicParams};

pub(crate) const SAMPLE_RATE: u32 = 48_000;
const FADE_IN: Duration = Duration::from_millis(250);

/// Calibrated loudness: both channels target this long-run RMS at 100%
/// volume (-16 dBFS), so equal volume percentages sound equally loud. The
/// tone scales its sine down to it; the music drives its mix up into a
/// saturator to reach it (see `lofi::SATURATOR_DRIVE`).
pub(crate) const REFERENCE_RMS: f32 = 0.158;

/// Sine amplitude whose RMS equals `REFERENCE_RMS`.
const TONE_AMPLITUDE: f32 = REFERENCE_RMS * std::f32::consts::SQRT_2;

/// Everything the audio layer should be playing right now. `None` per
/// channel means silence for that channel.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct AudioTargets {
    pub tone: Option<ToneSettings>,
    pub music: Option<MusicSettings>,
}

/// What to do with one channel's player. Pure decision, exercised without
/// an audio device in tests.
#[derive(Debug, PartialEq, Eq)]
enum ChannelAction {
    Stop,
    SetVolume,
    Rebuild,
}

fn plan_channel<T: Copy>(
    prev: Option<T>,
    next: Option<T>,
    has_player: bool,
    same: impl Fn(T, T) -> bool,
) -> ChannelAction {
    match (prev, next) {
        (_, None) => ChannelAction::Stop,
        (Some(prev), Some(next)) if has_player && same(prev, next) => ChannelAction::SetVolume,
        (_, Some(_)) => ChannelAction::Rebuild,
    }
}

/// Owns audio-device resources: one shared output device with independent
/// tone and music players on its mixer. Dropping the players and device sink
/// stops playback, so normal exit and error unwinding cannot leave audio
/// running. Volume-only changes re-gain a player in place; anything else
/// rebuilds only the affected channel.
pub struct Audio {
    device: Option<MixerDeviceSink>,
    tone_player: Option<Player>,
    music_player: Option<Player>,
    requested: AudioTargets,
    #[cfg(test)]
    fail_next_sync: bool,
}

impl Audio {
    pub fn new() -> Self {
        Self {
            device: None,
            tone_player: None,
            music_player: None,
            requested: AudioTargets::default(),
            #[cfg(test)]
            fail_next_sync: false,
        }
    }

    /// Match playback to the requested targets. Both channels `None` fully
    /// closes the output device. Failed requests are remembered until state
    /// changes, avoiding a device-open attempt on every 100 ms TUI tick.
    pub fn sync(&mut self, targets: AudioTargets) -> Result<()> {
        if targets == self.requested {
            return Ok(());
        }

        let prev = std::mem::replace(&mut self.requested, targets);

        if targets.tone.is_none() && targets.music.is_none() {
            self.close();
            return Ok(());
        }

        #[cfg(test)]
        if std::mem::take(&mut self.fail_next_sync) {
            self.close();
            anyhow::bail!("test audio failure");
        }

        if self.device.is_none() {
            // Any players that survived a failed open belong to a dead mixer.
            self.tone_player = None;
            self.music_player = None;
            let mut device = DeviceSinkBuilder::open_default_sink()
                .context("opening the default audio output")?;
            device.log_on_drop(false);
            self.device = Some(device);
        }

        self.sync_tone(prev.tone, targets.tone);
        self.sync_music(prev.music, targets.music);
        Ok(())
    }

    fn sync_tone(&mut self, prev: Option<ToneSettings>, next: Option<ToneSettings>) {
        match plan_channel(prev, next, self.tone_player.is_some(), ToneSettings::same_tone) {
            ChannelAction::Stop => self.tone_player = None,
            ChannelAction::SetVolume => {
                let settings = next.expect("SetVolume implies a target");
                if let Some(player) = &self.tone_player {
                    player.set_volume(f32::from(settings.volume_percent) / 100.0);
                }
            }
            ChannelAction::Rebuild => {
                let settings = next.expect("Rebuild implies a target");
                let device = self.device.as_ref().expect("device open while playing");
                let player = Player::connect_new(device.mixer());
                player.set_volume(f32::from(settings.volume_percent) / 100.0);
                player.append(BinauralSource::new(settings).fade_in(FADE_IN));
                self.tone_player = Some(player);
            }
        }
    }

    fn sync_music(&mut self, prev: Option<MusicSettings>, next: Option<MusicSettings>) {
        match plan_channel(prev, next, self.music_player.is_some(), MusicSettings::same_track) {
            ChannelAction::Stop => self.music_player = None,
            ChannelAction::SetVolume => {
                let settings = next.expect("SetVolume implies a target");
                if let Some(player) = &self.music_player {
                    player.set_volume(f32::from(settings.volume_percent) / 100.0);
                }
            }
            ChannelAction::Rebuild => {
                let settings = next.expect("Rebuild implies a target");
                let device = self.device.as_ref().expect("device open while playing");
                let player = Player::connect_new(device.mixer());
                player.set_volume(f32::from(settings.volume_percent) / 100.0);
                // The seed lives outside the compared settings so change
                // detection never churns; each rebuild is a fresh performance.
                match settings.source {
                    MusicSource::Lofi => player.append(
                        LofiSource::new(MusicParams::derive(settings, fresh_seed()))
                            .fade_in(FADE_IN),
                    ),
                }
                self.music_player = Some(player);
            }
        }
    }

    fn close(&mut self) {
        self.tone_player = None;
        self.music_player = None;
        self.device = None;
    }

    #[cfg(test)]
    pub fn fail_next_sync(&mut self) {
        self.fail_next_sync = true;
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.close();
    }
}

fn fresh_seed() -> u64 {
    use std::hash::{BuildHasher, RandomState};
    RandomState::new().hash_one(0u64)
}

/// Infinite interleaved stereo source: carrier in the left ear, carrier plus
/// configured beat frequency in the right. Independent phase accumulators
/// avoid precision drift during long work sessions.
struct BinauralSource {
    channel: u8,
    left_phase: f64,
    right_phase: f64,
    left_step: f64,
    right_step: f64,
}

impl BinauralSource {
    fn new(settings: ToneSettings) -> Self {
        Self {
            channel: 0,
            left_phase: 0.0,
            right_phase: 0.0,
            left_step: TAU * f64::from(settings.base_hz) / SAMPLE_RATE as f64,
            right_step: TAU * f64::from(settings.base_hz + settings.beat_hz) / SAMPLE_RATE as f64,
        }
    }

    #[cfg(test)]
    fn frequencies(&self) -> (f64, f64) {
        (
            self.left_step * SAMPLE_RATE as f64 / TAU,
            self.right_step * SAMPLE_RATE as f64 / TAU,
        )
    }
}

impl Iterator for BinauralSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = if self.channel == 0 {
            let sample = self.left_phase.sin();
            self.left_phase = (self.left_phase + self.left_step).rem_euclid(TAU);
            self.channel = 1;
            sample
        } else {
            let sample = self.right_phase.sin();
            self.right_phase = (self.right_phase + self.right_step).rem_euclid(TAU);
            self.channel = 0;
            sample
        };
        Some(sample as f32 * TONE_AMPLITUDE)
    }
}

impl Source for BinauralSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        NonZeroU16::new(2).expect("stereo channel count is non-zero")
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        NonZeroU32::new(SAMPLE_RATE).expect("sample rate is non-zero")
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_channel_covers_every_transition() {
        let same = |a: u8, b: u8| a == b;
        // Any request for silence stops the channel.
        assert_eq!(plan_channel(Some(1), None, true, same), ChannelAction::Stop);
        assert_eq!(plan_channel(None, None, false, same), ChannelAction::Stop);
        // Same track with a live player only needs a gain change.
        assert_eq!(
            plan_channel(Some(1), Some(1), true, same),
            ChannelAction::SetVolume
        );
        // A different track rebuilds.
        assert_eq!(
            plan_channel(Some(1), Some(2), true, same),
            ChannelAction::Rebuild
        );
        // Starting from silence, or recovering from a failed open where no
        // player exists, rebuilds even if the settings match.
        assert_eq!(
            plan_channel(None, Some(1), false, same),
            ChannelAction::Rebuild
        );
        assert_eq!(
            plan_channel(Some(1), Some(1), false, same),
            ChannelAction::Rebuild
        );
    }

    #[test]
    fn source_is_stereo_with_expected_frequencies() {
        for settings in [
            ToneSettings {
                base_hz: 220,
                beat_hz: 18,
                volume_percent: 8,
            },
            ToneSettings {
                base_hz: 320,
                beat_hz: 40,
                volume_percent: 8,
            },
            ToneSettings {
                base_hz: 470,
                beat_hz: 13,
                volume_percent: 37,
            },
        ] {
            let source = BinauralSource::new(settings);
            let (left, right) = source.frequencies();
            assert_eq!(source.channels().get(), 2);
            assert_eq!(source.sample_rate().get(), SAMPLE_RATE);
            assert!((left - f64::from(settings.base_hz)).abs() < 1e-10);
            assert!((right - left - f64::from(settings.beat_hz)).abs() < 1e-10);
        }
    }

    #[test]
    fn generated_samples_stay_at_safe_amplitude() {
        let mut source = BinauralSource::new(ToneSettings {
            base_hz: 220,
            beat_hz: 40,
            volume_percent: 100,
        });

        assert!(
            source
                .by_ref()
                .take(SAMPLE_RATE as usize * 2)
                .all(|sample| sample.abs() <= TONE_AMPLITUDE)
        );
    }

    #[test]
    fn tone_rms_matches_the_reference_level() {
        let mut source = BinauralSource::new(ToneSettings {
            base_hz: 220,
            beat_hz: 40,
            volume_percent: 100,
        });
        let samples: Vec<f32> = source.by_ref().take(SAMPLE_RATE as usize * 2 * 5).collect();
        let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
        assert!(
            (rms - REFERENCE_RMS).abs() < REFERENCE_RMS * 0.02,
            "tone rms {rms} vs reference {REFERENCE_RMS}"
        );
    }
}
