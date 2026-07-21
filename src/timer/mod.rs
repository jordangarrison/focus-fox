use std::time::Duration;

use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Work,
    ShortBreak,
    LongBreak,
}

impl Phase {
    pub fn label(&self) -> &'static str {
        match self {
            Phase::Work => "Focus",
            Phase::ShortBreak => "Short Break",
            Phase::LongBreak => "Long Break",
        }
    }
}

/// Pomodoro state machine: Work -> ShortBreak (or LongBreak every N
/// sessions) -> Work -> ...
#[derive(Debug)]
pub struct Timer {
    pub phase: Phase,
    pub remaining: Duration,
    pub total: Duration,
    /// Work sessions finished naturally (skipped sessions don't count).
    pub completed_work: u32,
    pub paused: bool,
    config: Config,
}

impl Timer {
    pub fn new(config: Config) -> Self {
        Self {
            phase: Phase::Work,
            remaining: config.work,
            total: config.work,
            completed_work: 0,
            paused: false,
            config,
        }
    }

    /// Advance the clock. Returns the new phase when the current one ends.
    pub fn tick(&mut self, delta: Duration) -> Option<Phase> {
        if self.paused {
            return None;
        }
        if delta < self.remaining {
            self.remaining -= delta;
            return None;
        }
        Some(self.advance(true))
    }

    /// Jump to the next phase without finishing this one.
    pub fn skip(&mut self) -> Phase {
        self.advance(false)
    }

    /// Restart the current phase from the top.
    pub fn reset(&mut self) {
        self.remaining = self.total;
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    /// Fraction of the current phase elapsed, 0.0..=1.0.
    pub fn progress(&self) -> f64 {
        if self.total.is_zero() {
            return 1.0;
        }
        1.0 - self.remaining.as_secs_f64() / self.total.as_secs_f64()
    }

    /// Position within the current cycle of work sessions, for the dots row.
    pub fn cycle_position(&self) -> (u32, u32) {
        let n = self.config.sessions_before_long_break;
        let mut done = self.completed_work % n;
        // Right after the Nth session finishes we're heading into (or in) the
        // long break — show the cycle as full rather than empty.
        if done == 0 && self.completed_work > 0 && self.phase != Phase::Work {
            done = n;
        }
        (done, n)
    }

    fn advance(&mut self, finished: bool) -> Phase {
        self.phase = match self.phase {
            Phase::Work => {
                if finished {
                    self.completed_work += 1;
                }
                if finished && self.completed_work % self.config.sessions_before_long_break == 0 {
                    Phase::LongBreak
                } else {
                    Phase::ShortBreak
                }
            }
            Phase::ShortBreak | Phase::LongBreak => Phase::Work,
        };
        self.total = match self.phase {
            Phase::Work => self.config.work,
            Phase::ShortBreak => self.config.short_break,
            Phase::LongBreak => self.config.long_break,
        };
        self.remaining = self.total;
        self.phase
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            work: Duration::from_secs(10),
            short_break: Duration::from_secs(2),
            long_break: Duration::from_secs(5),
            sessions_before_long_break: 2,
            notify: false,
        }
    }

    #[test]
    fn ticks_down_and_transitions_to_short_break() {
        let mut t = Timer::new(config());
        assert_eq!(t.tick(Duration::from_secs(9)), None);
        assert_eq!(t.remaining, Duration::from_secs(1));
        assert_eq!(t.tick(Duration::from_secs(1)), Some(Phase::ShortBreak));
        assert_eq!(t.completed_work, 1);
        assert_eq!(t.remaining, Duration::from_secs(2));
    }

    #[test]
    fn long_break_after_configured_sessions() {
        let mut t = Timer::new(config());
        t.tick(Duration::from_secs(10)); // work 1 -> short break
        t.tick(Duration::from_secs(2)); // -> work 2
        assert_eq!(t.tick(Duration::from_secs(10)), Some(Phase::LongBreak));
        t.tick(Duration::from_secs(5)); // -> work 3
        assert_eq!(t.phase, Phase::Work);
    }

    #[test]
    fn skipped_work_does_not_count_toward_long_break() {
        let mut t = Timer::new(config());
        assert_eq!(t.skip(), Phase::ShortBreak);
        assert_eq!(t.completed_work, 0);
        t.skip(); // -> work
        t.tick(Duration::from_secs(10)); // work 1 done -> short break
        assert_eq!(t.phase, Phase::ShortBreak);
    }

    #[test]
    fn pause_stops_the_clock() {
        let mut t = Timer::new(config());
        t.toggle_pause();
        assert_eq!(t.tick(Duration::from_secs(60)), None);
        assert_eq!(t.remaining, Duration::from_secs(10));
    }

    #[test]
    fn reset_restarts_current_phase() {
        let mut t = Timer::new(config());
        t.tick(Duration::from_secs(4));
        t.reset();
        assert_eq!(t.remaining, Duration::from_secs(10));
    }
}
