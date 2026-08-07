# Changelog

## [1.0.0](https://github.com/jordangarrison/focus-fox/compare/v0.6.0...v1.0.0) (2026-08-07)


### ⚠ BREAKING CHANGES

* volume percentages now live on a calibrated loudness scale shared by beats and music (100% is roughly -16 dBFS RMS). A stored binaural_volume_percent keeps its number but plays quieter than in 0.6; raise an old 8 to about 36 for the previous loudness.

### Features

* layer generative lofi music under the binaural beats ([#13](https://github.com/jordangarrison/focus-fox/issues/13)) ([a34f789](https://github.com/jordangarrison/focus-fox/commit/a34f789a64bede4cc193835823fa844f125a7d53))

## [0.6.0](https://github.com/jordangarrison/focus-fox/compare/v0.5.0...v0.6.0) (2026-08-06)


### Features

* open audio settings from the timer screen ([#11](https://github.com/jordangarrison/focus-fox/issues/11)) ([4aa80bb](https://github.com/jordangarrison/focus-fox/commit/4aa80bb2cdd2525c691bb8144f6fc7867bb426a4))

## [0.5.0](https://github.com/jordangarrison/focus-fox/compare/v0.4.0...v0.5.0) (2026-08-04)


### Features

* toggle binaural audio from the timer screen ([#8](https://github.com/jordangarrison/focus-fox/issues/8)) ([1fdf6cb](https://github.com/jordangarrison/focus-fox/commit/1fdf6cbc8b33b3db4c377e4f0cef7e35b8b0aea9))

## [0.4.0](https://github.com/jordangarrison/focus-fox/compare/v0.3.0...v0.4.0) (2026-08-04)


### Features

* add binaural tone presets ([#5](https://github.com/jordangarrison/focus-fox/issues/5)) ([0a621ca](https://github.com/jordangarrison/focus-fox/commit/0a621cac6d63a3c8d4db0fe4657b442bc97350a7))

## [0.3.0](https://github.com/jordangarrison/focus-fox/compare/v0.2.0...v0.3.0) (2026-07-29)


### Features

* add adaptive terminal themes ([#4](https://github.com/jordangarrison/focus-fox/issues/4)) ([bffa2d8](https://github.com/jordangarrison/focus-fox/commit/bffa2d8f691ac411489c57ce647f8a200ad14d9e))
* scrub the active timer with h/l or arrow keys ([8e65b5b](https://github.com/jordangarrison/focus-fox/commit/8e65b5bd8a7a3bd3d8f6c4a590723a437154852a))

## [0.2.0](https://github.com/jordangarrison/focus-fox/compare/v0.1.0...v0.2.0) (2026-07-24)


### Features

* add session record type for statistics ([346ac4a](https://github.com/jordangarrison/focus-fox/commit/346ac4ab71db506f61eaf8b35deb2e657b2ba1da))
* add stats overlay to the tui ([87edbca](https://github.com/jordangarrison/focus-fox/commit/87edbca6019bacb5c2667e5c43e221f735d6e000))
* add stats subcommand ([893dda8](https://github.com/jordangarrison/focus-fox/commit/893dda86a457501adb41949c86b4c21f36377eac))
* add year-partitioned jsonl history store ([8f81c4f](https://github.com/jordangarrison/focus-fox/commit/8f81c4fcce247ff3a7c6bb3733d82536ba3dc890))
* compute pomodoro statistics summary ([55143ac](https://github.com/jordangarrison/focus-fox/commit/55143aca28ba6098e5d3fdad34187d7ec5d0203a))
* record finished and skipped phases to history ([d5d0a83](https://github.com/jordangarrison/focus-fox/commit/d5d0a83aebf9cb9d0a1ca255eacf68322c5b57f0))
* render stats summary as plain text ([b0fbfbb](https://github.com/jordangarrison/focus-fox/commit/b0fbfbbe3e3fab567931b22a7c8a97d79f895cc8))


### Bug Fixes

* align stat columns in the stats overlay ([55bf1f8](https://github.com/jordangarrison/focus-fox/commit/55bf1f8802f98eb5acffe7322a0c462bcf1e95a4))
* close the stats overlay when a phase-end alert fires ([f3f3f9c](https://github.com/jordangarrison/focus-fox/commit/f3f3f9c13188b54bd76bab762f0c2f09c2bb3e31))
