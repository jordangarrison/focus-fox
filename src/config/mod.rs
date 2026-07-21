use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use crate::cli::Args;

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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            work: Duration::from_secs(25 * 60),
            short_break: Duration::from_secs(5 * 60),
            long_break: Duration::from_secs(15 * 60),
            sessions_before_long_break: 4,
            notify: true,
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
        toml::from_str(&contents).with_context(|| format!("parsing config at {}", path.display()))
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
        self
    }
}
