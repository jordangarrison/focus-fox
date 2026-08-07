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
  On the timer screen, `←`/`→` (or `h`/`l`) scrub the clock ±1 minute
  (`Timer::seek_back`/`seek_forward`); forward past the end finishes the
  phase naturally on the next tick, and `a` toggles binaural audio,
  persisting the setting to the config file like a menu adjustment.
  `app.rs` owns the event loop (100ms tick, keyboard handling), `ui.rs`
  renders the menu, the alert banner, and the big block-digit clock,
  progress gauge, and session dots. The launch menu includes an
  `auto`/`dark`/`light` theme preference; `auto` resolves the terminal
  background once before Ratatui starts. Audio settings — opt-in binaural
  playback, named listening-mode presets, one persistent Custom tone,
  linear volume, and preview — live in a full-frame overlay
  (`App.audio_view`, like `stats_view`) opened with `a`/Enter from the
  menu or `A` from the timer, which keeps ticking underneath; the overlay
  swallows all keys (they collide with timer bindings) and alerts evict
  it like they evict the stats overlay.
- **`src/audio.rs`** - Best-effort stereo tone synthesis through rodio. A
  configurable left carrier and right carrier-plus-difference tone play only
  during preview or unpaused Work phases without an alert. Tone gain uses an
  independent player for future music-volume support. Playback and audio-device
  resources stop for breaks, menu visits, preview exit, and app exit. Linux uses
  ALSA and macOS uses CoreAudio.
- **`src/theme.rs`** - Persisted theme preference, terminal background
  and true-color capability detection, plus semantic light/dark color palettes
  with ANSI fallbacks used by every TUI screen.
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
  humantime durations), including opt-in binaural playback, preset selection,
  persistent Custom frequencies, and beat volume. Legacy difference-only audio
  configs migrate during deserialization. CLI args override file values via
  `merge_args`.
- **`src/cli/`** - Clap argument parsing.

## Documentation

Whenever a change is user-visible — new keybindings, new features, new
CLI flags or subcommands, changed behavior — update the docs in the same
commit:

- **README.md** — the Keys tables, Usage examples, and Configuration
  section are the user-facing reference; keep them in sync.
- **The in-app help line** in `src/tui/ui.rs` (`render_help` calls).
- **This file** — the Module Structure descriptions above.

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
5. Once every asset upload succeeds, `release.yml` dispatches the shared
   `jordangarrison/homebrew-tap` `update-formula.yml` workflow, waits for its
   Homebrew test matrix, and propagates failure. The GitHub release stays
   published if the tap update fails and the formula can be retried manually.

Notes:

- All assets come from the flake — never add rustup/cargo steps to CI;
  reproducibility from `flake.lock` is the point.
- Linux assets are built from a static musl binary (`.#static`); the
  deb/rpm/arch packages are generated from it with nfpm. macOS is a
  tarball of the native aarch64-darwin build with its libiconv load
  command rewritten to `/usr/lib/libiconv.2.dylib` — the raw Nix build
  links libiconv from the Nix store, which crashes at dyld load time on
  Macs without Nix. The relink derivation fails the build if any
  `/nix/store` reference survives (`otool -L` check +
  `disallowedReferences`).
- The flake reads `version` from `Cargo.toml`, so release-please bumps
  flow through automatically. `.release-please-manifest.json` tracks the
  released version; tags are plain `vX.Y.Z`
  (`include-component-in-tag: false` — don't remove it, component tags
  like `focus-fox-v0.1.0` break the `v*` trigger and tag history).
- No x86_64-darwin / universal mac build: nixpkgs 26.11 dropped the
  platform, and the flake enumerates supported systems explicitly
  (don't switch back to `eachDefaultSystem`).
- Manual escape hatch: pushing a `v*` tag by hand also triggers
  `release.yml`, which creates the GitHub release if missing and updates the
  Homebrew tap after all assets upload.
- The repository secret `HOMEBREW_TAP_TOKEN` must be a fine-grained token
  scoped only to `jordangarrison/homebrew-tap` with repository Actions write
  permission. It dispatches and watches the tap workflow without content
  access; GitHub documents that permission for the
  [workflow dispatch endpoint](https://docs.github.com/en/rest/actions/workflows#create-a-workflow-dispatch-event).
- Future tools share the same tap updater. Add a formula and manifest entry to
  the tap, then reuse the small dispatch-and-wait job from `release.yml`.

### Notes

- The crate ships two identical binaries, `focus-fox` and `fox` (two
  `[[bin]]` entries pointing at `src/main.rs`; `default-run = "focus-fox"`
  keeps plain `cargo run` working).
- crossterm is used via the `ratatui::crossterm` re-export — don't add a
  separate crossterm dependency (version-mismatch risk).
- Keep timer logic in `src/timer/` free of TUI/IO concerns so it stays
  testable.
