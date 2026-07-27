use std::fmt;
use std::time::Duration;

use clap::ValueEnum;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use terminal_colorsaurus::{QueryOptions, ThemeMode as TerminalThemeMode};

/// User preference for choosing the TUI color theme.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    #[default]
    Auto,
    Dark,
    Light,
}

impl ThemePreference {
    pub fn adjust(self, direction: i64) -> Self {
        const THEMES: [ThemePreference; 3] = [
            ThemePreference::Auto,
            ThemePreference::Dark,
            ThemePreference::Light,
        ];
        let current = THEMES.iter().position(|theme| *theme == self).unwrap_or(0);
        let next = (current as i64 + direction).rem_euclid(THEMES.len() as i64) as usize;
        THEMES[next]
    }

    pub fn resolve(self, detected: ThemeMode) -> ThemeMode {
        match self {
            Self::Auto => detected,
            Self::Dark => ThemeMode::Dark,
            Self::Light => ThemeMode::Light,
        }
    }
}

impl fmt::Display for ThemePreference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Auto => "auto",
            Self::Dark => "dark",
            Self::Light => "light",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl fmt::Display for ThemeMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Dark => "dark",
            Self::Light => "light",
        })
    }
}

/// Detect the terminal's actual color scheme before Ratatui takes over.
///
/// Terminal color wins over OS color because a terminal can use a scheme
/// independent of the desktop. Unsupported terminals fall back to dark.
pub fn detect_terminal_theme() -> ThemeMode {
    let mut options = QueryOptions::default();
    options.timeout = Duration::from_millis(200);
    match terminal_colorsaurus::theme_mode(options) {
        Ok(TerminalThemeMode::Light) => ThemeMode::Light,
        Ok(TerminalThemeMode::Dark) | Err(_) => ThemeMode::Dark,
    }
}

/// Semantic colors shared by every screen.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub fox: Color,
    pub work: Color,
    pub short_break: Color,
    pub long_break: Color,
    pub muted: Color,
    pub secondary: Color,
    pub warning: Color,
}

impl ThemeMode {
    pub fn palette(self) -> Palette {
        match self {
            Self::Dark => Palette {
                fox: Color::Rgb(245, 185, 66),
                work: Color::Rgb(255, 107, 107),
                short_break: Color::Rgb(105, 219, 124),
                long_break: Color::Rgb(116, 192, 252),
                muted: Color::Rgb(134, 142, 150),
                secondary: Color::Rgb(173, 181, 189),
                warning: Color::Rgb(255, 212, 59),
            },
            Self::Light => Palette {
                fox: Color::Rgb(154, 91, 0),
                work: Color::Rgb(201, 42, 42),
                short_break: Color::Rgb(43, 138, 62),
                long_break: Color::Rgb(24, 100, 171),
                muted: Color::Rgb(92, 99, 106),
                secondary: Color::Rgb(73, 80, 87),
                warning: Color::Rgb(156, 111, 0),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_adjustment_wraps_in_both_directions() {
        assert_eq!(ThemePreference::Auto.adjust(1), ThemePreference::Dark);
        assert_eq!(ThemePreference::Auto.adjust(-1), ThemePreference::Light);
        assert_eq!(ThemePreference::Light.adjust(1), ThemePreference::Auto);
    }

    #[test]
    fn forced_preference_overrides_detected_theme() {
        assert_eq!(
            ThemePreference::Dark.resolve(ThemeMode::Light),
            ThemeMode::Dark
        );
        assert_eq!(
            ThemePreference::Light.resolve(ThemeMode::Dark),
            ThemeMode::Light
        );
        assert_eq!(
            ThemePreference::Auto.resolve(ThemeMode::Light),
            ThemeMode::Light
        );
    }
}
