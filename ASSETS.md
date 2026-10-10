# Assets

Assets that would improve Quaesito but that I cannot produce myself. Each entry has a prompt to
paste into ChatGPT (or another image generator), the file name and where to put it. None of them
is required: the app works without them.

Brand colours for every prompt: amber `#FFB703`, deep indigo `#14121F`, off-white `#F4F1FF`.

## Wanted

### Recorded sound effects (optional)

The big screen synthesises its sound effects in the browser (`assets/js/sfx.js`), so nothing is
missing. Recorded effects would sound richer; if you add them I will play them instead.

- **Files:** `assets/sfx/{tick,last-tick,time-up,reveal,join,drumroll,fanfare}.mp3`, mono, 44.1 kHz,
  normalised to about −14 LUFS, no silence before the sound
- **Used by:** the big screen (`src/ui/sfx.rs`)
- **Prompts** (for a sound-effect generator such as ElevenLabs Sound Effects, one per file):

  ```
  tick:      a single short, crisp clock tick, dry, 0.1 seconds
  last-tick: a single short, higher-pitched game-show clock beep, 0.15 seconds
  time-up:   a friendly game-show "time's up" buzzer, low, 0.7 seconds, not harsh
  reveal:    a bright, cheerful 4-note ascending chime for revealing a quiz answer, 0.6 seconds
  join:      a soft, playful "pop" for a player joining a party game, 0.2 seconds
  drumroll:  a snare drumroll building up for 1.3 seconds, ending with a cymbal hit
  fanfare:   a short triumphant brass fanfare for announcing the winner, 1.5 seconds
  ```

## Done

- `assets/brand/icon.png`: the app icon (1254×1254 source). The app uses `icon-192.png`, a cropped
  192×192 copy, as favicon and logo mark.
- `assets/brand/logo-{dark,light}.{png,svg}`: logo with word mark (README header).
- `assets/brand/lobby.webp`: lobby illustration on the big screen, shown until five teams have joined.
  Compressed from the 1448×1086 PNG original (kept locally as `lobby.png`, git-ignored).
- `assets/brand/social-preview.jpg`: GitHub social preview, 1280×640. Not used by the app: upload it
  by hand under the repository's Settings → General → Social preview (GitHub has no API for it).
