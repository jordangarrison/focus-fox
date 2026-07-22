use clap::Parser;
use std::time::Duration;

/// Focus Fox - a terminal pomodoro timer
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Args {
    /// Work session length (e.g. "25m", "45m", "1h")
    #[arg(short, long, value_parser = humantime::parse_duration)]
    pub work: Option<Duration>,

    /// Short break length (e.g. "5m")
    #[arg(short = 'b', long, value_parser = humantime::parse_duration)]
    pub short_break: Option<Duration>,

    /// Long break length (e.g. "15m")
    #[arg(short = 'l', long, value_parser = humantime::parse_duration)]
    pub long_break: Option<Duration>,

    /// Work sessions before a long break
    #[arg(short, long)]
    pub sessions: Option<u32>,

    /// Disable desktop notifications
    #[arg(long)]
    pub no_notify: bool,

    /// Disable the full-screen alert between sessions
    #[arg(long)]
    pub no_alert: bool,
}
