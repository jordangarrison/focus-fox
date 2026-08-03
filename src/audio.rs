use std::f64::consts::TAU;
use std::num::{NonZeroU16, NonZeroU32};
use std::time::Duration;

use anyhow::{Context, Result};
use rodio::Player;
use rodio::source::Source;
use rodio::stream::{DeviceSinkBuilder, MixerDeviceSink};

use crate::config::ToneSettings;

const SAMPLE_RATE: u32 = 48_000;
const FADE_IN: Duration = Duration::from_millis(250);

/// Owns audio-device resources. Dropping the player and device sink stops
/// playback, so normal exit and error unwinding cannot leave tones running.
pub struct Audio {
    player: Option<Player>,
    device: Option<MixerDeviceSink>,
    requested_settings: Option<ToneSettings>,
    #[cfg(test)]
    fail_next_sync: bool,
}

impl Audio {
    pub fn new() -> Self {
        Self {
            player: None,
            device: None,
            requested_settings: None,
            #[cfg(test)]
            fail_next_sync: false,
        }
    }

    /// Match playback to requested tone settings. `None` fully closes
    /// the output device. Failed requests are remembered until state changes,
    /// avoiding a device-open attempt on every 100 ms TUI tick.
    pub fn sync(&mut self, settings: Option<ToneSettings>) -> Result<()> {
        if settings == self.requested_settings {
            return Ok(());
        }

        self.stop();
        self.requested_settings = settings;

        let Some(settings) = settings else {
            return Ok(());
        };

        #[cfg(test)]
        if std::mem::take(&mut self.fail_next_sync) {
            anyhow::bail!("test audio failure");
        }

        let mut device =
            DeviceSinkBuilder::open_default_sink().context("opening the default audio output")?;
        device.log_on_drop(false);
        let player = Player::connect_new(device.mixer());
        // Keep tone gain on its own player so future music playback can have
        // an independent volume control and player.
        player.set_volume(f32::from(settings.volume_percent) / 100.0);
        player.append(BinauralSource::new(settings).fade_in(FADE_IN));

        self.player = Some(player);
        self.device = Some(device);
        Ok(())
    }

    fn stop(&mut self) {
        self.player = None;
        self.device = None;
        self.requested_settings = None;
    }

    #[cfg(test)]
    pub fn fail_next_sync(&mut self) {
        self.fail_next_sync = true;
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.stop();
    }
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
        Some(sample as f32)
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
                .all(|sample| sample.abs() <= 1.0)
        );
    }
}
