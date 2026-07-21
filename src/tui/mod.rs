mod app;
mod ui;

use anyhow::Result;

use crate::config::Config;

pub fn run(config: Config) -> Result<()> {
    let terminal = ratatui::init();
    let result = app::App::new(config).run(terminal);
    ratatui::restore();
    result
}
