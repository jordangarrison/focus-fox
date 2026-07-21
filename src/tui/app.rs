use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::config::Config;
use crate::notify;
use crate::timer::{Phase, Timer};

pub struct App {
    pub timer: Timer,
    notify: bool,
    should_quit: bool,
}

impl App {
    pub fn new(config: Config) -> Self {
        let notify = config.notify;
        Self {
            timer: Timer::new(config),
            notify,
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
            if let Some(phase) = self.timer.tick(now - last_tick) {
                self.announce(phase);
            }
            last_tick = now;
        }
        Ok(())
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char(' ') | KeyCode::Char('p') => self.timer.toggle_pause(),
            KeyCode::Char('s') => {
                let phase = self.timer.skip();
                self.announce(phase);
            }
            KeyCode::Char('r') => self.timer.reset(),
            _ => {}
        }
    }

    fn announce(&self, phase: Phase) {
        if !self.notify {
            return;
        }
        let (summary, body) = match phase {
            Phase::Work => ("Back to it 🦊", "Time to focus."),
            Phase::ShortBreak => ("Break time", "Stretch your legs for a bit."),
            Phase::LongBreak => ("Long break", "You earned it. Step away."),
        };
        notify::send(summary, body);
    }
}
