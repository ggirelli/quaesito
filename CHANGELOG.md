# Changelog

All notable changes to quaesito are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Starting a game asks how many teams may join: 50 by default, from 1 up to 10,000. The lobby shows the limit.
- A demo quiz with one question of each kind, added the first time quaesito starts.
- A help button on the home page and in the admin pages, opening the user guide.
- The downloads include the user guide, served by quaesito at `/docs/`, so it works offline.
- Bonus questions on choices, true/false, number, order and pin questions: a typed question answered together with the main
  one, worth +25%, +50% or +100% of its points and judged like typed answers on the reveal.
- Results slides, for quizzes that reveal the answers at the end: placed anywhere (at an intermission, say), they walk
  through the answers since the previous results slide, then show the standings with the points each team gained and
  the places it moved.
- The README, the user guide and the downloads say up front that quaesito is free, MIT-licensed, without
  warranty and fully vibe coded, and that donations are welcome on Ko-fi.
- "Test from here" on every question and slide in the editor starts a test right there, skipping the lobby.

### Changed

- Reveals say "Nobody answered this one." when no team answered, also in the walk through the answers at the end.
- Stopping the clock by hand stops the soundtrack too; when time runs out, the music still plays on through the reveal.
- Answers have new colours (violet, teal, magenta, orange, sky, lime) and shapes (sparkle, moon, heart, bolt, leaf,
  drop), drawn so they look the same on every phone. The right answer is ringed in white.
- The README is shorter and points to the user guide for details.
- The demo video and the screenshots show a question with a picture and one with a soundtrack.
- The demo video is narrated with Chatterbox, which sounds livelier than Kokoro.

### Fixed

- A question's picture no longer pushes the big screen's controls off the bottom of a 16:9 screen: the
  big screen is exactly one screen tall and the picture takes the room the question leaves.
- Nothing is loaded from the internet any more: the user guide bundles its fonts and no longer asks GitHub for the
  repository's stars, and the server tells browsers to load nothing from other sites.
- Moving a question or slide in the editor keeps it on screen and keeps the focus on it.

## [0.1.0] - 2026-10-11

The first release: everything needed for a quiz night with friends.

### Added

- **Quiz editor** in the browser with autosave, a "ready to play" check, import and export (a zip
  with pictures and soundtracks), duplicate, shuffle (round by round between slides) and moving
  items to the top or bottom.
- **Question types:** multiple choice (with picture answers), true/false, typed answers judged by
  the host (full, half or no points, bonus and malus), closest number, put in order, several typed
  answers in one question, and a pin on a picture scored by distance. Slides between questions.
- **Pictures and sound:** question pictures (optionally revealed by zooming out or tile by tile),
  soundtracks started by the host together with the clock, and synthesised sound effects.
- **Big screen:** join QR code and lobby, live answer counts, reveals with the fastest team,
  points, answer times and rank changes, a podium revealed from third to first, and printable or
  CSV results.
- **Phones:** join by QR code or game code, team name and picture, and a pad for every question
  type; reconnects by itself.
- **Hosting:** pause, extra time, going back, renaming and removing teams, automatic or manual
  advance, and games that survive a server restart.
- **Settings per quiz:** reveal after each question or at the end, all-or-nothing or partial
  credit, speed or flat points, jokers, per-question points multiplier, and what phones show.
- **Test mode:** the big screen and phones side by side to try a quiz on your own.
- **Videos** in place of a question's or slide's picture, and configurable upload limits.
- **Downloads** for Linux, macOS (Apple silicon) and Windows with a start script, a documentation
  site with screenshots and a narrated demo video.

[Unreleased]: https://github.com/ggirelli/quaesito/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ggirelli/quaesito/releases/tag/v0.1.0
