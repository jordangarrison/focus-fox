use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::audio::Audio;
use crate::config::{
    BINAURAL_BASE_STEP_HZ, Config, MAX_BINAURAL_BASE_HZ, MAX_BINAURAL_BEAT_HZ,
    MAX_BINAURAL_VOLUME_PERCENT, MIN_BINAURAL_BASE_HZ, MIN_BINAURAL_BEAT_HZ,
    MIN_BINAURAL_VOLUME_PERCENT, ToneSettings,
};
use crate::notify;
use crate::stats::{Record, Summary, store::Store};
use crate::theme::{Palette, ThemeMode};
use crate::timer::{Phase, Timer};

/// How far one left/right keypress scrubs the running timer.
const SEEK_STEP: Duration = Duration::from_secs(60);

pub const MENU_ITEMS: [&str; 8] = [
    "Work",
    "Short break",
    "Long break",
    "Sessions",
    "Notifications",
    "Alert screen",
    "Audio settings",
    "Theme",
];

pub const AUDIO_MENU_ITEMS: [&str; 5] = [
    "Enabled",
    "Tone preset",
    "Base tone",
    "Beat difference",
    "Beat volume",
];

pub enum Screen {
    Menu { selected: usize },
    AudioMenu { selected: usize, preview: bool },
    Timer(Timer),
}

pub struct App {
    pub config: Config,
    pub screen: Screen,
    pub status: Option<String>,
    /// When set, the timer is frozen on a full-screen alert announcing this
    /// upcoming phase until the user presses Enter.
    pub alert: Option<Phase>,
    /// History log; None when the data dir can't be determined (recording
    /// is best-effort, like notifications).
    pub store: Option<Store>,
    /// When set, a stats panel is drawn over the current screen. The timer
    /// keeps ticking underneath.
    pub stats_view: Option<Summary>,
    detected_theme: ThemeMode,
    true_color: bool,
    should_quit: bool,
    audio: Audio,
}

impl App {
    pub fn new(
        config: Config,
        store: Option<Store>,
        detected_theme: ThemeMode,
        true_color: bool,
    ) -> Self {
        Self {
            config,
            screen: Screen::Menu { selected: 0 },
            status: None,
            alert: None,
            store,
            stats_view: None,
            detected_theme,
            true_color,
            should_quit: false,
            audio: Audio::new(),
        }
    }

    pub fn theme_mode(&self) -> ThemeMode {
        self.config.theme.resolve(self.detected_theme)
    }

    pub fn palette(&self) -> Palette {
        self.theme_mode().palette(self.true_color)
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let tick_rate = Duration::from_millis(100);
        let mut last_tick = Instant::now();

        while !self.should_quit {
            terminal.draw(|frame| super::ui::render(frame, &self))?;

            if event::poll(tick_rate)?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                self.handle_key(key.code, key.modifiers);
            }

            let now = Instant::now();
            self.advance_clock(now - last_tick);
            last_tick = now;
            self.sync_audio();
        }
        Ok(())
    }

    /// Tick the timer unless an alert is holding it between phases.
    fn advance_clock(&mut self, delta: Duration) {
        if self.alert.is_some() {
            return;
        }
        if let Screen::Timer(timer) = &mut self.screen {
            let (ended, planned) = (timer.phase, timer.total);
            if let Some(phase) = timer.tick(delta) {
                record_phase_end(&self.store, &mut self.status, ended, planned, planned, true);
                announce(&self.config, phase);
                if self.config.alert_screen {
                    self.alert = Some(phase);
                    // The alert outranks the stats overlay — never let it
                    // fire hidden underneath the panel.
                    self.stats_view = None;
                }
            }
        }
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        self.status = None;
        match code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return;
            }
            _ => {}
        }
        // The stats overlay swallows Esc (close, don't quit) and toggles on t.
        if self.stats_view.is_some() {
            if matches!(code, KeyCode::Esc | KeyCode::Char('t')) {
                self.stats_view = None;
            }
            return;
        }
        if code == KeyCode::Esc {
            if matches!(self.screen, Screen::AudioMenu { .. }) {
                self.screen = Screen::Menu { selected: 6 };
            } else {
                self.should_quit = true;
            }
            return;
        }
        if self.alert.is_some() {
            match code {
                KeyCode::Enter => self.alert = None,
                // Skip the phase the alert announced and start the one after
                // it immediately — the keypress proves the user is present.
                KeyCode::Char('s') => {
                    self.alert = None;
                    if let Screen::Timer(timer) = &mut self.screen {
                        let (ended, planned) = (timer.phase, timer.total);
                        let phase = timer.skip();
                        record_phase_end(
                            &self.store,
                            &mut self.status,
                            ended,
                            planned,
                            Duration::ZERO,
                            false,
                        );
                        announce(&self.config, phase);
                    }
                }
                _ => {}
            }
            return;
        }
        if matches!(self.screen, Screen::AudioMenu { .. }) {
            self.handle_audio_menu_key(code);
            return;
        }
        if code == KeyCode::Char('t') {
            self.open_stats();
            return;
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
                        if selected == 6 {
                            self.open_audio_menu();
                        } else {
                            self.screen = Screen::Timer(Timer::new(self.config.clone()));
                        }
                    }
                    KeyCode::Char('a') => self.open_audio_menu(),
                    _ => {}
                }
            }
            Screen::Timer(timer) => match code {
                KeyCode::Char(' ') | KeyCode::Char('p') => timer.toggle_pause(),
                KeyCode::Char('s') => {
                    let (ended, planned, elapsed) =
                        (timer.phase, timer.total, timer.total - timer.remaining);
                    let phase = timer.skip();
                    record_phase_end(
                        &self.store,
                        &mut self.status,
                        ended,
                        planned,
                        elapsed,
                        false,
                    );
                    announce(&self.config, phase);
                }
                KeyCode::Char('r') => timer.reset(),
                KeyCode::Right | KeyCode::Char('l') => timer.seek_forward(SEEK_STEP),
                KeyCode::Left | KeyCode::Char('h') => timer.seek_back(SEEK_STEP),
                KeyCode::Char('m') => self.screen = Screen::Menu { selected: 0 },
                _ => {}
            },
            Screen::AudioMenu { .. } => unreachable!("audio menu handled above"),
        }
    }

    fn open_audio_menu(&mut self) {
        self.screen = Screen::AudioMenu {
            selected: 0,
            preview: false,
        };
    }

    fn handle_audio_menu_key(&mut self, code: KeyCode) {
        let (selected, preview) = match &self.screen {
            Screen::AudioMenu { selected, preview } => (*selected, *preview),
            _ => return,
        };
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.screen = Screen::AudioMenu {
                    selected: selected
                        .checked_sub(1)
                        .unwrap_or(AUDIO_MENU_ITEMS.len() - 1),
                    preview,
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.screen = Screen::AudioMenu {
                    selected: (selected + 1) % AUDIO_MENU_ITEMS.len(),
                    preview,
                };
            }
            KeyCode::Left | KeyCode::Char('h') => self.adjust_audio(selected, -1),
            KeyCode::Right | KeyCode::Char('l') => self.adjust_audio(selected, 1),
            KeyCode::Char('p') => {
                self.screen = Screen::AudioMenu {
                    selected,
                    preview: !preview,
                };
            }
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('m') => {
                self.screen = Screen::Menu { selected: 6 };
            }
            _ => {}
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
            5 => c.alert_screen = !c.alert_screen,
            6 => c.binaural_beats = !c.binaural_beats,
            7 => c.theme = c.theme.adjust(dir),
            _ => {}
        }
        self.save_config();
    }

    fn adjust_audio(&mut self, selected: usize, dir: i64) {
        let c = &mut self.config;
        match selected {
            0 => c.binaural_beats = !c.binaural_beats,
            1 => c.binaural_preset = c.binaural_preset.adjust(dir),
            2 => {
                c.select_custom_from_active();
                c.binaural_base_hz =
                    (i64::from(c.binaural_base_hz) + dir * i64::from(BINAURAL_BASE_STEP_HZ)).clamp(
                        i64::from(MIN_BINAURAL_BASE_HZ),
                        i64::from(MAX_BINAURAL_BASE_HZ),
                    ) as u16;
            }
            3 => {
                c.select_custom_from_active();
                c.binaural_beat_hz = (i64::from(c.binaural_beat_hz) + dir).clamp(
                    i64::from(MIN_BINAURAL_BEAT_HZ),
                    i64::from(MAX_BINAURAL_BEAT_HZ),
                ) as u16;
            }
            4 => {
                c.binaural_volume_percent = (i64::from(c.binaural_volume_percent) + dir).clamp(
                    i64::from(MIN_BINAURAL_VOLUME_PERCENT),
                    i64::from(MAX_BINAURAL_VOLUME_PERCENT),
                ) as u8;
            }
            _ => {}
        }
        self.save_config();
    }

    fn save_config(&mut self) {
        // Unit tests exercise menu controls and must not touch the user's XDG config.
        if cfg!(test) {
            return;
        }

        // Menu settings persist between app starts; only surface failures.
        if let Err(err) = self.config.save() {
            self.status = Some(format!("save failed: {err}"));
        }
    }

    /// Load history and compute the summary once, at open time.
    fn open_stats(&mut self) {
        use chrono::Datelike;
        let now = chrono::Local::now();
        let records = self
            .store
            .as_ref()
            .map(|s| s.load_recent(now.year()))
            .unwrap_or_default();
        self.stats_view = Some(Summary::compute(&records, now.date_naive()));
    }

    /// Audio follows timer state rather than wall-clock events. This covers
    /// menu returns, pauses, skips, phase changes, alerts, and every exit path.
    fn audio_target(&self) -> Option<ToneSettings> {
        if self.should_quit || self.alert.is_some() {
            return None;
        }
        if matches!(self.screen, Screen::AudioMenu { preview: true, .. }) {
            return Some(self.config.tone_settings());
        }
        if !self.config.binaural_beats {
            return None;
        }
        match &self.screen {
            Screen::Timer(timer) if timer.phase == Phase::Work && !timer.paused => {
                Some(self.config.tone_settings())
            }
            Screen::Menu { .. } | Screen::AudioMenu { .. } | Screen::Timer(_) => None,
        }
    }

    fn sync_audio(&mut self) {
        let target = self.audio_target();
        if let Err(err) = self.audio.sync(target) {
            self.status = Some(format!("audio unavailable: {err}"));
        }
    }
}

/// Step a duration to the next/previous multiple of five minutes, never
/// below one minute. Off-grid values snap to the grid rather than keeping
/// their offset (1m -> 5m -> 10m, not 1m -> 6m -> 11m).
fn bump_duration(d: Duration, dir: i64) -> Duration {
    const STEP: u64 = 5 * 60;
    let secs = d.as_secs();
    let snapped = if dir > 0 {
        (secs / STEP + 1) * STEP
    } else {
        secs.saturating_sub(1) / STEP * STEP
    };
    Duration::from_secs(snapped.max(60))
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

/// Best-effort history append — a failure surfaces in the status line and
/// never interrupts the timer.
fn record_phase_end(
    store: &Option<Store>,
    status: &mut Option<String>,
    phase: Phase,
    planned: Duration,
    elapsed: Duration,
    completed: bool,
) {
    let Some(store) = store else { return };
    let record = Record {
        at: chrono::Local::now().fixed_offset(),
        phase,
        planned_secs: planned.as_secs(),
        actual_secs: elapsed.as_secs(),
        completed,
    };
    if let Err(err) = store.append(&record) {
        *status = Some(format!("history save failed: {err}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mins(m: u64) -> Duration {
        Duration::from_secs(m * 60)
    }

    #[test]
    fn steps_on_the_five_minute_grid() {
        assert_eq!(bump_duration(mins(25), 1), mins(30));
        assert_eq!(bump_duration(mins(25), -1), mins(20));
    }

    #[test]
    fn snaps_back_to_grid_from_the_one_minute_floor() {
        assert_eq!(bump_duration(mins(5), -1), mins(1));
        assert_eq!(bump_duration(mins(1), 1), mins(5));
    }

    #[test]
    fn off_grid_values_snap_instead_of_keeping_offset() {
        assert_eq!(bump_duration(mins(7), 1), mins(10));
        assert_eq!(bump_duration(mins(7), -1), mins(5));
    }

    #[test]
    fn menu_adjusts_and_resolves_theme() {
        let mut app = App::new(Config::default(), None, ThemeMode::Light, false);
        assert_eq!(app.theme_mode(), ThemeMode::Light);

        app.adjust(7, 1);
        assert_eq!(app.config.theme, crate::theme::ThemePreference::Dark);
        assert_eq!(app.theme_mode(), ThemeMode::Dark);
    }

    fn app_on_timer(alert_screen: bool) -> App {
        let config = Config {
            work: Duration::from_secs(10),
            short_break: Duration::from_secs(2),
            long_break: Duration::from_secs(5),
            sessions_before_long_break: 4,
            notify: false,
            alert_screen,
            ..Config::default()
        };
        let mut app = App::new(config.clone(), None, ThemeMode::Dark, false);
        app.screen = Screen::Timer(Timer::new(config));
        app
    }

    fn remaining(app: &App) -> Duration {
        match &app.screen {
            Screen::Timer(timer) => timer.remaining,
            Screen::Menu { .. } | Screen::AudioMenu { .. } => panic!("expected timer screen"),
        }
    }

    #[test]
    fn launch_menu_toggles_audio_and_opens_submenu() {
        let mut app = App::new(Config::default(), None, ThemeMode::Dark, false);

        app.adjust(6, 1);
        assert!(app.config.binaural_beats);

        app.handle_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(matches!(
            app.screen,
            Screen::AudioMenu {
                selected: 0,
                preview: false
            }
        ));
    }

    #[test]
    fn audio_menu_controls_have_bounds_and_builtin_edit_becomes_custom() {
        let mut app = App::new(Config::default(), None, ThemeMode::Dark, false);

        app.config.binaural_beat_hz = MAX_BINAURAL_BEAT_HZ;
        app.config.binaural_preset = crate::config::BinauralPreset::Custom;
        app.adjust_audio(3, 1);
        assert_eq!(app.config.binaural_beat_hz, MAX_BINAURAL_BEAT_HZ);

        app.config.binaural_beat_hz = MIN_BINAURAL_BEAT_HZ;
        app.adjust_audio(3, -1);
        assert_eq!(app.config.binaural_beat_hz, MIN_BINAURAL_BEAT_HZ);

        app.config.binaural_preset = crate::config::BinauralPreset::ResearchGamma;
        app.adjust_audio(2, 1);
        assert_eq!(
            app.config.binaural_preset,
            crate::config::BinauralPreset::Custom
        );
        assert_eq!(app.config.binaural_base_hz, 330);
        assert_eq!(app.config.binaural_beat_hz, 40);
    }

    #[test]
    fn audio_target_follows_running_work_only() {
        let mut app = app_on_timer(false);
        app.config.binaural_beats = true;
        let settings = app.config.tone_settings();
        assert_eq!(app.audio_target(), Some(settings));

        app.handle_key(KeyCode::Char('p'), KeyModifiers::NONE);
        assert_eq!(app.audio_target(), None);
        app.handle_key(KeyCode::Char('p'), KeyModifiers::NONE);
        assert_eq!(app.audio_target(), Some(settings));

        app.handle_key(KeyCode::Char('s'), KeyModifiers::NONE);
        assert_eq!(app.audio_target(), None);
        app.handle_key(KeyCode::Char('s'), KeyModifiers::NONE);
        assert_eq!(app.audio_target(), Some(settings));

        app.handle_key(KeyCode::Char('m'), KeyModifiers::NONE);
        assert_eq!(app.audio_target(), None);
    }

    #[test]
    fn alert_and_exit_suppress_audio() {
        let mut app = app_on_timer(true);
        app.config.binaural_beats = true;
        app.advance_clock(Duration::from_secs(10));
        assert!(app.alert.is_some());
        assert_eq!(app.audio_target(), None);

        app.handle_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(app.audio_target(), None);
    }

    #[test]
    fn preview_works_while_disabled_and_stops_when_submenu_closes() {
        let mut app = App::new(Config::default(), None, ThemeMode::Dark, false);
        assert!(!app.config.binaural_beats);
        app.handle_key(KeyCode::Char('a'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Char('p'), KeyModifiers::NONE);
        assert_eq!(app.audio_target(), Some(app.config.tone_settings()));

        app.config.binaural_volume_percent = 31;
        assert_eq!(app.audio_target().unwrap().volume_percent, 31);

        app.handle_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(matches!(app.screen, Screen::Menu { selected: 6 }));
        assert_eq!(app.audio_target(), None);
        assert!(!app.should_quit);
    }

    #[test]
    fn audio_failure_surfaces_in_status_line() {
        let mut app = app_on_timer(false);
        app.config.binaural_beats = true;
        app.audio.fail_next_sync();
        app.sync_audio();
        assert_eq!(
            app.status.as_deref(),
            Some("audio unavailable: test audio failure")
        );
    }

    #[test]
    fn alert_freezes_timer_until_enter() {
        let mut app = app_on_timer(true);
        app.advance_clock(Duration::from_secs(10)); // work ends -> short break
        assert_eq!(app.alert, Some(Phase::ShortBreak));

        // Clock is held: the break hasn't started counting down.
        app.advance_clock(Duration::from_secs(60));
        assert_eq!(remaining(&app), Duration::from_secs(2));

        // Unbound keys don't dismiss it; Enter does.
        app.handle_key(KeyCode::Char('x'), KeyModifiers::NONE);
        assert_eq!(app.alert, Some(Phase::ShortBreak));
        app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.alert, None);

        app.advance_clock(Duration::from_secs(1));
        assert_eq!(remaining(&app), Duration::from_secs(1));
    }

    #[test]
    fn skip_from_alert_jumps_past_the_announced_phase() {
        let mut app = app_on_timer(true);
        app.advance_clock(Duration::from_secs(10)); // work ends -> break alert
        assert_eq!(app.alert, Some(Phase::ShortBreak));

        // Skip the break entirely: straight into work, running, no new alert.
        app.handle_key(KeyCode::Char('s'), KeyModifiers::NONE);
        assert_eq!(app.alert, None);
        assert_eq!(remaining(&app), Duration::from_secs(10));
        app.advance_clock(Duration::from_secs(1));
        assert_eq!(remaining(&app), Duration::from_secs(9));
    }

    #[test]
    fn alert_disabled_rolls_straight_into_next_phase() {
        let mut app = app_on_timer(false);
        app.advance_clock(Duration::from_secs(10));
        assert_eq!(app.alert, None);
        app.advance_clock(Duration::from_secs(1));
        assert_eq!(remaining(&app), Duration::from_secs(1));
    }

    #[test]
    fn arrows_scrub_the_running_timer() {
        let mut app = app_on_timer(false);
        app.handle_key(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(
            remaining(&app),
            Duration::from_secs(10).saturating_sub(SEEK_STEP)
        );
        app.handle_key(KeyCode::Char('h'), KeyModifiers::NONE);
        assert_eq!(remaining(&app), Duration::from_secs(10));
        // Rewinding never goes past the start of the phase.
        app.handle_key(KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(remaining(&app), Duration::from_secs(10));
    }

    #[test]
    fn seeking_past_the_end_completes_the_phase_naturally() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_store(dir.path());
        app.handle_key(KeyCode::Char('l'), KeyModifiers::NONE); // 10s phase, 60s jump
        assert_eq!(remaining(&app), Duration::ZERO);
        app.advance_clock(Duration::from_millis(100));
        let records = load_records(&app);
        assert_eq!(records.len(), 1);
        assert!(records[0].completed);
    }

    #[test]
    fn manual_skip_does_not_raise_alert() {
        let mut app = app_on_timer(true);
        app.handle_key(KeyCode::Char('s'), KeyModifiers::NONE);
        assert_eq!(app.alert, None);
    }

    use crate::stats::store::Store;

    fn app_with_store(dir: &std::path::Path) -> App {
        let mut app = app_on_timer(false);
        app.store = Some(Store::new(dir.to_path_buf()));
        app
    }

    fn load_records(app: &App) -> Vec<crate::stats::Record> {
        use chrono::Datelike;
        app.store
            .as_ref()
            .unwrap()
            .load_recent(chrono::Local::now().year())
    }

    #[test]
    fn natural_finish_appends_a_completed_record() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_store(dir.path());
        app.advance_clock(Duration::from_secs(10)); // work ends
        let records = load_records(&app);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].phase, Phase::Work);
        assert_eq!(records[0].planned_secs, 10);
        assert_eq!(records[0].actual_secs, 10);
        assert!(records[0].completed);
    }

    #[test]
    fn skip_appends_a_partial_record() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_store(dir.path());
        app.advance_clock(Duration::from_secs(4));
        app.handle_key(KeyCode::Char('s'), KeyModifiers::NONE);
        let records = load_records(&app);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].actual_secs, 4);
        assert!(!records[0].completed);
    }

    #[test]
    fn skip_from_alert_records_the_announced_phase_as_unstarted() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_store(dir.path());
        app.config.alert_screen = true;
        app.advance_clock(Duration::from_secs(10)); // work done -> break alert
        app.handle_key(KeyCode::Char('s'), KeyModifiers::NONE); // skip the break
        let records = load_records(&app);
        assert_eq!(records.len(), 2);
        assert_eq!(records[1].phase, Phase::ShortBreak);
        assert_eq!(records[1].actual_secs, 0);
        assert!(!records[1].completed);
    }

    #[test]
    fn no_store_means_no_recording_and_no_crash() {
        let mut app = app_on_timer(false);
        app.advance_clock(Duration::from_secs(10));
        assert!(app.store.is_none());
        assert!(app.status.is_none());
    }

    #[test]
    fn t_opens_stats_and_esc_closes_without_quitting() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_store(dir.path());
        app.handle_key(KeyCode::Char('t'), KeyModifiers::NONE);
        assert!(app.stats_view.is_some());
        app.handle_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.stats_view.is_none());
        assert!(!app.should_quit);
    }

    #[test]
    fn t_also_closes_the_stats_overlay() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_store(dir.path());
        app.handle_key(KeyCode::Char('t'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Char('t'), KeyModifiers::NONE);
        assert!(app.stats_view.is_none());
    }

    #[test]
    fn timer_keeps_ticking_under_the_stats_overlay() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_store(dir.path());
        app.handle_key(KeyCode::Char('t'), KeyModifiers::NONE);
        app.advance_clock(Duration::from_secs(3));
        assert_eq!(remaining(&app), Duration::from_secs(7));
    }

    #[test]
    fn t_is_ignored_while_an_alert_is_up() {
        let mut app = app_on_timer(true);
        app.advance_clock(Duration::from_secs(10)); // -> alert
        app.handle_key(KeyCode::Char('t'), KeyModifiers::NONE);
        assert!(app.stats_view.is_none());
        assert_eq!(app.alert, Some(Phase::ShortBreak));
    }

    #[test]
    fn esc_still_quits_when_no_overlay_is_open() {
        let mut app = app_on_timer(false);
        app.handle_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.should_quit);
    }

    #[test]
    fn stats_opens_from_the_menu_too() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(
            app_on_timer(false).config,
            Some(Store::new(dir.path().to_path_buf())),
            ThemeMode::Dark,
            false,
        );
        app.handle_key(KeyCode::Char('t'), KeyModifiers::NONE);
        assert!(app.stats_view.is_some());
    }

    #[test]
    fn alert_evicts_the_stats_overlay() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_store(dir.path());
        app.config.alert_screen = true;
        app.handle_key(KeyCode::Char('t'), KeyModifiers::NONE);
        app.advance_clock(Duration::from_secs(10)); // work ends under the overlay
        assert!(app.stats_view.is_none());
        assert_eq!(app.alert, Some(Phase::ShortBreak));
    }
}
