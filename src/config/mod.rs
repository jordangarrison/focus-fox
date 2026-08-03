use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use crate::cli::Args;
use crate::theme::ThemePreference;

pub const MIN_BINAURAL_BEAT_HZ: u16 = 1;
pub const MAX_BINAURAL_BEAT_HZ: u16 = 100;
pub const DEFAULT_BINAURAL_BEAT_HZ: u16 = 40;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Work session length
    #[serde(with = "humantime_serde")]
    pub work: Duration,

    /// Short break length
    #[serde(with = "humantime_serde")]
    pub short_break: Duration,

    /// Long break length
    #[serde(with = "humantime_serde")]
    pub long_break: Duration,

    /// Work sessions before a long break
    pub sessions_before_long_break: u32,

    /// Send desktop notifications on phase changes
    pub notify: bool,

    /// Hold on a full-screen alert at phase changes until Enter is pressed
    pub alert_screen: bool,

    /// Play binaural tones during work sessions
    pub binaural_beats: bool,

    /// Frequency difference between left and right tones
    pub binaural_beat_hz: u16,

    /// Color theme, or automatic terminal-background detection
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
            binaural_beat_hz: DEFAULT_BINAURAL_BEAT_HZ,
            theme: ThemePreference::Auto,
        }
    }
}

impl Config {
    pub fn config_path() -> Option<PathBuf> {
        ProjectDirs::from("dev", "jordangarrison", "focus-fox")
            .map(|dirs| dirs.config_dir().join("config.toml"))
    }

    /// Load config from the XDG config file, falling back to defaults.
    pub fn load() -> Result<Self> {
        let Some(path) = Self::config_path() else {
            return Ok(Self::default());
        };
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("reading config at {}", path.display()))?;
        let config: Self = toml::from_str(&contents)
            .with_context(|| format!("parsing config at {}", path.display()))?;
        Ok(config.normalized())
    }

    /// Write the current config to the XDG config file, creating it if needed.
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

    /// CLI arguments override config file values.
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

    fn normalized(mut self) -> Self {
        self.binaural_beat_hz = self
            .binaural_beat_hz
            .clamp(MIN_BINAURAL_BEAT_HZ, MAX_BINAURAL_BEAT_HZ);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_config_without_theme_uses_auto() {
        let config: Config = toml::from_str(
            r#"
work = "25m"
short_break = "5m"
long_break = "15m"
sessions_before_long_break = 4
notify = true
alert_screen = true
"#,
        )
        .unwrap();

        assert_eq!(config.theme, ThemePreference::Auto);
        assert!(!config.binaural_beats);
        assert_eq!(config.binaural_beat_hz, DEFAULT_BINAURAL_BEAT_HZ);
    }

    #[test]
    fn theme_serializes_as_lowercase_name() {
        let config = Config {
            theme: ThemePreference::Light,
            ..Config::default()
        };

        assert!(
            toml::to_string(&config)
                .unwrap()
                .contains("theme = \"light\"")
        );
    }

    #[test]
    fn binaural_settings_round_trip() {
        let config = Config {
            binaural_beats: true,
            binaural_beat_hz: 12,
            ..Config::default()
        };

        let encoded = toml::to_string(&config).unwrap();
        let decoded: Config = toml::from_str(&encoded).unwrap();

        assert!(decoded.binaural_beats);
        assert_eq!(decoded.binaural_beat_hz, 12);
    }

    #[test]
    fn binaural_frequency_is_normalized_to_supported_range() {
        let config = Config {
            binaural_beat_hz: 0,
            ..Config::default()
        }
        .normalized();
        assert_eq!(config.binaural_beat_hz, MIN_BINAURAL_BEAT_HZ);

        let config = Config {
            binaural_beat_hz: u16::MAX,
            ..Config::default()
        }
        .normalized();
        assert_eq!(config.binaural_beat_hz, MAX_BINAURAL_BEAT_HZ);
    }
}
