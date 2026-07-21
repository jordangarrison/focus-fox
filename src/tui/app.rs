use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::config::Config;
use crate::notify;
use crate::timer::{Phase, Timer};

pub const MENU_ITEMS: [&str; 5] = [
    "Work",
    "Short break",
    "Long break",
    "Sessions",
    "Notifications",
];

pub enum Screen {
    Menu { selected: usize },
    Timer(Timer),
}

pub struct App {
    pub config: Config,
    pub screen: Screen,
    pub status: Option<String>,
    should_quit: bool,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            screen: Screen::Menu { selected: 0 },
            status: None,
            should_quit: false,
        }
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let tick_rate = Duration::from_millis(100);
        let mut last_tick = Instant::now();

        while !self.should_quit {
            terminal.draw(|frame| super::ui::render(frame, &self))?;

            if event::poll(tick_rate)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key.code, key.modifiers);
                    }
                }
            }

            let now = Instant::now();
            if let Screen::Timer(timer) = &mut self.screen {
                if let Some(phase) = timer.tick(now - last_tick) {
                    announce(&self.config, phase);
                }
            }
            last_tick = now;
        }
        Ok(())
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        self.status = None;
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return;
            }
            _ => {}
        }
        match &mut self.screen {
            Screen::Menu { selected } => {
                let selected = *selected;
                match code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.select(selected.checked_sub(1).unwrap_or(MENU_ITEMS.len() - 1));
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.select((selected + 1) % MENU_ITEMS.len());
                    }
                    KeyCode::Left | KeyCode::Char('h') => self.adjust(selected, -1),
                    KeyCode::Right | KeyCode::Char('l') => self.adjust(selected, 1),
                    KeyCode::Enter => {
                        self.screen = Screen::Timer(Timer::new(self.config.clone()));
                    }
                    KeyCode::Char('w') => self.save_config(),
                    _ => {}
                }
            }
            Screen::Timer(timer) => match code {
                KeyCode::Char(' ') | KeyCode::Char('p') => timer.toggle_pause(),
                KeyCode::Char('s') => {
                    let phase = timer.skip();
                    announce(&self.config, phase);
                }
                KeyCode::Char('r') => timer.reset(),
                KeyCode::Char('m') => self.screen = Screen::Menu { selected: 0 },
                _ => {}
            },
        }
    }

    fn select(&mut self, index: usize) {
        self.screen = Screen::Menu { selected: index };
    }

    fn adjust(&mut self, selected: usize, dir: i64) {
        let c = &mut self.config;
        match selected {
            0 => c.work = bump_duration(c.work, dir),
            1 => c.short_break = bump_duration(c.short_break, dir),
            2 => c.long_break = bump_duration(c.long_break, dir),
            3 => {
                c.sessions_before_long_break =
                    (c.sessions_before_long_break as i64 + dir).max(1) as u32;
            }
            4 => c.notify = !c.notify,
            _ => {}
        }
    }

    fn save_config(&mut self) {
        self.status = Some(match self.config.save() {
            Ok(path) => format!("saved to {}", path.display()),
            Err(err) => format!("save failed: {err}"),
        });
    }
}

/// Adjust a duration by one minute, never below one minute.
fn bump_duration(d: Duration, dir: i64) -> Duration {
    let minute = Duration::from_secs(60);
    if dir > 0 {
        d.saturating_add(minute)
    } else {
        d.saturating_sub(minute).max(minute)
    }
}

fn announce(config: &Config, phase: Phase) {
    if !config.notify {
        return;
    }
    let (summary, body) = match phase {
        Phase::Work => ("Back to it 🦊", "Time to focus."),
        Phase::ShortBreak => ("Break time", "Stretch your legs for a bit."),
        Phase::LongBreak => ("Long break", "You earned it. Step away."),
    };
    notify::send(summary, body);
}
