# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Development
cargo build
cargo run
cargo test

# Run with arguments
cargo run -- -w 45m -b 10m
cargo run -- --no-notify

# Nix
nix develop     # dev shell (rust toolchain + libnotify)
nix build       # package with notify-send wrapped onto PATH
```

## Architecture

Focus Fox is a terminal pomodoro timer built with ratatui. Same stack and
module layout as sweet-nothings.

### Module Structure

- **`src/timer/`** - Pomodoro state machine (`Timer`): Work → ShortBreak,
  with a LongBreak every N naturally-completed work sessions. Skipped work
  sessions don't count toward the long break. All logic is pure and unit
  tested here.
- **`src/tui/`** - Terminal UI with two screens (`Screen` enum in `app.rs`):
  a configuration menu shown at launch (arrow keys adjust settings, Enter
  starts; every adjustment auto-saves to the config file so settings
  persist between runs) and the timer screen. `app.rs`
  owns the event loop (100ms tick, keyboard handling), `ui.rs` renders the
  menu and the big block-digit clock, progress gauge, and session dots.
- **`src/notify/`** - Best-effort desktop notifications by shelling out to
  `notify-send`; failures never interrupt the timer.
- **`src/config/`** - XDG config (`~/.config/focus-fox/config.toml`, TOML,
  humantime durations). CLI args override file values via `merge_args`.
- **`src/cli/`** - Clap argument parsing.

### Notes

- The crate ships two identical binaries, `focus-fox` and `fox` (two
  `[[bin]]` entries pointing at `src/main.rs`; `default-run = "focus-fox"`
  keeps plain `cargo run` working).
- crossterm is used via the `ratatui::crossterm` re-export — don't add a
  separate crossterm dependency (version-mismatch risk).
- Keep timer logic in `src/timer/` free of TUI/IO concerns so it stays
  testable.
