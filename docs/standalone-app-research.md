# Research: Focus Fox as a standalone desktop app

Date: 2026-08-10.

The goal: open Focus Fox as a real app window on macOS and on Linux desktops
that don't make it easy to bind a hotkey to "spawn a terminal running this
command." Keep the ratatui UI. No terminal required.

## Decision: docs, not a windowed binary

We're not building the windowed app. Instead we document how to launch
Focus Fox from a hotkey or app icon with the user's own terminal:

- README section "Launch from a hotkey or app icon" with per-terminal
  launch commands and per-DE hotkey instructions.
- `contrib/focus-fox.desktop`, an example desktop entry with an explicit
  terminal `Exec` line (not `Terminal=true`, which GNOME/KDE/XFCE all
  handle differently and buggily, see the launcher findings below).
- `contrib/raycast/focus-fox.sh`, a Raycast script command for macOS.

Why: the windowed option's code was cheap but its orbit was not. A niche
single-maintainer backend crate, egui version lockstep, a second Linux
binary that gives up the zero-dependency static story for a hand-maintained
dep list, an unsigned macOS .app users have to "Open Anyway," and a
Homebrew cask that turns into a $99/yr Apple subscription after the
September 2026 Gatekeeper deadline. Permanent complexity to put a window
around a UI that already works. Docs cost nothing and rot slower.

The rest of this doc is the research that fed that call, kept because the
windowed path below is the right one if demand ever justifies revisiting.

## The windowed option (evaluated, not chosen)

The best version of it: render the existing ratatui UI in an egui window
with `egui_ratatui`, shipped as a third, feature-gated binary
(`focus-fox-app`) so the TUI binaries and the static musl release pipeline
stay exactly as they are.

The code side is small, roughly 2 to 4 days. The packaging side is the real
cost, and it is additive: new artifacts next to the old ones, not changes to
them. Linux packaging first. The macOS notarization decision is the only
part that costs money ($99/yr).

## Why the code side is cheap

The coupling audit came back clean. The terminal only touches three places:

- `src/tui/ui.rs` (631 lines) renders against a plain ratatui `Frame`. Zero
  crossterm imports. It works unmodified on any ratatui `Backend`.
- `src/tui/app.rs` funnels every key through
  `handle_key(KeyCode, KeyModifiers)`. The unit tests already call it with no
  terminal at all. The only terminal-bound code is the `event::poll` /
  `event::read` loop inside `App::run`.
- `src/tui/mod.rs` plus `src/theme.rs` do `ratatui::init()` and the
  terminal-colorsaurus / supports-color probes.

Audio (rodio), config, stats, and notify are process-level. They don't care
whether stdout is a TTY. Both binaries would share the same XDG config and
history files, which is a feature: adjust a setting in the app window, the
TUI sees it next run.

## The stack it would use

| Layer      | Crate                                                             | State (Aug 2026)                                                            |
|------------|-------------------------------------------------------------------|-----------------------------------------------------------------------------|
| Backend    | `egui_ratatui` 2.2.0 (Apr 2026)                                   | Implements ratatui `Backend`, already targets the 0.30 `ratatui-core` split |
| Rasterizer | `soft_ratatui` 0.2.2 (Apr 2026)                                   | CPU glyph rendering, ~14k downloads/mo, font backends incl. cosmic-text     |
| Input      | `terminput` 0.5.15 + `terminput-egui` 0.7.0 (Jul 2026)            | egui events → terminput → crossterm-shaped events                           |
| Window     | `eframe` (pin to the egui version egui_ratatui wants, 0.34 today) | winit + glow/wgpu                                                           |

How each concern maps:

- **Rendering.** `RataguiBackend` implements the `Backend` trait, so
  `Terminal::new(backend)` just works. `ui.rs` does not change.
- **Input.** crossterm's `event::read()` doesn't exist in a window. Read
  `egui::Context::input()` each frame, run it through `terminput-egui`, and
  convert to crossterm `KeyCode`/`KeyModifiers` via `terminput-crossterm`.
  `handle_key` does not change. Estimate 50 to 150 lines of glue.
- **Event loop.** Extract the body of `App::run` (tick timer, sync audio,
  record phases, draw) into a shared per-frame method. The TUI keeps its
  100ms `event::poll` loop. The eframe host calls the same method from
  `update()` and uses `ctx.request_repaint_after(Duration::from_millis(100))`
  to keep ticking while idle. This is the only real refactor in `app.rs`.
- **Theme.** Skip the colorsaurus probe in GUI mode. Map the `auto` setting
  to eframe's system theme (winit reports light/dark and theme changes).
  `true_color` is always true in a window. `src/theme.rs` grows one small
  GUI path.
- **Fonts.** The big clock is Unicode block digits. soft_ratatui rasterizes
  glyphs itself, so embed one monospace font with good block-element
  coverage (DejaVu Sans Mono or JetBrains Mono) rather than hoping the
  system has one.
- **Audio.** rodio is unchanged. Same process, same `Audio::sync` tick.
- **Notifications.** Keep the `notify-send` shell-out. It works fine from a
  GUI process on Linux and already silently no-ops on macOS. Native macOS
  notifications need a signed .app bundle no matter what UI stack we use, so
  that's a packaging question for later, not a code question now.
- **Crate layout.** New cargo feature `gui` gating `eframe`, `egui_ratatui`,
  and `terminput`, plus a third `[[bin]] focus-fox-app`. Plain
  `cargo build` and the existing `.#static` musl build never compile any of
  it. This is the whole trick for not disturbing the release pipeline.

Effort: 2 to 4 days to a working window, roughly 300 to 500 new lines.

## Packaging, the actual hard part

One fact drives everything: **a windowed binary cannot be fully static on
Linux.** musl solves libc, not windowing. winit needs libwayland-client and
libxkbcommon (dlopen'd, soft-fail) and/or X11 libs, and egui's glow/wgpu
renderers need the system's libGL or Vulkan driver, which must be dynamic
because it has to match the host GPU. Alacritty gave up on portable static
binaries for the same reason and ships distro packages. So the current
`.#static` story stays TUI-only, and that's fine.

### Linux plan

Ship the GUI binary dynamically linked (normal glibc nix build) inside the
existing deb/rpm/arch packages, with hand-declared runtime deps in nfpm
(nfpm does not auto-detect them): `libxkbcommon`, wayland, X11 libs,
`libgl1`/mesa. Add a `.desktop` file and an icon via nfpm `contents`, that's
what makes GNOME/KDE treat it as a real app you can pin and hotkey. The
static tarball keeps shipping only the TUI binaries.

AppImage is possible later (`nix bundle` + `nix-appimage`) but the output
is rough today: no desktop-file/icon integration, and GPU apps need nixGL
shims on non-NixOS hosts. `nix-bundle-app` (SergioRibera) is newer and
covers deb/rpm/AppImage and macOS .app from one tool, worth a prototype but
don't bet the pipeline on it yet. Skip Flatpak, there is no mature
nix-to-flatpak path.

Estimate: 1 to 2 days for deb/rpm/arch + .desktop + icon.

### macOS plan

A bare Mach-O binary can't appear in the Dock, Spotlight, or Launchpad. We
need a `.app` bundle: `Contents/MacOS/focus-fox-app`, `Info.plist`, icon.
Building one in nix is mechanical, and the existing `install_name_tool`
libiconv relink (with the `disallowedReferences` check) extends to it
unchanged. Ship it zipped in the GitHub release.

The catch is Gatekeeper. Since Sequoia, an unsigned app requires the user to
go to System Settings → Privacy & Security → "Open Anyway" once. Ad-hoc
signing does not help. And Homebrew forces the decision: `--no-quarantine`
is deprecated, and **on September 1, 2026 Homebrew removes every cask that
fails Gatekeeper**. So a cask for the GUI app effectively requires a $99/yr
Apple Developer ID plus notarization. `rcodesign` can sign and notarize from
Linux CI, no Mac needed, so the pipeline can do it, the only question is
paying Apple.

Plan: ship the unsigned .app zip on GitHub releases with an "Open Anyway"
note in the README. The existing Homebrew formula keeps shipping the TUI
binaries, which are unaffected. Add a cask only if we decide the $99/yr is
worth it.

Estimate: 1 to 2 days for the bundle + relink. Notarization is a separate
half-day once a Developer ID exists.

## Options considered and rejected

- **`ratatui-wgpu`** (0.5.0, Mar 2026). GPU rendering with real text shaping,
  better font quality. But you hand-write the whole winit event loop and all
  keyboard translation, no terminput adapter exists for winit. More glue for
  a timer that repaints 10 times a second. Fallback option if egui_ratatui
  dies.
- **Embed a terminal widget** (`egui_term` + `alacritty_terminal`, PTY
  running the real binary). Renders the UI twice and `egui_term` is stale:
  one release, Apr 2025, pinned two egui majors behind. Wrong layer.
- **Bundle a terminal emulator** (alacritty/wezterm `--command`). Tens of MB
  for someone else's app we'd have to configure to not look like a terminal.
  Alacritty's own .app launcher wrapper has a history of breakage.
- **Thin launcher** (.desktop `Terminal=true`, Platypus on macOS). This is
  the thing that already doesn't work well, and the research confirmed why:
  GNOME uses a hardcoded terminal fallback list in glib, KDE has open bugs
  honoring the configured terminal, XFCE has its own exo quirks, and
  xdg-terminal-exec is an unratified spec Alacritty already wontfix'd.
  What's rejected here is *shipping* a launcher that guesses the user's
  terminal. The chosen docs approach sidesteps every one of these bugs by
  having the user name their terminal explicitly in the Exec line.
- **Full GUI rewrite** (egui/iced native widgets, Tauri). Off the table by
  decision: we keep ratatui. For the record egui would be the closest
  paradigm match (~800-1200 LOC rewrite), iced is a one-maintainer project,
  Tauri drags in webkit2gtk which is a known nixpkgs pain, and Slint's
  royalty-free license requires attribution, bad fit for a small MIT app.

## Risks

- `egui_ratatui` is niche: ~22 downloads/month, single maintainer. Mitigation:
  the API surface we depend on is one `Backend` impl over `soft_ratatui`
  (which has real usage). Worst case we vendor it or move to `ratatui-wgpu`.
  `ui.rs` stays portable either way, that's the insurance.
- egui version lockstep. `egui_ratatui` pins an egui version, so eframe must
  match it, not latest. Annoying, not blocking.
- soft_ratatui font rendering quality is bitmap-first. Needs a visual check
  early, the block-digit clock is the whole aesthetic. If it looks bad with
  the embedded-graphics backend, switch its cosmic-text feature on.
- macOS: focus-fox has no signing history. First .app release should get a
  quick test on a clean Mac before announcing it.

## Suggested order (if we ever build it)

1. Spike (half a day): `gui` feature, eframe window, RataguiBackend, render
   the menu screen read-only. Proves fonts and versions before any refactor.
2. Input + frame refactor (1-2 days): terminput glue, shared per-frame
   method, theme path. Full parity with the TUI.
3. Linux packaging (1-2 days): .desktop, icon, nfpm deps, release.yml.
4. macOS .app (1-2 days): nix bundle derivation, relink, zip in release.
5. Decide on Apple Developer ID. Only needed for a Homebrew cask.

## Sources

- ratatui 0.30 / backends: https://ratatui.rs/highlights/v030/,
  https://ratatui.rs/concepts/backends/comparison/,
  https://github.com/ratatui/awesome-ratatui
- egui_ratatui / soft_ratatui: https://github.com/gold-silver-copper/egui_ratatui,
  https://lib.rs/crates/soft_ratatui
- terminput: https://lib.rs/crates/terminput, https://lib.rs/crates/terminput-egui
- ratatui-wgpu: https://github.com/Jesterhearts/ratatui-wgpu
- egui_term / alacritty_terminal: https://github.com/Harzu/egui_term,
  https://crates.io/crates/alacritty_terminal
- Static GUI linking reality: https://jangafx.com/insights/linux-binary-compatibility,
  https://github.com/emilk/egui/issues/2144,
  https://github.com/alacritty/alacritty/blob/master/INSTALL.md
- winit dlopen behavior: https://deepwiki.com/rust-windowing/winit/5.2-linux-(x11-and-wayland)-implementation
- nix bundling: https://github.com/ralismark/nix-appimage,
  https://github.com/SergioRibera/nix-bundle-app
- Gatekeeper in Sequoia+: https://support.apple.com/en-us/102445,
  https://eclecticlight.co/2024/08/10/gatekeeper-and-notarization-in-sequoia/
- Homebrew cask deadline: https://brew.sh/2025/11/12/homebrew-5.0.0/,
  https://github.com/Homebrew/brew/issues/20755
- rcodesign (notarize from Linux): https://gregoryszorc.com/docs/apple-codesign/stable/apple_codesign_rcodesign_notarizing.html
- .desktop Terminal=true fragmentation: https://github.com/GNOME/glib/blob/main/gio/gdesktopappinfo.c,
  https://gitlab.gnome.org/GNOME/glib/-/issues/1584,
  https://gitlab.xfce.org/xfce/exo/-/issues/99,
  https://github.com/Vladimir-csp/xdg-terminal-exec,
  https://github.com/alacritty/alacritty/issues/8089
