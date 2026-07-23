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

# Release assets (linux: also .#deb, .#rpm, .#arch, .#static, .#tarball)
nix build .#release   # every downloadable asset for this system in ./result
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
  persist between runs) and the timer screen. When a phase ends naturally
  (and the "Alert screen" setting is on), a full-screen alert freezes the
  timer until Enter is pressed (`App.alert`); manual skips bypass it.
  `app.rs` owns the event loop (100ms tick, keyboard handling), `ui.rs`
  renders the menu, the alert banner, and the big block-digit clock,
  progress gauge, and session dots.
- **`src/notify/`** - Best-effort desktop notifications by shelling out to
  `notify-send`; failures never interrupt the timer.
- **`src/stats/`** - Session history and statistics. Every phase that ends
  (naturally or skipped) is appended as one JSON line to
  `~/.local/share/focus-fox/history-<year>.jsonl` (year-partitioned; the
  filename comes from the record's timestamp — that's the whole rotation
  mechanism). `Summary::compute` is a pure fold over records (today/week/
  streak/lifetime), unit tested like the timer. Viewed with `t` in the TUI
  (an overlay — the timer keeps ticking) or `focus-fox stats` on the CLI.
  Recording is best-effort: failures surface in the status line, never
  interrupt the timer.
- **`src/config/`** - XDG config (`~/.config/focus-fox/config.toml`, TOML,
  humantime durations). CLI args override file values via `merge_args`.
- **`src/cli/`** - Clap argument parsing.

## Releases

Fully automated via release-please + nix. The flow:

1. Land conventional commits on `main`. `feat:`/`fix:` accumulate into a
   release-please PR (`chore(main): release ...`); use `ci:`/`chore:`/
   `docs:` for changes that shouldn't trigger a release.
2. Merging that PR bumps `Cargo.toml`/`Cargo.lock`, updates
   `CHANGELOG.md`, tags `vX.Y.Z`, and creates the GitHub release
   (`release-please.yml`, config in `release-please-config.json` +
   `.release-please-manifest.json`).
3. `release-please.yml` then dispatches `release.yml` onto the new tag —
   required because tags created with `GITHUB_TOKEN` don't fire tag-push
   workflows.
4. `release.yml` runs `nix build .#release` on three runners
   (`ubuntu-latest`, `ubuntu-24.04-arm`, `macos-14`) and uploads
   everything with `gh release upload`.

Notes:

- All assets come from the flake — never add rustup/cargo steps to CI;
  reproducibility from `flake.lock` is the point.
- Linux assets are built from a static musl binary (`.#static`); the
  deb/rpm/arch packages are generated from it with nfpm. macOS is a
  plain tarball of the native aarch64-darwin build.
- The flake reads `version` from `Cargo.toml`, so release-please bumps
  flow through automatically. `.release-please-manifest.json` tracks the
  released version; tags are plain `vX.Y.Z`
  (`include-component-in-tag: false` — don't remove it, component tags
  like `focus-fox-v0.1.0` break the `v*` trigger and tag history).
- No x86_64-darwin / universal mac build: nixpkgs 26.11 dropped the
  platform, and the flake enumerates supported systems explicitly
  (don't switch back to `eachDefaultSystem`).
- Manual escape hatch: pushing a `v*` tag by hand also triggers
  `release.yml`, which creates the GitHub release if missing.

### Notes

- The crate ships two identical binaries, `focus-fox` and `fox` (two
  `[[bin]]` entries pointing at `src/main.rs`; `default-run = "focus-fox"`
  keeps plain `cargo run` working).
- crossterm is used via the `ratatui::crossterm` re-export — don't add a
  separate crossterm dependency (version-mismatch risk).
- Keep timer logic in `src/timer/` free of TUI/IO concerns so it stays
  testable.
