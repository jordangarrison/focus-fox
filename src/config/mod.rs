use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Deserializer, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use crate::cli::Args;
use crate::theme::ThemePreference;

pub const MIN_BINAURAL_BASE_HZ: u16 = 100;
pub const MAX_BINAURAL_BASE_HZ: u16 = 1_000;
pub const BINAURAL_BASE_STEP_HZ: u16 = 10;
pub const MIN_BINAURAL_BEAT_HZ: u16 = 1;
pub const MAX_BINAURAL_BEAT_HZ: u16 = 100;
pub const MIN_BINAURAL_VOLUME_PERCENT: u8 = 1;
pub const MAX_BINAURAL_VOLUME_PERCENT: u8 = 100;
pub const DEFAULT_BINAURAL_BASE_HZ: u16 = 220;
pub const DEFAULT_BINAURAL_BEAT_HZ: u16 = 40;
// Tone and music are loudness-calibrated to a shared reference (see
// audio::REFERENCE_RMS), so equal percentages sound equally loud and the
// two defaults match. 36% on that scale equals the loudness the tone
// default produced before calibration.
pub const DEFAULT_BINAURAL_VOLUME_PERCENT: u8 = 36;
pub const MIN_MUSIC_VOLUME_PERCENT: u8 = 1;
pub const MAX_MUSIC_VOLUME_PERCENT: u8 = 100;
pub const DEFAULT_MUSIC_VOLUME_PERCENT: u8 = 36;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToneSettings {
    pub base_hz: u16,
    pub beat_hz: u16,
    pub volume_percent: u8,
}

impl ToneSettings {
    /// True when only the volume differs, so playback can continue with a
    /// gain change instead of a rebuild.
    pub fn same_tone(self, other: Self) -> bool {
        Self {
            volume_percent: 0,
            ..self
        } == Self {
            volume_percent: 0,
            ..other
        }
    }
}

/// Where background music comes from. One built-in source today; user-supplied
/// tracks or an API-backed source would be new variants here.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MusicSource {
    #[default]
    Lofi,
}

/// Everything music playback depends on. Excludes the tone volume and any
/// random seed so per-channel change detection stays correct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MusicSettings {
    pub source: MusicSource,
    pub preset: BinauralPreset,
    pub base_hz: u16,
    pub beat_hz: u16,
    pub volume_percent: u8,
}


#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinauralPreset {
    ActiveFocus,
    #[default]
    GammaExperiment,
    ResearchGamma,
    CalmConcentration,
    Meditative,
    WindDown,
    Custom,
}

impl BinauralPreset {
    pub const ALL: [Self; 7] = [
        Self::ActiveFocus,
        Self::GammaExperiment,
        Self::ResearchGamma,
        Self::CalmConcentration,
        Self::Meditative,
        Self::WindDown,
        Self::Custom,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::ActiveFocus => "Active focus",
            Self::GammaExperiment => "Gamma experiment",
            Self::ResearchGamma => "Research gamma",
            Self::CalmConcentration => "Calm concentration",
            Self::Meditative => "Meditative",
            Self::WindDown => "Wind-down",
            Self::Custom => "Custom",
        }
    }

    pub const fn frequencies(self) -> Option<(u16, u16)> {
        match self {
            Self::ActiveFocus => Some((220, 18)),
            Self::GammaExperiment => Some((220, 40)),
            Self::ResearchGamma => Some((320, 40)),
            Self::CalmConcentration => Some((220, 10)),
            Self::Meditative => Some((220, 6)),
            Self::WindDown => Some((160, 3)),
            Self::Custom => None,
        }
    }

    pub fn adjust(self, direction: i64) -> Self {
        let index = Self::ALL
            .iter()
            .position(|preset| *preset == self)
            .unwrap_or(0);
        let len = Self::ALL.len() as i64;
        Self::ALL[(index as i64 + direction.signum()).rem_euclid(len) as usize]
    }
}

impl std::fmt::Display for BinauralPreset {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.name())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Config {
    #[serde(with = "humantime_serde")]
    pub work: Duration,
    #[serde(with = "humantime_serde")]
    pub short_break: Duration,
    #[serde(with = "humantime_serde")]
    pub long_break: Duration,
    pub sessions_before_long_break: u32,
    pub notify: bool,
    pub alert_screen: bool,
    /// Play binaural tones during unpaused work sessions.
    pub binaural_beats: bool,
    /// Selected listening-mode preset. Base and beat fields retain Custom values.
    pub binaural_preset: BinauralPreset,
    pub binaural_base_hz: u16,
    pub binaural_beat_hz: u16,
    pub binaural_volume_percent: u8,
    /// Play background music during unpaused work sessions.
    pub music_enabled: bool,
    pub music_source: MusicSource,
    pub music_volume_percent: u8,
    /// Keep music playing through breaks (tones stay work-only).
    pub music_during_breaks: bool,
    pub theme: ThemePreference,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            work: Duration::from_secs(25 * 60),
            short_break: Duration::from_secs(5 * 60),
            long_break: Duration::from_secs(15 * 60),
            sessions_before_long_break: 4,
            notify: true,
            alert_screen: true,
            binaural_beats: false,
            binaural_preset: BinauralPreset::GammaExperiment,
            binaural_base_hz: DEFAULT_BINAURAL_BASE_HZ,
            binaural_beat_hz: DEFAULT_BINAURAL_BEAT_HZ,
            binaural_volume_percent: DEFAULT_BINAURAL_VOLUME_PERCENT,
            music_enabled: false,
            music_source: MusicSource::Lofi,
            music_volume_percent: DEFAULT_MUSIC_VOLUME_PERCENT,
            music_during_breaks: false,
            theme: ThemePreference::Auto,
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct ConfigFile {
    #[serde(with = "humantime_serde")]
    work: Duration,
    #[serde(with = "humantime_serde")]
    short_break: Duration,
    #[serde(with = "humantime_serde")]
    long_break: Duration,
    sessions_before_long_break: u32,
    notify: bool,
    alert_screen: bool,
    binaural_beats: bool,
    binaural_preset: Option<BinauralPreset>,
    binaural_base_hz: Option<u16>,
    binaural_beat_hz: Option<u16>,
    binaural_volume_percent: Option<u8>,
    music_enabled: bool,
    music_source: MusicSource,
    music_volume_percent: u8,
    music_during_breaks: bool,
    theme: ThemePreference,
}

impl Default for ConfigFile {
    fn default() -> Self {
        let config = Config::default();
        Self {
            work: config.work,
            short_break: config.short_break,
            long_break: config.long_break,
            sessions_before_long_break: config.sessions_before_long_break,
            notify: config.notify,
            alert_screen: config.alert_screen,
            binaural_beats: config.binaural_beats,
            binaural_preset: None,
            binaural_base_hz: None,
            binaural_beat_hz: None,
            binaural_volume_percent: None,
            music_enabled: config.music_enabled,
            music_source: config.music_source,
            music_volume_percent: config.music_volume_percent,
            music_during_breaks: config.music_during_breaks,
            theme: config.theme,
        }
    }
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let file = ConfigFile::deserialize(deserializer)?;
        let beat_hz = file.binaural_beat_hz.unwrap_or(DEFAULT_BINAURAL_BEAT_HZ);
        // Configs written before presets existed only stored beat difference.
        // Their default 40 Hz becomes Gamma experiment; every other value
        // becomes Custom with the legacy 220 Hz carrier.
        let preset = file.binaural_preset.unwrap_or({
            if beat_hz == DEFAULT_BINAURAL_BEAT_HZ {
                BinauralPreset::GammaExperiment
            } else {
                BinauralPreset::Custom
            }
        });
        Ok(Self {
            work: file.work,
            short_break: file.short_break,
            long_break: file.long_break,
            sessions_before_long_break: file.sessions_before_long_break,
            notify: file.notify,
            alert_screen: file.alert_screen,
            binaural_beats: file.binaural_beats,
            binaural_preset: preset,
            binaural_base_hz: file.binaural_base_hz.unwrap_or(DEFAULT_BINAURAL_BASE_HZ),
            binaural_beat_hz: beat_hz,
            binaural_volume_percent: file
                .binaural_volume_percent
                .unwrap_or(DEFAULT_BINAURAL_VOLUME_PERCENT),
            music_enabled: file.music_enabled,
            music_source: file.music_source,
            music_volume_percent: file.music_volume_percent,
            music_during_breaks: file.music_during_breaks,
            theme: file.theme,
        }
        .normalized())
    }
}

impl Config {
    pub fn config_path() -> Option<PathBuf> {
        ProjectDirs::from("dev", "jordangarrison", "focus-fox")
            .map(|dirs| dirs.config_dir().join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let Some(path) = Self::config_path() else {
            return Ok(Self::default());
        };
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("reading config at {}", path.display()))?;
        toml::from_str(&contents).with_context(|| format!("parsing config at {}", path.display()))
    }

    pub fn save(&self) -> Result<PathBuf> {
        let path = Self::config_path().context("could not determine config directory")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let contents = toml::to_string_pretty(self).context("serializing config")?;
        std::fs::write(&path, contents)
            .with_context(|| format!("writing config to {}", path.display()))?;
        Ok(path)
    }

    pub fn merge_args(mut self, args: &Args) -> Self {
        if let Some(work) = args.work {
            self.work = work;
        }
        if let Some(short_break) = args.short_break {
            self.short_break = short_break;
        }
        if let Some(long_break) = args.long_break {
            self.long_break = long_break;
        }
        if let Some(sessions) = args.sessions {
            self.sessions_before_long_break = sessions.max(1);
        }
        if args.no_notify {
            self.notify = false;
        }
        if args.no_alert {
            self.alert_screen = false;
        }
        if let Some(theme) = args.theme {
            self.theme = theme;
        }
        self.normalized()
    }

    pub fn tone_settings(&self) -> ToneSettings {
        let (base_hz, beat_hz) = self
            .binaural_preset
            .frequencies()
            .unwrap_or((self.binaural_base_hz, self.binaural_beat_hz));
        ToneSettings {
            base_hz,
            beat_hz,
            volume_percent: self.binaural_volume_percent,
        }
    }

    pub fn music_settings(&self) -> MusicSettings {
        let tone = self.tone_settings();
        MusicSettings {
            source: self.music_source,
            preset: self.binaural_preset,
            base_hz: tone.base_hz,
            beat_hz: tone.beat_hz,
            volume_percent: self.music_volume_percent,
        }
    }

    pub fn select_custom_from_active(&mut self) {
        if let Some((base_hz, beat_hz)) = self.binaural_preset.frequencies() {
            self.binaural_base_hz = base_hz;
            self.binaural_beat_hz = beat_hz;
            self.binaural_preset = BinauralPreset::Custom;
        }
    }

    fn normalized(mut self) -> Self {
        self.binaural_base_hz = normalize_base_hz(self.binaural_base_hz);
        self.binaural_beat_hz = self
            .binaural_beat_hz
            .clamp(MIN_BINAURAL_BEAT_HZ, MAX_BINAURAL_BEAT_HZ);
        self.binaural_volume_percent = self
            .binaural_volume_percent
            .clamp(MIN_BINAURAL_VOLUME_PERCENT, MAX_BINAURAL_VOLUME_PERCENT);
        self.music_volume_percent = self
            .music_volume_percent
            .clamp(MIN_MUSIC_VOLUME_PERCENT, MAX_MUSIC_VOLUME_PERCENT);
        self
    }
}

fn normalize_base_hz(value: u16) -> u16 {
    let value = value.clamp(MIN_BINAURAL_BASE_HZ, MAX_BINAURAL_BASE_HZ);
    let rounded =
        (value + BINAURAL_BASE_STEP_HZ / 2) / BINAURAL_BASE_STEP_HZ * BINAURAL_BASE_STEP_HZ;
    rounded.clamp(MIN_BINAURAL_BASE_HZ, MAX_BINAURAL_BASE_HZ)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_catalog_has_expected_order_names_and_frequencies() {
        let expected = [
            ("Active focus", Some((220, 18))),
            ("Gamma experiment", Some((220, 40))),
            ("Research gamma", Some((320, 40))),
            ("Calm concentration", Some((220, 10))),
            ("Meditative", Some((220, 6))),
            ("Wind-down", Some((160, 3))),
            ("Custom", None),
        ];
        for (preset, (name, frequencies)) in BinauralPreset::ALL.iter().zip(expected) {
            assert_eq!(preset.name(), name);
            assert_eq!(preset.frequencies(), frequencies);
        }
        assert_eq!(
            BinauralPreset::ActiveFocus.adjust(-1),
            BinauralPreset::Custom
        );
        assert_eq!(
            BinauralPreset::Custom.adjust(1),
            BinauralPreset::ActiveFocus
        );
    }

    #[test]
    fn default_is_disabled_gamma_experiment() {
        let config = Config::default();
        assert!(!config.binaural_beats);
        assert_eq!(config.binaural_preset, BinauralPreset::GammaExperiment);
        assert_eq!(
            config.tone_settings(),
            ToneSettings {
                base_hz: 220,
                beat_hz: 40,
                volume_percent: DEFAULT_BINAURAL_VOLUME_PERCENT,
            }
        );
    }

    #[test]
    fn preset_serializes_as_snake_case_and_round_trips() {
        let config = Config {
            binaural_beats: true,
            binaural_preset: BinauralPreset::ResearchGamma,
            binaural_base_hz: 470,
            binaural_beat_hz: 12,
            binaural_volume_percent: 23,
            ..Config::default()
        };
        let encoded = toml::to_string(&config).unwrap();
        assert!(encoded.contains("binaural_preset = \"research_gamma\""));
        let decoded: Config = toml::from_str(&encoded).unwrap();
        assert_eq!(decoded.binaural_preset, BinauralPreset::ResearchGamma);
        assert_eq!(decoded.binaural_base_hz, 470);
        assert_eq!(decoded.binaural_beat_hz, 12);
        assert_eq!(decoded.binaural_volume_percent, 23);
    }

    #[test]
    fn older_config_without_audio_fields_uses_new_defaults() {
        let config: Config = toml::from_str(
            r#"work = "25m"
short_break = "5m"
long_break = "15m"
sessions_before_long_break = 4
notify = true
alert_screen = true
"#,
        )
        .unwrap();
        assert!(!config.binaural_beats);
        assert_eq!(config.binaural_preset, BinauralPreset::GammaExperiment);
        assert!(!config.music_enabled);
        assert_eq!(config.music_source, MusicSource::Lofi);
        assert_eq!(config.music_volume_percent, DEFAULT_MUSIC_VOLUME_PERCENT);
        assert!(!config.music_during_breaks);
        assert_eq!(config.theme, ThemePreference::Auto);
    }

    #[test]
    fn music_fields_round_trip_and_clamp() {
        let config = Config {
            music_enabled: true,
            music_volume_percent: 55,
            music_during_breaks: true,
            ..Config::default()
        };
        let encoded = toml::to_string(&config).unwrap();
        assert!(encoded.contains("music_source = \"lofi\""));
        let decoded: Config = toml::from_str(&encoded).unwrap();
        assert!(decoded.music_enabled);
        assert_eq!(decoded.music_volume_percent, 55);
        assert!(decoded.music_during_breaks);

        let clamped: Config = toml::from_str("music_volume_percent = 200").unwrap();
        assert_eq!(clamped.music_volume_percent, MAX_MUSIC_VOLUME_PERCENT);
        let clamped: Config = toml::from_str("music_volume_percent = 0").unwrap();
        assert_eq!(clamped.music_volume_percent, MIN_MUSIC_VOLUME_PERCENT);
    }

    #[test]
    fn music_settings_resolve_preset_frequencies() {
        let config = Config {
            binaural_preset: BinauralPreset::WindDown,
            music_volume_percent: 33,
            ..Config::default()
        };
        assert_eq!(
            config.music_settings(),
            MusicSettings {
                source: MusicSource::Lofi,
                preset: BinauralPreset::WindDown,
                base_hz: 160,
                beat_hz: 3,
                volume_percent: 33,
            }
        );
    }

    #[test]
    fn same_tone_ignores_volume_only() {
        let tone = Config::default().tone_settings();
        let louder = ToneSettings {
            volume_percent: 90,
            ..tone
        };
        let retuned = ToneSettings {
            base_hz: tone.base_hz + 10,
            ..tone
        };
        assert!(tone.same_tone(louder));
        assert!(!tone.same_tone(retuned));
    }

    #[test]
    fn legacy_beat_difference_migrates_to_preset_or_custom() {
        let gamma: Config = toml::from_str("binaural_beats = true\nbinaural_beat_hz = 40").unwrap();
        assert_eq!(gamma.binaural_preset, BinauralPreset::GammaExperiment);

        let custom: Config =
            toml::from_str("binaural_beats = true\nbinaural_beat_hz = 12").unwrap();
        assert_eq!(custom.binaural_preset, BinauralPreset::Custom);
        assert_eq!(custom.tone_settings().base_hz, 220);
        assert_eq!(custom.tone_settings().beat_hz, 12);
    }

    #[test]
    fn custom_values_normalize_and_survive_builtin_selection() {
        let config: Config = toml::from_str(
            r#"binaural_preset = "research_gamma"
binaural_base_hz = 104
binaural_beat_hz = 0
binaural_volume_percent = 255
"#,
        )
        .unwrap();
        assert_eq!(config.binaural_base_hz, 100);
        assert_eq!(config.binaural_beat_hz, 1);
        assert_eq!(config.binaural_volume_percent, 100);
        assert_eq!(config.tone_settings().base_hz, 320);

        let mut config = Config {
            binaural_preset: BinauralPreset::Meditative,
            binaural_base_hz: 510,
            binaural_beat_hz: 27,
            ..Config::default()
        };
        config.binaural_preset = BinauralPreset::Custom;
        assert_eq!(
            (
                config.tone_settings().base_hz,
                config.tone_settings().beat_hz
            ),
            (510, 27)
        );
    }

    #[test]
    fn editing_builtin_copies_it_into_custom_slot() {
        let mut config = Config {
            binaural_preset: BinauralPreset::WindDown,
            binaural_base_hz: 800,
            binaural_beat_hz: 80,
            ..Config::default()
        };
        config.select_custom_from_active();
        assert_eq!(config.binaural_preset, BinauralPreset::Custom);
        assert_eq!((config.binaural_base_hz, config.binaural_beat_hz), (160, 3));
    }
}
