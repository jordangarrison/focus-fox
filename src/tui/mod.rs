mod app;
mod ui;

use anyhow::Result;

use crate::config::Config;
use crate::stats::store::Store;
use crate::theme;

pub fn run(config: Config) -> Result<()> {
    let detected_theme = theme::detect_terminal_theme();
    let terminal = ratatui::init();
    let store = Store::default_dir().map(Store::new);
    let result = app::App::new(config, store, detected_theme).run(terminal);
    ratatui::restore();
    result
}
