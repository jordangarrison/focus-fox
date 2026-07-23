# Focus Fox Statistics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Log every finished/skipped pomodoro phase to a year-partitioned JSONL file and show today/week/streak/lifetime stats via a TUI overlay (`t`) and a `focus-fox stats` subcommand.

**Architecture:** A new `src/stats/` module mirrors the codebase's existing split: a pure, IO-free core (`Record`, `Summary::compute`, `render_text`) unit-tested like `src/timer/`, plus a small `store` submodule owning file IO (`~/.local/share/focus-fox/history-<year>.jsonl`). `App` records phase ends best-effort (like `notify/`), and the TUI shows stats as an overlay (like the existing alert) so the running timer is never dropped.

**Tech Stack:** Rust, ratatui, clap subcommands, serde_json, chrono. Spec: `docs/superpowers/specs/2026-07-23-statistics-design.md`.

**Conventions:** Run all commands from the repo root (`/home/jordangarrison/dev/jordangarrison/focus-fox`). If `cargo` is missing from the environment, use `nix develop -c cargo <args>`. Never force-push.

---

## File structure

- Create: `src/stats/mod.rs` — `Record`, `Summary::compute` (pure fold), `render_text`, `fmt_focus`, `print` (CLI entry)
- Create: `src/stats/store.rs` — `Store`: XDG data dir, `append`, `load_recent`
- Modify: `src/timer/mod.rs` — serde derives on `Phase` (stays IO-free)
- Modify: `src/tui/app.rs` — record phase ends; `stats_view` overlay state; `t`/`Esc` keys
- Modify: `src/tui/ui.rs` — `render_stats` overlay; `frame_block` takes a title; help lines
- Modify: `src/tui/mod.rs` — construct the store, pass into `App::new`
- Modify: `src/cli/mod.rs` — `Command::Stats` subcommand
- Modify: `src/main.rs` — `mod stats;` + subcommand dispatch
- Modify: `Cargo.toml` — `serde_json`, `chrono`; dev-dep `tempfile`
- Modify: `CLAUDE.md` — document the stats module

---

### Task 1: Record type + dependencies

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/timer/mod.rs` (Phase enum, ~line 5)
- Modify: `src/main.rs`
- Create: `src/stats/mod.rs`

- [ ] **Step 1: Add dependencies**

In `Cargo.toml`, append to `[dependencies]` and add a dev-dependencies section at the end of the file:

```toml
# Statistics
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Give `timer::Phase` serde derives**

In `src/timer/mod.rs`, add the import and change the `Phase` derive block. The enum body is unchanged:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Work,
    ShortBreak,
    LongBreak,
}
```

(Serde derives keep `timer/` free of IO — this is the same line `config/` already walks.)

- [ ] **Step 3: Register the module**

In `src/main.rs`, add `mod stats;` to the module list (alphabetical, after `mod notify;`).

- [ ] **Step 4: Write the failing test**

Create `src/stats/mod.rs`:

```rust
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

use crate::timer::Phase;

/// One finished or skipped phase, as written to the history log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// End of the phase, local time.
    pub at: DateTime<FixedOffset>,
    pub phase: Phase,
    /// Configured phase length when it ran.
    pub planned_secs: u64,
    /// Timer time actually elapsed (pauses excluded).
    pub actual_secs: u64,
    /// True for a natural finish, false for a skip.
    pub completed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_round_trips_through_json() {
        let record = Record {
            at: DateTime::parse_from_rfc3339("2026-07-23T14:30:00-05:00").unwrap(),
            phase: Phase::Work,
            planned_secs: 1500,
            actual_secs: 1400,
            completed: true,
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"phase\":\"work\""), "json was: {json}");
        assert!(json.contains("\"completed\":true"));
        let back: Record = serde_json::from_str(&json).unwrap();
        assert_eq!(back, record);
    }

    #[test]
    fn break_phases_use_snake_case() {
        assert_eq!(
            serde_json::to_string(&Phase::ShortBreak).unwrap(),
            "\"short_break\""
        );
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test stats`
Expected: both tests PASS (the "failing" state here is the compile error before Steps 1–3 are complete; once it compiles, serde derives do the work). Also run `cargo test` to confirm nothing else broke.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/timer/mod.rs src/main.rs src/stats/mod.rs
git commit -m "feat: add session record type for statistics"
```

---

### Task 2: `Summary::compute` — the pure fold

**Files:**
- Modify: `src/stats/mod.rs`

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module in `src/stats/mod.rs` (add `use chrono::NaiveDate;` and `use std::time::Duration;` inside the tests module):

```rust
    fn rec(day: &str, time: &str, phase: Phase, actual_mins: u64, completed: bool) -> Record {
        Record {
            at: DateTime::parse_from_rfc3339(&format!("{day}T{time}:00-05:00")).unwrap(),
            phase,
            planned_secs: 1500,
            actual_secs: actual_mins * 60,
            completed,
        }
    }

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    // 2024-01-01 was a Monday; "today" is Wednesday 2024-01-03 throughout.
    const TODAY: &str = "2024-01-03";

    #[test]
    fn today_counts_completed_sessions_but_credits_partial_focus() {
        let records = vec![
            rec(TODAY, "09:00", Phase::Work, 25, true),
            rec(TODAY, "09:40", Phase::Work, 10, false), // skip: focus counts, session doesn't
            rec(TODAY, "10:10", Phase::ShortBreak, 5, true), // breaks never count as focus
            rec("2024-01-02", "10:00", Phase::Work, 25, true), // yesterday
        ];
        let s = Summary::compute(&records, d(TODAY));
        assert_eq!(s.today_sessions, 1);
        assert_eq!(s.today_focus, Duration::from_secs(35 * 60));
    }

    #[test]
    fn week_starts_monday() {
        let records = vec![
            rec("2023-12-31", "10:00", Phase::Work, 25, true), // Sunday: last week
            rec("2024-01-01", "10:00", Phase::Work, 25, true), // Monday: this week
            rec(TODAY, "10:00", Phase::Work, 25, true),
        ];
        let s = Summary::compute(&records, d(TODAY));
        assert_eq!(s.week_sessions, 2);
        assert_eq!(s.week_focus, Duration::from_secs(50 * 60));
        assert_eq!(s.lifetime_sessions, 3);
        assert_eq!(s.lifetime_focus, Duration::from_secs(75 * 60));
    }

    #[test]
    fn streak_counts_consecutive_days() {
        let records = vec![
            rec("2024-01-01", "10:00", Phase::Work, 25, true),
            rec("2024-01-02", "10:00", Phase::Work, 25, true),
            rec(TODAY, "00:10", Phase::Work, 25, true), // just after midnight still today
        ];
        assert_eq!(Summary::compute(&records, d(TODAY)).streak_days, 3);
    }

    #[test]
    fn streak_breaks_on_a_gap() {
        let records = vec![
            rec("2024-01-01", "10:00", Phase::Work, 25, true),
            rec(TODAY, "10:00", Phase::Work, 25, true), // nothing on the 2nd
        ];
        assert_eq!(Summary::compute(&records, d(TODAY)).streak_days, 1);
    }

    #[test]
    fn empty_morning_does_not_break_the_streak() {
        let records = vec![
            rec("2024-01-01", "10:00", Phase::Work, 25, true),
            rec("2024-01-02", "10:00", Phase::Work, 25, true),
        ];
        // No session yet today: streak counts back from yesterday.
        assert_eq!(Summary::compute(&records, d(TODAY)).streak_days, 2);
    }

    #[test]
    fn skips_and_breaks_do_not_extend_a_streak() {
        let records = vec![
            rec("2024-01-02", "10:00", Phase::Work, 10, false),
            rec(TODAY, "10:00", Phase::ShortBreak, 5, true),
        ];
        assert_eq!(Summary::compute(&records, d(TODAY)).streak_days, 0);
    }

    #[test]
    fn recent_is_last_ten_newest_first() {
        let records: Vec<Record> = (0..12)
            .map(|i| rec(TODAY, &format!("{:02}:00", 8 + i), Phase::Work, 25, true))
            .collect();
        let s = Summary::compute(&records, d(TODAY));
        assert_eq!(s.recent.len(), 10);
        assert_eq!(s.recent[0], records[11]);
        assert_eq!(s.recent[9], records[2]);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test stats`
Expected: FAIL to compile — `Summary` not found.

- [ ] **Step 3: Implement `Summary::compute`**

Add to `src/stats/mod.rs` (below `Record`; add the imports to the top of the file):

```rust
use std::collections::HashSet;
use std::time::Duration;

use chrono::{Days, NaiveDate, Weekday};
```

```rust
/// Aggregates over the history, computed as a pure fold — no IO here.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub today_sessions: u32,
    pub today_focus: Duration,
    pub week_sessions: u32,
    pub week_focus: Duration,
    /// Consecutive days with >=1 completed work session, counting back from
    /// today — or from yesterday when today has none yet.
    pub streak_days: u32,
    pub lifetime_sessions: u32,
    pub lifetime_focus: Duration,
    /// Last 10 records, newest first.
    pub recent: Vec<Record>,
}

impl Summary {
    pub fn compute(records: &[Record], today: NaiveDate) -> Self {
        let week_start = today.week(Weekday::Mon).first_day();
        let mut s = Summary {
            today_sessions: 0,
            today_focus: Duration::ZERO,
            week_sessions: 0,
            week_focus: Duration::ZERO,
            streak_days: 0,
            lifetime_sessions: 0,
            lifetime_focus: Duration::ZERO,
            recent: records.iter().rev().take(10).cloned().collect(),
        };

        let mut completed_days: HashSet<NaiveDate> = HashSet::new();
        for r in records {
            if r.phase != Phase::Work {
                continue;
            }
            let date = r.at.date_naive();
            let focus = Duration::from_secs(r.actual_secs);
            let in_week = (week_start..=today).contains(&date);
            s.lifetime_focus += focus;
            if date == today {
                s.today_focus += focus;
            }
            if in_week {
                s.week_focus += focus;
            }
            if r.completed {
                s.lifetime_sessions += 1;
                completed_days.insert(date);
                if date == today {
                    s.today_sessions += 1;
                }
                if in_week {
                    s.week_sessions += 1;
                }
            }
        }

        let mut day = if completed_days.contains(&today) {
            today
        } else {
            today - Days::new(1)
        };
        while completed_days.contains(&day) {
            s.streak_days += 1;
            day = day - Days::new(1);
        }
        s
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test stats`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/stats/mod.rs
git commit -m "feat: compute pomodoro statistics summary"
```

---

### Task 3: JSONL store, partitioned by year

**Files:**
- Create: `src/stats/store.rs`
- Modify: `src/stats/mod.rs` (add `pub mod store;` at the top)

- [ ] **Step 1: Write the failing tests**

Create `src/stats/store.rs`:

```rust
use anyhow::{Context, Result};
use chrono::Datelike;
use directories::ProjectDirs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use super::Record;

/// Appends history records to, and loads them from, year-partitioned JSONL
/// files (`history-<year>.jsonl`) in the XDG data directory.
pub struct Store {
    dir: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::Phase;
    use chrono::DateTime;

    fn rec(rfc3339: &str, completed: bool) -> Record {
        Record {
            at: DateTime::parse_from_rfc3339(rfc3339).unwrap(),
            phase: Phase::Work,
            planned_secs: 1500,
            actual_secs: 1500,
            completed,
        }
    }

    #[test]
    fn append_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let a = rec("2024-01-02T10:00:00-05:00", true);
        let b = rec("2024-01-02T11:00:00-05:00", false);
        store.append(&a).unwrap();
        store.append(&b).unwrap();
        assert_eq!(store.load_recent(2024), vec![a, b]);
    }

    #[test]
    fn records_land_in_their_years_file_and_both_years_load() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let old = rec("2023-12-31T23:00:00-05:00", true);
        let new = rec("2024-01-01T10:00:00-05:00", true);
        store.append(&old).unwrap();
        store.append(&new).unwrap();
        assert!(dir.path().join("history-2023.jsonl").exists());
        assert!(dir.path().join("history-2024.jsonl").exists());
        assert_eq!(store.load_recent(2024), vec![old, new]);
        // A load from 2023's perspective doesn't see the future file.
        assert_eq!(store.load_recent(2023), vec![old]);
    }

    #[test]
    fn corrupt_lines_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let good = rec("2024-01-02T10:00:00-05:00", true);
        store.append(&good).unwrap();
        std::fs::write(
            dir.path().join("history-2024.jsonl"),
            format!("not json\n{}\n", serde_json::to_string(&good).unwrap()),
        )
        .unwrap();
        assert_eq!(store.load_recent(2024), vec![good]);
    }

    #[test]
    fn missing_files_mean_empty_history() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        assert_eq!(store.load_recent(2024), Vec::<Record>::new());
    }
}
```

And in `src/stats/mod.rs`, add as the first line:

```rust
pub mod store;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test stats::store`
Expected: FAIL to compile — `Store::new`, `append`, `load_recent` missing.

- [ ] **Step 3: Implement the store**

Add to `src/stats/store.rs`, between the struct and the tests module:

```rust
impl Store {
    /// XDG data dir (~/.local/share/focus-fox on Linux) — same ProjectDirs
    /// triple as Config, but the data dir rather than the config dir.
    pub fn default_dir() -> Option<PathBuf> {
        ProjectDirs::from("dev", "jordangarrison", "focus-fox")
            .map(|dirs| dirs.data_dir().to_path_buf())
    }

    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Append one record to the file for the record's own year.
    pub fn append(&self, record: &Record) -> Result<()> {
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("creating {}", self.dir.display()))?;
        let path = self.dir.join(format!("history-{}.jsonl", record.at.year()));
        let line = serde_json::to_string(record).context("serializing history record")?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("opening {}", path.display()))?;
        writeln!(file, "{line}").with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    /// Load `year - 1` and `year` — enough for every stat we compute
    /// (streaks and weeks can span New Year's). Missing files are empty
    /// history; unparseable lines are skipped.
    pub fn load_recent(&self, year: i32) -> Vec<Record> {
        let mut records = Vec::new();
        for y in [year - 1, year] {
            let path = self.dir.join(format!("history-{y}.jsonl"));
            let Ok(contents) = std::fs::read_to_string(&path) else {
                continue;
            };
            records.extend(
                contents
                    .lines()
                    .filter_map(|line| serde_json::from_str::<Record>(line).ok()),
            );
        }
        records
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test stats`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/stats/store.rs src/stats/mod.rs
git commit -m "feat: add year-partitioned jsonl history store"
```

---

### Task 4: Plain-text rendering (shared by CLI, formats reused by TUI)

**Files:**
- Modify: `src/stats/mod.rs`

- [ ] **Step 1: Write the failing tests**

Append to the tests module in `src/stats/mod.rs`:

```rust
    #[test]
    fn render_text_shows_the_summary() {
        let records = vec![
            rec("2024-01-02", "10:00", Phase::Work, 25, true),
            rec(TODAY, "09:00", Phase::Work, 25, true),
            rec(TODAY, "09:30", Phase::ShortBreak, 5, true),
        ];
        let text = render_text(&Summary::compute(&records, d(TODAY)));
        assert!(text.contains("Today      1 sessions · 25m"), "text was:\n{text}");
        assert!(text.contains("Streak     2 days"));
        assert!(text.contains("Lifetime   2 sessions · 50m"));
        assert!(text.contains("Short Break"));
    }

    #[test]
    fn render_text_with_no_history_is_friendly() {
        let text = render_text(&Summary::compute(&[], d(TODAY)));
        assert!(text.contains("No sessions yet"));
    }

    #[test]
    fn fmt_focus_rounds_to_minutes() {
        assert_eq!(fmt_focus(Duration::from_secs(1499)), "24m");
        assert_eq!(fmt_focus(Duration::from_secs(4500)), "1h 15m");
        assert_eq!(fmt_focus(Duration::from_secs(30)), "30s");
        assert_eq!(fmt_focus(Duration::ZERO), "0s");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test stats`
Expected: FAIL to compile — `render_text`, `fmt_focus` missing.

- [ ] **Step 3: Implement**

Add to `src/stats/mod.rs`:

```rust
/// Human duration for stat lines: whole minutes once >=1m, seconds below.
pub fn fmt_focus(d: Duration) -> String {
    let mins = d.as_secs() / 60;
    if mins == 0 {
        return format!("{}s", d.as_secs());
    }
    humantime::format_duration(Duration::from_secs(mins * 60)).to_string()
}

/// One line of the recent-history list, shared by CLI and TUI.
pub fn recent_line(r: &Record) -> String {
    format!(
        "{}  {:<11} {:>7}  {}",
        r.at.format("%a %H:%M"),
        r.phase.label(),
        fmt_focus(Duration::from_secs(r.actual_secs)),
        if r.completed { "✓" } else { "⨯" },
    )
}

/// Plain-text summary for `focus-fox stats`.
pub fn render_text(s: &Summary) -> String {
    if s.recent.is_empty() {
        return "No sessions yet — run focus-fox and finish one. 🦊\n".to_string();
    }
    let mut out = String::new();
    out.push_str(&format!(
        "Today      {} sessions · {}\n",
        s.today_sessions,
        fmt_focus(s.today_focus)
    ));
    out.push_str(&format!(
        "This week  {} sessions · {}\n",
        s.week_sessions,
        fmt_focus(s.week_focus)
    ));
    out.push_str(&format!("Streak     {} days\n", s.streak_days));
    out.push_str(&format!(
        "Lifetime   {} sessions · {}\n",
        s.lifetime_sessions,
        fmt_focus(s.lifetime_focus)
    ));
    out.push_str("\nRecent:\n");
    for r in &s.recent {
        out.push_str(&format!("  {}\n", recent_line(r)));
    }
    out
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test stats`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/stats/mod.rs
git commit -m "feat: render stats summary as plain text"
```

---

### Task 5: Record phase ends from the App

**Files:**
- Modify: `src/tui/app.rs`
- Modify: `src/tui/mod.rs`

- [ ] **Step 1: Write the failing tests**

Append to the tests module in `src/tui/app.rs`:

```rust
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
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test tui`
Expected: FAIL to compile — `App` has no `store` field and `App::new` takes one argument.

- [ ] **Step 3: Implement recording**

In `src/tui/app.rs`:

Add imports at the top:

```rust
use crate::stats::{Record, store::Store};
```

Add the fields to `App` (after `alert`):

```rust
    /// History log; None when the data dir can't be determined (recording
    /// is best-effort, like notifications).
    pub store: Option<Store>,
```

Change `App::new` to take the store:

```rust
    pub fn new(config: Config, store: Option<Store>) -> Self {
        Self {
            config,
            screen: Screen::Menu { selected: 0 },
            status: None,
            alert: None,
            store,
            should_quit: false,
        }
    }
```

Replace `advance_clock` — capture the ending phase before ticking (a natural finish elapses the full planned duration):

```rust
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
                }
            }
        }
    }
```

In `handle_key`, replace the alert-skip arm (the `KeyCode::Char('s')` arm inside `if self.alert.is_some()`) — the announced phase never ran, so it logs zero elapsed:

```rust
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
```

Replace the timer-screen skip arm (`KeyCode::Char('s')` inside `Screen::Timer(timer) => match code`):

```rust
                KeyCode::Char('s') => {
                    let (ended, planned, elapsed) =
                        (timer.phase, timer.total, timer.total - timer.remaining);
                    let phase = timer.skip();
                    record_phase_end(&self.store, &mut self.status, ended, planned, elapsed, false);
                    announce(&self.config, phase);
                }
```

Add the free function next to `announce` at the bottom of the file (a free function so it can borrow `store`/`status` while the screen's timer is mutably borrowed):

```rust
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
```

Update the two existing test constructors to the new signature: in `app_on_timer`, change `App::new(config.clone())` to `App::new(config.clone(), None)`.

In `src/tui/mod.rs`, wire the real store:

```rust
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: all PASS (including the pre-existing alert/skip tests, which now pass `None` and record nothing).

- [ ] **Step 5: Commit**

```bash
git add src/tui/app.rs src/tui/mod.rs
git commit -m "feat: record finished and skipped phases to history"
```

---

### Task 6: TUI stats overlay

**Files:**
- Modify: `src/tui/app.rs`
- Modify: `src/tui/ui.rs`

- [ ] **Step 1: Write the failing tests**

Append to the tests module in `src/tui/app.rs`:

```rust
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
        let mut app = App::new(app_on_timer(false).config, Some(Store::new(dir.path().to_path_buf())));
        app.handle_key(KeyCode::Char('t'), KeyModifiers::NONE);
        assert!(app.stats_view.is_some());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test tui`
Expected: FAIL to compile — no `stats_view` field.

- [ ] **Step 3: Implement overlay state and keys**

In `src/tui/app.rs`:

Extend the stats import and add `Summary`:

```rust
use crate::stats::{Record, Summary, store::Store};
```

Add the field to `App` (after `store`) and initialize it to `None` in `App::new`:

```rust
    /// When set, a stats panel is drawn over the current screen. The timer
    /// keeps ticking underneath.
    pub stats_view: Option<Summary>,
```

Restructure the top of `handle_key`. Replace the current quit-key match and alert check with (the rest of the function — the `Screen::Menu`/`Screen::Timer` match — stays):

```rust
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
            self.should_quit = true;
            return;
        }
        if self.alert.is_some() {
            // ... existing alert handling, unchanged ...
            return;
        }
        if code == KeyCode::Char('t') {
            self.open_stats();
            return;
        }
        match &mut self.screen {
            // ... existing menu/timer handling, unchanged ...
        }
    }
```

Add `open_stats` as a method on `App`:

```rust
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: all PASS.

- [ ] **Step 5: Render the overlay**

In `src/tui/ui.rs`:

Add imports: `Clear` to the widgets import, and the stats module:

```rust
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::stats::{self, Summary};
```

Give `frame_block` a title parameter and update its three existing callers (`render_menu`, `render_timer`, `render_alert`) to pass `" 🦊 Focus Fox "`:

```rust
fn frame_block(frame: &mut Frame, accent: Color, title: &str) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(title.to_string())
        .title_alignment(Alignment::Center);
    let inner = block.inner(frame.area());
    frame.render_widget(block, frame.area());
    inner
}
```

Extend `render` to draw the overlay last, on top of whatever screen is showing:

```rust
pub fn render(frame: &mut Frame, app: &App) {
    match (&app.screen, app.alert) {
        (Screen::Timer(timer), Some(phase)) => render_alert(frame, timer, phase),
        (Screen::Timer(timer), None) => render_timer(frame, timer),
        (Screen::Menu { selected }, _) => render_menu(frame, app, *selected),
    }
    if let Some(summary) = &app.stats_view {
        render_stats(frame, summary);
    }
}
```

Add the stats panel (near `render_alert`):

```rust
// --- Stats overlay ---

/// Full-frame stats panel drawn over the current screen; the timer keeps
/// ticking underneath.
fn render_stats(frame: &mut Frame, s: &Summary) {
    frame.render_widget(Clear, frame.area());
    let inner = frame_block(frame, FOX, " 🦊 Stats ");

    let mut lines: Vec<Line> = vec![
        stat_line("Today", s.today_sessions, s.today_focus),
        stat_line("This week", s.week_sessions, s.week_focus),
        Line::from(format!("{:<10} {} days", "Streak", s.streak_days)),
        stat_line("Lifetime", s.lifetime_sessions, s.lifetime_focus),
    ];
    if !s.recent.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "Recent",
            Style::default().fg(FOX).add_modifier(Modifier::BOLD),
        ));
        lines.extend(s.recent.iter().map(|r| {
            Line::styled(stats::recent_line(r), Style::default().fg(Color::Gray))
        }));
    } else {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "No sessions yet — finish one. 🦊",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(lines.len() as u16),
            Constraint::Fill(1),
            Constraint::Length(1), // key help
        ])
        .split(inner);

    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), rows[1]);
    render_help(frame, rows[3], "t/esc close");
}

fn stat_line(label: &str, sessions: u32, focus: std::time::Duration) -> Line<'static> {
    Line::from(format!(
        "{label:<10} {sessions} sessions · {}",
        stats::fmt_focus(focus)
    ))
}
```

Update the two help lines to mention the new key:

- `render_menu`: `"↑↓ select · ←→ adjust · enter start · t stats · q quit"`
- `render_timer`: `"space pause · s skip · r reset · t stats · m menu · q quit"`

- [ ] **Step 6: Verify build, tests, and by hand**

Run: `cargo test` — expected: all PASS.
Run: `cargo run` — press `t` on the menu: stats panel appears; `Esc` closes it (app stays open); start a timer, press `t` again, watch the clock keep moving after closing. `q` quits from anywhere.

- [ ] **Step 7: Commit**

```bash
git add src/tui/app.rs src/tui/ui.rs
git commit -m "feat: add stats overlay to the tui"
```

---

### Task 7: `focus-fox stats` subcommand + docs

**Files:**
- Modify: `src/cli/mod.rs`
- Modify: `src/stats/mod.rs`
- Modify: `src/main.rs`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add the subcommand**

In `src/cli/mod.rs`, change the imports and add the subcommand to `Args` (existing flags unchanged):

```rust
use clap::{Parser, Subcommand};
```

```rust
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Print session statistics and exit
    Stats,
}
```

And as the first field of `Args`:

```rust
    #[command(subcommand)]
    pub command: Option<Command>,
```

- [ ] **Step 2: Add the CLI entry point to stats**

Add to `src/stats/mod.rs`:

```rust
/// Entry point for `focus-fox stats`: load history, print, exit.
pub fn print() -> anyhow::Result<()> {
    use chrono::Datelike;
    let now = chrono::Local::now();
    let records = store::Store::default_dir()
        .map(|dir| store::Store::new(dir).load_recent(now.year()))
        .unwrap_or_default();
    print!("{}", render_text(&Summary::compute(&records, now.date_naive())));
    Ok(())
}
```

- [ ] **Step 3: Dispatch in main**

Replace `src/main.rs`'s `main`:

```rust
fn main() -> Result<()> {
    let args = cli::Args::parse();
    if let Some(cli::Command::Stats) = args.command {
        return stats::print();
    }
    let config = config::Config::load()?.merge_args(&args);
    tui::run(config)
}
```

- [ ] **Step 4: Verify**

Run: `cargo test` — expected: all PASS.
Run: `cargo run -- stats` — expected: either "No sessions yet — run focus-fox and finish one. 🦊" or your real summary, then exit 0 without entering the TUI.
Run: `cargo run -- --help` — expected: `stats` listed as a subcommand, existing flags intact.

- [ ] **Step 5: Update CLAUDE.md**

In `CLAUDE.md`'s Module Structure section, add after the `src/notify/` bullet:

```markdown
- **`src/stats/`** - Session history and statistics. Every phase that ends
  (naturally or skipped) is appended as one JSON line to
  `~/.local/share/focus-fox/history-<year>.jsonl` (year-partitioned; the
  filename comes from the record's timestamp — that's the whole rotation
  mechanism). `Summary::compute` is a pure fold over records (today/week/
  streak/lifetime), unit tested like the timer. Viewed with `t` in the TUI
  (an overlay — the timer keeps ticking) or `focus-fox stats` on the CLI.
  Recording is best-effort: failures surface in the status line, never
  interrupt the timer.
```

- [ ] **Step 6: Final check and commit**

Run: `cargo test && cargo clippy -- -D warnings` (if clippy is unavailable, `cargo test` alone).
Expected: PASS, no warnings.

```bash
git add src/cli/mod.rs src/stats/mod.rs src/main.rs CLAUDE.md
git commit -m "feat: add stats subcommand"
```
