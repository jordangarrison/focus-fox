//! Procedural lofi music engine. Endless seeded synthesis — no assets,
//! no decoders, no network. The mood comes from the listening preset and
//! the key/tempo from the tone settings, so the music always fits the
//! binaural beats playing beside it.

use std::f64::consts::TAU;
use std::num::{NonZeroU16, NonZeroU32};
use std::time::Duration;

use rodio::source::Source;

use crate::config::{BinauralPreset, MusicSettings};

use super::SAMPLE_RATE;

const FRAMES_PER_SEC: f64 = SAMPLE_RATE as f64;

// Voice gain budget: these set the balance between voices; absolute
// loudness comes from the saturator stage below.
const PIANO_GAIN: f32 = 0.11;
const MELODY_GAIN: f32 = 0.10;
const BASS_GAIN: f32 = 0.16;
const KICK_GAIN: f32 = 0.20;
const HAT_GAIN: f32 = 0.06;
const CRACKLE_GAIN: f32 = 0.065;

// Drive into the tape-style tanh saturator, calibrated (see the calibration
// test) so long-run RMS lands on `REFERENCE_RMS` — the same loudness the
// binaural tone produces at equal volume. tanh bounds every sample within
// ±1 no matter what the voices sum to, while gently squashing kick and
// chord transients the way tape would.
const SATURATOR_DRIVE: f32 = 8.0;

// Post-saturator peak ceiling. The tone and music sum on one mixer with no
// limiter downstream, so the music's peak must leave room for the tone's:
// MUSIC_PEAK_CEILING + TONE_AMPLITUDE < 1.0 keeps the worst-case sum (both
// channels at 100%) below full scale. Enforced by a test.
pub(super) const MUSIC_PEAK_CEILING: f32 = 0.77;

// Tape-style pitch wobble shared by the melodic voices.
const WOW_HZ: f64 = 0.4;
const WOW_DEPTH: f64 = 0.002;

const PIANO_POOL: usize = 8;
const NOTE_ATTACK_SECS: f32 = 0.003;

/// Deterministic SplitMix64. A dependency-free stand-in for `rand`: every
/// musical choice flows from the seed, which keeps the engine unit-testable.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn chance(&mut self, probability: f32) -> bool {
        self.next_f32() < probability
    }

    fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.next_f32()
    }

    fn pick(&mut self, len: usize) -> usize {
        (self.next_u64() % len as u64) as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mood {
    Bright,
    Soft,
    Ambient,
}

/// The musical brief derived from the audio settings: what key, what tempo,
/// what character. Pure data; `derive` is the only constructor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MusicParams {
    root_midi: u8,
    bpm: f32,
    mood: Mood,
    seed: u64,
}

impl MusicParams {
    pub(crate) fn derive(settings: MusicSettings, seed: u64) -> Self {
        let mood = match settings.preset {
            BinauralPreset::ActiveFocus
            | BinauralPreset::GammaExperiment
            | BinauralPreset::ResearchGamma => Mood::Bright,
            BinauralPreset::CalmConcentration
            | BinauralPreset::Meditative
            | BinauralPreset::Custom => Mood::Soft,
            BinauralPreset::WindDown => Mood::Ambient,
        };

        // Same pitch class as the carrier tone, folded into the A2–A3
        // register so the key stays comfortable for any base frequency.
        let midi = (69.0 + 12.0 * (f64::from(settings.base_hz) / 440.0).log2()).round() as i32;
        let mut root_midi = midi;
        while root_midi < 45 {
            root_midi += 12;
        }
        while root_midi > 57 {
            root_midi -= 12;
        }

        // Octave-fold the beat frequency into lofi tempo territory. The
        // config layer clamps beat_hz to ≥1, but guard locally too — a zero
        // would never leave this loop.
        let mut bpm = f32::from(settings.beat_hz.max(1));
        while bpm < 45.0 {
            bpm *= 2.0;
        }
        while bpm >= 90.0 {
            bpm /= 2.0;
        }
        let bpm = bpm.clamp(60.0, 88.0);

        Self {
            root_midi: root_midi as u8,
            bpm,
            mood,
            seed,
        }
    }
}

/// A chord voiced as semitone offsets from the root, plus its bass note.
struct Chord {
    notes: [i8; 4],
    bass: i8,
}

// Two precomposed minor progressions; the seed picks one per session.
// Randomness only ever selects among vetted material like this — it never
// invents notes.
const PROGRESSIONS: [[Chord; 4]; 2] = [
    // i9 → VImaj7 → IIImaj7 → VII7
    [
        Chord { notes: [0, 3, 7, 14], bass: 0 },
        Chord { notes: [-4, 0, 3, 7], bass: -4 },
        Chord { notes: [3, 7, 10, 14], bass: 3 },
        Chord { notes: [-2, 2, 5, 10], bass: -2 },
    ],
    // i7 → iv7 → VImaj7 → V7
    [
        Chord { notes: [0, 3, 7, 10], bass: 0 },
        Chord { notes: [5, 8, 12, 15], bass: 5 },
        Chord { notes: [-4, 0, 3, 8], bass: -4 },
        Chord { notes: [-1, 2, 7, 11], bass: -5 },
    ],
];

// Minor pentatonic offsets above the root, one octave up.
const MELODY_OFFSETS: [i8; 5] = [12, 15, 17, 19, 22];

/// Per-mood tuning: rhythmic density, brightness, envelope shapes.
struct MoodParams {
    beats_per_bar: u32,
    second_hit_prob: f32,
    hat_prob_down: f32,
    hat_prob_off: f32,
    kick_every_bar: bool,
    kick_alternate_bars: bool,
    kick_beat3_prob: f32,
    melody_prob: f32,
    chord_attack_secs: f32,
    piano_decay_secs: f32,
    second_harmonic: f32,
    cutoff_hz: f64,
}

fn mood_params(mood: Mood) -> MoodParams {
    match mood {
        Mood::Bright => MoodParams {
            beats_per_bar: 4,
            second_hit_prob: 0.6,
            hat_prob_down: 0.3,
            hat_prob_off: 0.8,
            kick_every_bar: true,
            kick_alternate_bars: false,
            kick_beat3_prob: 0.5,
            melody_prob: 0.3,
            chord_attack_secs: NOTE_ATTACK_SECS,
            piano_decay_secs: 2.0,
            second_harmonic: 0.25,
            cutoff_hz: 5_000.0,
        },
        Mood::Soft => MoodParams {
            beats_per_bar: 4,
            second_hit_prob: 0.25,
            hat_prob_down: 0.15,
            hat_prob_off: 0.4,
            kick_every_bar: false,
            kick_alternate_bars: true,
            kick_beat3_prob: 0.0,
            melody_prob: 0.15,
            chord_attack_secs: NOTE_ATTACK_SECS,
            piano_decay_secs: 2.5,
            second_harmonic: 0.15,
            cutoff_hz: 3_500.0,
        },
        Mood::Ambient => MoodParams {
            beats_per_bar: 8,
            second_hit_prob: 0.0,
            hat_prob_down: 0.0,
            hat_prob_off: 0.0,
            kick_every_bar: false,
            kick_alternate_bars: false,
            kick_beat3_prob: 0.0,
            melody_prob: 0.0,
            chord_attack_secs: 0.5,
            piano_decay_secs: 4.0,
            second_harmonic: 0.15,
            cutoff_hz: 2_500.0,
        },
    }
}

fn midi_to_hz(midi: f64) -> f64 {
    440.0 * ((midi - 69.0) / 12.0).exp2()
}

/// Exponential-decay multiplier reaching -40 dB after `seconds`.
fn decay_per_frame(seconds: f32) -> f32 {
    0.01f32.powf(1.0 / (seconds * SAMPLE_RATE as f32))
}

/// One melodic voice: short linear attack (kills retrigger clicks), then
/// exponential decay. `sample` returns mono; the caller applies the pan.
#[derive(Default)]
struct Pluck {
    phase: f64,
    step: f64,
    env: f32,
    peak: f32,
    attack_inc: f32,
    attack_left: u32,
    decay: f32,
    delay: u32,
    pan_l: f32,
    pan_r: f32,
}

impl Pluck {
    #[allow(clippy::too_many_arguments)]
    fn trigger(&mut self, freq: f64, peak: f32, attack_frames: u32, decay: f32, pan: f32, delay: u32) {
        let attack_frames = attack_frames.max(1);
        self.phase = 0.0;
        self.step = TAU * freq / FRAMES_PER_SEC;
        self.env = 0.0;
        self.peak = peak;
        self.attack_inc = peak / attack_frames as f32;
        self.attack_left = attack_frames;
        self.decay = decay;
        self.delay = delay;
        // Constant-power pan; each side stays ≤ 1 so gain budgeting holds.
        self.pan_l = ((1.0 - pan) / 2.0).sqrt();
        self.pan_r = ((1.0 + pan) / 2.0).sqrt();
    }

    fn sample(&mut self, wow: f64, second_harmonic: f32) -> f32 {
        if self.delay > 0 {
            self.delay -= 1;
            return 0.0;
        }
        if self.attack_left > 0 {
            self.attack_left -= 1;
            self.env = (self.env + self.attack_inc).min(self.peak);
        } else if self.env < 1e-5 {
            // Flush before the multiply: a decaying-forever envelope would
            // stall in the subnormal range and grind denormal arithmetic on
            // the audio thread.
            self.env = 0.0;
            return 0.0;
        } else {
            self.env *= self.decay;
        }
        // Fundamental plus a touch of second harmonic ≈ a mellow e-piano;
        // normalized so the waveform stays within ±1.
        let wave = (self.phase.sin() as f32 + second_harmonic * (2.0 * self.phase).sin() as f32)
            / (1.0 + second_harmonic);
        self.phase = (self.phase + self.step * wow).rem_euclid(TAU);
        wave * self.env
    }
}

/// Pitch-swept sine thump: the oscillator step itself decays, dropping the
/// pitch from 110 Hz — a cheap 808-style kick.
#[derive(Default)]
struct KickDrum {
    phase: f64,
    step: f64,
    env: f32,
    attack_left: u32,
    decay: f32,
}

impl KickDrum {
    fn trigger(&mut self) {
        self.phase = 0.0;
        self.step = TAU * 110.0 / FRAMES_PER_SEC;
        self.env = 0.0;
        self.attack_left = (0.002 * SAMPLE_RATE as f32) as u32;
        self.decay = decay_per_frame(0.15);
    }

    fn sample(&mut self) -> f32 {
        if self.attack_left > 0 {
            self.attack_left -= 1;
            self.env = (self.env + 1.0 / (0.002 * SAMPLE_RATE as f32)).min(1.0);
        } else if self.env < 1e-5 {
            // Flush before the multiply — see Pluck::sample.
            self.env = 0.0;
            return 0.0;
        } else {
            self.env *= self.decay;
        }
        let wave = self.phase.sin() as f32;
        self.phase = (self.phase + self.step).rem_euclid(TAU);
        self.step *= 0.9998;
        wave * self.env
    }
}

/// Short burst of highpassed white noise.
#[derive(Default)]
struct HatCymbal {
    env: f32,
    lowpass: f32,
    decay: f32,
}

impl HatCymbal {
    fn trigger(&mut self, peak: f32) {
        self.env = peak;
        self.decay = decay_per_frame(0.03);
    }

    fn sample(&mut self, rng: &mut Rng) -> f32 {
        if self.env < 1e-5 {
            return 0.0;
        }
        let white = rng.range(-1.0, 1.0);
        self.lowpass += 0.1 * (white - self.lowpass);
        let sample = (white - self.lowpass) * self.env;
        self.env *= self.decay;
        sample
    }
}

/// Vinyl texture: rare random clicks over a faint constant noise bed.
#[derive(Default)]
struct Crackle {
    click_env: f32,
    click_decay: f32,
}

impl Crackle {
    fn sample(&mut self, rng: &mut Rng) -> f32 {
        if rng.chance(0.0002) {
            self.click_env = rng.range(0.3, 1.0);
            self.click_decay = decay_per_frame(0.002);
        }
        let mut sample = rng.range(-1.0, 1.0) * 0.08;
        if self.click_env > 1e-4 {
            sample += rng.range(-1.0, 1.0) * self.click_env;
            self.click_env *= self.click_decay;
        }
        sample
    }
}

/// Infinite stereo lofi performance. Harmony cycles every four bars, but the
/// seeded random walk over density, velocity, timing, and melody never
/// repeats state, so the output never audibly loops.
pub(crate) struct LofiSource {
    rng: Rng,
    mood: MoodParams,
    // Timing grid: integer frame counters, no float drift.
    frames_per_beat: u32,
    frames_per_bar: u32,
    frame_in_bar: u32,
    bar: u32,
    // Composition state.
    progression: &'static [Chord; 4],
    root_midi: u8,
    piano_decay: f32,
    chord_attack_frames: u32,
    second_hit: bool,
    kick_on_3: bool,
    melody_idx: usize,
    // Voices.
    piano: [Pluck; PIANO_POOL],
    next_piano: usize,
    melody: Pluck,
    bass: Pluck,
    kick: KickDrum,
    hat: HatCymbal,
    crackle: Crackle,
    // Lofi character.
    wow_phase: f64,
    lowpass_k: f32,
    lowpass_l: f32,
    lowpass_r: f32,
    // Stereo interleave: frames render as (l, r) pairs.
    pending_right: Option<f32>,
}

impl LofiSource {
    pub(crate) fn new(params: MusicParams) -> Self {
        let mood = mood_params(params.mood);
        let mut rng = Rng::new(params.seed);
        let progression = &PROGRESSIONS[rng.pick(PROGRESSIONS.len())];
        // Integer rounding drifts tempo <0.01% — inaudible.
        let frames_per_beat = (FRAMES_PER_SEC * 60.0 / f64::from(params.bpm)) as u32;
        let frames_per_bar = frames_per_beat * mood.beats_per_bar;
        let lowpass_k = 1.0 - (-TAU * mood.cutoff_hz / FRAMES_PER_SEC).exp() as f32;
        let piano_decay = decay_per_frame(mood.piano_decay_secs);
        let chord_attack_frames = (mood.chord_attack_secs * SAMPLE_RATE as f32) as u32;
        Self {
            rng,
            mood,
            frames_per_beat,
            frames_per_bar,
            frame_in_bar: 0,
            bar: 0,
            progression,
            root_midi: params.root_midi,
            piano_decay,
            chord_attack_frames,
            second_hit: false,
            kick_on_3: false,
            melody_idx: 0,
            piano: Default::default(),
            next_piano: 0,
            melody: Pluck::default(),
            bass: Pluck::default(),
            kick: KickDrum::default(),
            hat: HatCymbal::default(),
            crackle: Crackle::default(),
            wow_phase: 0.0,
            lowpass_k,
            lowpass_l: 0.0,
            lowpass_r: 0.0,
            pending_right: None,
        }
    }

    #[cfg(test)]
    fn grid(&self) -> (u32, u32) {
        (self.frames_per_beat, self.frames_per_bar)
    }

    fn current_chord(&self) -> &'static Chord {
        &self.progression[(self.bar % 4) as usize]
    }

    fn trigger_chord(&mut self) {
        const PANS: [f32; 4] = [-0.3, -0.1, 0.1, 0.3];
        let notes = self.current_chord().notes;
        for (index, offset) in notes.into_iter().enumerate() {
            let detune = 1.0 + f64::from(self.rng.range(-0.003, 0.003));
            let freq = midi_to_hz(f64::from(self.root_midi) + f64::from(offset)) * detune;
            let peak = self.rng.range(0.7, 1.0);
            let delay = (self.rng.next_f32() * 0.015 * SAMPLE_RATE as f32) as u32;
            let attack = self.chord_attack_frames;
            let decay = self.piano_decay;
            let voice = &mut self.piano[self.next_piano];
            voice.trigger(freq, peak, attack, decay, PANS[index], delay);
            self.next_piano = (self.next_piano + 1) % PIANO_POOL;
        }
    }

    fn trigger_melody(&mut self) {
        // Bias toward stepping to a neighbor of the previous note.
        self.melody_idx = if self.rng.chance(0.6) {
            if self.rng.chance(0.5) {
                self.melody_idx.saturating_sub(1)
            } else {
                (self.melody_idx + 1).min(MELODY_OFFSETS.len() - 1)
            }
        } else {
            self.rng.pick(MELODY_OFFSETS.len())
        };
        let offset = MELODY_OFFSETS[self.melody_idx];
        let detune = 1.0 + f64::from(self.rng.range(-0.003, 0.003));
        let freq = midi_to_hz(f64::from(self.root_midi) + f64::from(offset)) * detune;
        let peak = self.rng.range(0.7, 1.0);
        let pan = self.rng.range(-0.2, 0.2);
        let delay = (self.rng.next_f32() * 0.015 * SAMPLE_RATE as f32) as u32;
        let attack = (NOTE_ATTACK_SECS * SAMPLE_RATE as f32) as u32;
        self.melody
            .trigger(freq, peak, attack, decay_per_frame(1.5), pan, delay);
    }

    fn begin_bar(&mut self) {
        self.trigger_chord();

        let bass_midi = f64::from(self.root_midi) - 12.0 + f64::from(self.current_chord().bass);
        let peak = self.rng.range(0.8, 1.0);
        let attack = self.chord_attack_frames;
        self.bass
            .trigger(midi_to_hz(bass_midi), peak, attack, decay_per_frame(1.0), 0.0, 0);

        if self.mood.kick_every_bar || (self.mood.kick_alternate_bars && self.bar.is_multiple_of(2)) {
            self.kick.trigger();
        }
        self.second_hit = self.rng.chance(self.mood.second_hit_prob);
        self.kick_on_3 = self.rng.chance(self.mood.kick_beat3_prob);
    }

    /// Fire any events landing on this frame, then advance the grid.
    fn schedule(&mut self) {
        if self.frame_in_bar == 0 {
            self.begin_bar();
        }
        let beat = self.frame_in_bar / self.frames_per_beat;
        let frame_in_beat = self.frame_in_bar % self.frames_per_beat;

        if frame_in_beat == 0 && beat == 2 {
            if self.second_hit {
                self.trigger_chord();
            }
            if self.kick_on_3 {
                self.kick.trigger();
            }
        }

        let eighth_len = self.frames_per_beat / 2;
        if self.mood.hat_prob_off > 0.0 && self.frame_in_bar.is_multiple_of(eighth_len) {
            let eighth = self.frame_in_bar / eighth_len;
            let probability = if eighth.is_multiple_of(2) {
                self.mood.hat_prob_down
            } else {
                self.mood.hat_prob_off
            };
            if self.rng.chance(probability) {
                self.hat.trigger(self.rng.range(0.6, 1.0));
            }
        }

        if frame_in_beat == 0
            && (beat == 0 || beat == 2)
            && self.rng.chance(self.mood.melody_prob)
        {
            self.trigger_melody();
        }

        self.frame_in_bar += 1;
        if self.frame_in_bar >= self.frames_per_bar {
            self.frame_in_bar = 0;
            self.bar += 1;
        }
    }

    fn render_frame(&mut self) -> (f32, f32) {
        self.schedule();

        self.wow_phase = (self.wow_phase + TAU * WOW_HZ / FRAMES_PER_SEC).rem_euclid(TAU);
        let wow = 1.0 + WOW_DEPTH * self.wow_phase.sin();
        let second_harmonic = self.mood.second_harmonic;

        let (mut left, mut right) = (0.0f32, 0.0f32);
        for voice in &mut self.piano {
            let sample = voice.sample(wow, second_harmonic) * PIANO_GAIN;
            left += sample * voice.pan_l;
            right += sample * voice.pan_r;
        }
        let sample = self.melody.sample(wow, second_harmonic) * MELODY_GAIN;
        left += sample * self.melody.pan_l;
        right += sample * self.melody.pan_r;

        let center = std::f32::consts::FRAC_1_SQRT_2;
        let sample = self.bass.sample(1.0, 0.0) * BASS_GAIN;
        left += sample * center;
        right += sample * center;
        let sample = self.kick.sample() * KICK_GAIN;
        left += sample * center;
        right += sample * center;
        let sample = self.hat.sample(&mut self.rng) * HAT_GAIN;
        left += sample * 0.55;
        right += sample * 0.83;
        let sample = self.crackle.sample(&mut self.rng) * CRACKLE_GAIN;
        left += sample * center;
        right += sample * center;

        self.lowpass_l += self.lowpass_k * (left - self.lowpass_l);
        self.lowpass_r += self.lowpass_k * (right - self.lowpass_r);
        (
            (self.lowpass_l * SATURATOR_DRIVE).tanh() * MUSIC_PEAK_CEILING,
            (self.lowpass_r * SATURATOR_DRIVE).tanh() * MUSIC_PEAK_CEILING,
        )
    }
}

impl Iterator for LofiSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(right) = self.pending_right.take() {
            return Some(right);
        }
        let (left, right) = self.render_frame();
        self.pending_right = Some(right);
        Some(left)
    }
}

impl Source for LofiSource {
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
    use crate::audio::REFERENCE_RMS;
    use crate::config::MusicSource;

    fn settings(preset: BinauralPreset, base_hz: u16, beat_hz: u16) -> MusicSettings {
        MusicSettings {
            source: MusicSource::Lofi,
            preset,
            base_hz,
            beat_hz,
            volume_percent: 40,
        }
    }

    fn params(preset: BinauralPreset, seed: u64) -> MusicParams {
        MusicParams::derive(settings(preset, 220, 40), seed)
    }

    #[test]
    fn presets_map_to_expected_moods() {
        let cases = [
            (BinauralPreset::ActiveFocus, Mood::Bright),
            (BinauralPreset::GammaExperiment, Mood::Bright),
            (BinauralPreset::ResearchGamma, Mood::Bright),
            (BinauralPreset::CalmConcentration, Mood::Soft),
            (BinauralPreset::Meditative, Mood::Soft),
            (BinauralPreset::Custom, Mood::Soft),
            (BinauralPreset::WindDown, Mood::Ambient),
        ];
        for (preset, mood) in cases {
            assert_eq!(MusicParams::derive(settings(preset, 220, 40), 1).mood, mood);
        }
    }

    #[test]
    fn root_stays_in_register_and_keeps_carrier_pitch_class() {
        for base_hz in [100u16, 220, 470, 1000] {
            let derived = MusicParams::derive(settings(BinauralPreset::Custom, base_hz, 40), 1);
            assert!((45..=57).contains(&derived.root_midi), "base {base_hz}");
            let carrier_midi =
                (69.0 + 12.0 * (f64::from(base_hz) / 440.0).log2()).round() as i32;
            assert_eq!(
                (i32::from(derived.root_midi) - carrier_midi).rem_euclid(12),
                0,
                "base {base_hz}"
            );
        }
    }

    #[test]
    fn bpm_folds_into_lofi_range() {
        // 0 is below the config clamp but must not hang the fold.
        for beat_hz in [0u16, 1, 3, 6, 18, 40, 100] {
            let derived = MusicParams::derive(settings(BinauralPreset::Custom, 220, beat_hz), 1);
            assert!(
                (60.0..=88.0).contains(&derived.bpm),
                "beat {beat_hz} → {}",
                derived.bpm
            );
        }
        assert_eq!(
            MusicParams::derive(settings(BinauralPreset::Custom, 220, 40), 1).bpm,
            80.0
        );
        assert_eq!(
            MusicParams::derive(settings(BinauralPreset::Custom, 220, 18), 1).bpm,
            72.0
        );
    }

    #[test]
    fn source_format_matches_binaural_source() {
        let source = LofiSource::new(params(BinauralPreset::GammaExperiment, 1));
        assert_eq!(source.channels().get(), 2);
        assert_eq!(source.sample_rate().get(), SAMPLE_RATE);
        assert_eq!(source.current_span_len(), None);
        assert_eq!(source.total_duration(), None);
    }

    #[test]
    fn timing_grid_matches_bpm() {
        for (beat_hz, expected_frames_per_beat) in [(40u16, 36_000u32), (18, 40_000)] {
            let source =
                LofiSource::new(MusicParams::derive(settings(BinauralPreset::Custom, 220, beat_hz), 1));
            let (frames_per_beat, frames_per_bar) = source.grid();
            assert_eq!(frames_per_beat, expected_frames_per_beat);
            assert_eq!(frames_per_bar, frames_per_beat * 4);
        }
        let ambient =
            LofiSource::new(MusicParams::derive(settings(BinauralPreset::WindDown, 220, 3), 1));
        let (frames_per_beat, frames_per_bar) = ambient.grid();
        assert_eq!(frames_per_bar, frames_per_beat * 8);
    }

    #[test]
    fn same_seed_is_deterministic_and_different_seeds_diverge() {
        let take = 100_000;
        let a: Vec<f32> = LofiSource::new(params(BinauralPreset::GammaExperiment, 7))
            .take(take)
            .collect();
        let b: Vec<f32> = LofiSource::new(params(BinauralPreset::GammaExperiment, 7))
            .take(take)
            .collect();
        assert_eq!(a, b);

        let c: Vec<f32> = LofiSource::new(params(BinauralPreset::GammaExperiment, 8))
            .take(take)
            .collect();
        assert_ne!(a, c);
    }

    #[test]
    fn output_stays_within_headroom_for_every_mood() {
        for preset in [
            BinauralPreset::GammaExperiment,
            BinauralPreset::Meditative,
            BinauralPreset::WindDown,
        ] {
            for seed in [1u64, 99] {
                let mut source = LofiSource::new(params(preset, seed));
                let frames = SAMPLE_RATE as usize * 20;
                // tanh bounds every sample below the ceiling regardless of
                // what the voices sum to.
                assert!(
                    source
                        .by_ref()
                        .take(frames * 2)
                        .all(|s| s.abs() < MUSIC_PEAK_CEILING),
                    "{preset:?} seed {seed} exceeded headroom"
                );
            }
        }
    }

    #[test]
    fn every_mood_is_calibrated_to_the_reference_loudness() {
        for preset in [
            BinauralPreset::GammaExperiment,
            BinauralPreset::Meditative,
            BinauralPreset::WindDown,
        ] {
            for seed in [1u64, 42, 99] {
                let mut source = LofiSource::new(params(preset, seed));
                let samples: Vec<f32> =
                    source.by_ref().take(SAMPLE_RATE as usize * 2 * 30).collect();
                let mean = samples.iter().sum::<f32>() / samples.len() as f32;
                let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32)
                    .sqrt();
                assert!(mean.abs() < 0.01, "{preset:?} dc offset {mean}");
                // Music loudness matches the tone at equal volume: within
                // ±15% (≈1.2 dB) of the shared reference level.
                assert!(
                    (rms - REFERENCE_RMS).abs() < REFERENCE_RMS * 0.15,
                    "{preset:?} seed {seed}: rms {rms} vs reference {REFERENCE_RMS}"
                );
            }
        }
    }
}
