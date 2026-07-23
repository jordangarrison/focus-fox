mod cli;
mod config;
mod notify;
mod stats;
mod timer;
mod tui;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let args = cli::Args::parse();
    let config = config::Config::load()?.merge_args(&args);
    tui::run(config)
}
