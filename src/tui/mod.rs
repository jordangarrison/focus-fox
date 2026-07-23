mod app;
mod ui;

use anyhow::Result;

use crate::config::Config;
use crate::stats::store::Store;

pub fn run(config: Config) -> Result<()> {
    let terminal = ratatui::init();
    let store = Store::default_dir().map(Store::new);
    let result = app::App::new(config, store).run(terminal);
    ratatui::restore();
    result
}
