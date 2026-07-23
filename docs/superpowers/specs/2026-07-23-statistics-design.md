# Focus Fox statistics — design

Date: 2026-07-23
Status: approved

## Goal

Answer three questions from inside the app or from the shell, with minimal
UI to start:

1. How much did I focus today / this week?
2. Am I keeping a streak?
3. What does my full session history look like?

## Data: append-only JSONL event log, partitioned by year

New module `src/stats/`.

Every phase that ends — naturally or via skip — appends one JSON line to:

```
~/.local/share/focus-fox/history-<year>.jsonl
```

The directory comes from the same `ProjectDirs::from("dev", "jordangarrison",
"focus-fox")` triple the config module uses, but the **data** dir rather than
the config dir. The year in the filename is derived from the record's local
end-timestamp at write time; that is the entire "rotation" mechanism. At
heavy use (~30–40 records/day, ~150 bytes each) a year file tops out around
1–2 MB, so a linear scan stays trivially fast forever.

### Record shape

```json
{"at":"2026-07-23T14:30:00-05:00","phase":"work","planned_secs":1500,"actual_secs":1500,"completed":true}
```

- `at` — RFC3339 end-timestamp **in local time**, so day-bucketing matches
  the wall clock.
- `phase` — `"work"`, `"short_break"`, or `"long_break"`. All phases are
  logged, including breaks.
- `planned_secs` — the phase's configured length at the time it ran.
- `actual_secs` — timer time elapsed (`total − remaining`). Pauses never
  inflate it; a skipped phase records the time actually spent.
- `completed` — `true` for a natural finish, `false` for a skip.

### New dependencies

`serde_json` (record serialization) and `chrono` (timestamps, day/week
bucketing). No SQLite; at this volume SQL buys nothing a fold can't do.

## Stats computation: pure, like `timer/`

`stats::Summary::compute(&[Record], today: NaiveDate) -> Summary` is a pure
function with no IO, unit-tested directly. It produces:

- **Today**: completed work sessions, total focus time (`actual_secs` of
  work records, completed or not).
- **This week** (Monday start): same pair.
- **Streak**: consecutive days with ≥1 *completed* work session, counting
  back from today, or from yesterday if today has none yet (an empty
  morning doesn't read as a broken streak).
- **Lifetime**: total completed work sessions and focus time across all
  loaded records.
- **Recent**: the last 10 records, newest first, for a history list.

File IO lives in a small `stats::store` submodule:

- `append(record)` — best-effort append to the current year's file,
  creating the directory/file as needed.
- `load_recent()` — read the current year's file, plus the previous year's
  only when it exists (covers streaks/weeks spanning New Year's).
  Unparseable lines are skipped, not fatal.

## Recording hook

The `Timer` stays pure and IO-free. `App` (in `tui/app.rs`) observes phase
transitions — it already does, for notifications — and builds the record
there: it captures the outgoing phase, `total`, and `remaining` before
calling `tick()`/`skip()`, then appends via the store when a transition
happens. A failed append surfaces in the existing status line
(`app.status`) and never interrupts the timer — same best-effort philosophy
as `notify/`.

## TUI: stats as an overlay

Pressing `t` (from the menu or the timer screen) toggles a stats panel
rendered over the current screen — the same modal pattern as the existing
alert overlay (`App.alert`). This matters because `Screen::Timer` owns the
`Timer`; a separate `Screen` variant would drop a running timer, while an
overlay lets it keep ticking underneath. `t` or `Esc` closes the panel
(`Esc` therefore no longer quits the app while the panel is open; `q` and
Ctrl-C still do). While the alert overlay is up, `t` is ignored — the
alert keeps priority.

Panel contents (minimal to start): today, this week, streak, lifetime, and
the recent-sessions list.

## CLI: `focus-fox stats`

`cli::Args` grows a clap subcommand enum. Plain `focus-fox` behaves exactly
as today; `focus-fox stats` prints the same summary as plain text to stdout
and exits without entering the TUI. Existing flags are unaffected.

## Error handling

- Append failures: status-line message, timer uninterrupted.
- Missing data dir/file: treated as empty history.
- Corrupt lines: skipped on read.
- `focus-fox stats` with no history: prints a friendly "no sessions yet"
  message, exit 0.

## Testing

- Pure fold tests in `src/stats/`: day bucketing (including a session
  ending just after midnight), streak with a gap, streak counted from
  yesterday, week boundary, year boundary spanning two files' worth of
  records, skipped-vs-completed accounting.
- Store round-trip test against a temp dir (append then load, corrupt line
  skipped).
- App-level test that a finished phase appends a record and a skip appends
  a `completed:false` record (store pointed at a temp dir).

## Out of scope (YAGNI)

- Charts/graphs, per-tag or per-project tracking, exporting, configurable
  streak thresholds, file compaction/rotation beyond the year partition.
