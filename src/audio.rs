use std::f64::consts::TAU;
use std::num::{NonZeroU16, NonZeroU32};
use std::time::Duration;

use anyhow::{Context, Result};
use rodio::Player;
use rodio::source::Source;
use rodio::stream::{DeviceSinkBuilder, MixerDeviceSink};

const SAMPLE_RATE: u32 = 48_000;
const CARRIER_HZ: f64 = 220.0;
const TONE_AMPLITUDE: f64 = 0.08;
const FADE_IN: Duration = Duration::from_millis(250);

/// Owns audio-device resources. Dropping the player and device sink stops
/// playback, so normal exit and error unwinding cannot leave tones running.
pub struct Audio {
    player: Option<Player>,
    device: Option<MixerDeviceSink>,
    requested_beat_hz: Option<u16>,
}

impl Audio {
    pub fn new() -> Self {
        Self {
            player: None,
            device: None,
            requested_beat_hz: None,
        }
    }

    /// Match playback to the requested beat frequency. `None` fully closes
    /// the output device. Failed requests are remembered until state changes,
    /// avoiding a device-open attempt on every 100 ms TUI tick.
    pub fn sync(&mut self, beat_hz: Option<u16>) -> Result<()> {
        if beat_hz == self.requested_beat_hz {
            return Ok(());
        }

        self.stop();
        self.requested_beat_hz = beat_hz;

        let Some(beat_hz) = beat_hz else {
            return Ok(());
        };

        let mut device =
            DeviceSinkBuilder::open_default_sink().context("opening the default audio output")?;
        device.log_on_drop(false);
        let player = Player::connect_new(device.mixer());
        player.append(BinauralSource::new(beat_hz).fade_in(FADE_IN));

        self.player = Some(player);
        self.device = Some(device);
        Ok(())
    }

    fn stop(&mut self) {
        self.player = None;
        self.device = None;
        self.requested_beat_hz = None;
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
    fn new(beat_hz: u16) -> Self {
        Self {
            channel: 0,
            left_phase: 0.0,
            right_phase: 0.0,
            left_step: TAU * CARRIER_HZ / SAMPLE_RATE as f64,
            right_step: TAU * (CARRIER_HZ + f64::from(beat_hz)) / SAMPLE_RATE as f64,
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
        Some((sample * TONE_AMPLITUDE) as f32)
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
    fn source_is_stereo_with_expected_frequency_difference() {
        let source = BinauralSource::new(40);
        let (left, right) = source.frequencies();

        assert_eq!(source.channels().get(), 2);
        assert_eq!(source.sample_rate().get(), SAMPLE_RATE);
        assert!((left - CARRIER_HZ).abs() < f64::EPSILON);
        assert!((right - left - 40.0).abs() < f64::EPSILON);
    }

    #[test]
    fn generated_samples_stay_at_safe_amplitude() {
        let mut source = BinauralSource::new(40);

        assert!(
            source
                .by_ref()
                .take(SAMPLE_RATE as usize * 2)
                .all(|sample| sample.abs() <= TONE_AMPLITUDE as f32)
        );
    }
}
