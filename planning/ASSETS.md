# Assets

Assets that would improve Quaesito but that I cannot produce myself. Each entry has a prompt to
paste into ChatGPT (or another image generator), the file name and where to put it. None of them
is required: the app works without them.

Brand colours for every prompt: amber `#FFB703`, deep indigo `#14121F`, off-white `#F4F1FF`.
Answer colours and shapes, always paired: violet `#8B5CF6` sparkle, teal `#14B8A6` crescent moon,
magenta `#E0479E` heart, orange `#FB923C` lightning bolt, sky blue `#38BDF8` leaf, lime `#A3E635`
drop. Shapes are drawn in off-white on violet and magenta, in deep indigo on the other colours.

## Wanted

### Lobby illustration, new answer colours and shapes

The current one shows the old answer colours and shapes (pink triangle, blue diamond, green square,
pink circle) on the phones.

- **Files:** `assets/brand/lobby.png` (the full-size original, git-ignored), about 1448×1086,
  transparent background. I compress it to `assets/brand/lobby.webp` and copy it to
  `docs/assets/lobby.webp`.
- **Used by:** the big-screen lobby until five teams have joined, and the top of the documentation
  home page (in dark and light mode).
- **Prompt** (attach the current `lobby.png` so the scene stays the same):

  ```
  Redraw this flat vector illustration with the same scene, composition, characters and style:
  friends on a dark sofa seen from behind, holding phones up towards a TV that shows a big amber
  (#FFB703) question mark, a plant, a bowl of popcorn, amber spark lines. Change only the colours
  and symbols:
  - Each phone screen is filled with one answer colour and shows one white or dark symbol, centred:
    a violet (#8B5CF6) screen with an off-white four-pointed sparkle; a teal (#14B8A6) screen with
    a deep indigo crescent moon; a magenta (#E0479E) screen with an off-white heart; an orange
    (#FB923C) screen with a deep indigo lightning bolt.
  - Clothes, hair and cushions use the same accents (violet, teal, magenta, orange, sky blue
    #38BDF8, lime #A3E635) instead of the old pink, blue and green.
  Palette limited to amber #FFB703, deep indigo #14121F, off-white #F4F1FF and those accents.
  Transparent background, no text, no gradients, no photorealism. It must read well on a dark
  #14121F background and on a white one.
  ```

### GitHub social preview, new answer colours and shapes

The current one shows the old answer tiles (pink triangle, blue diamond, yellow circle, green
square) on the TV and the phones.

- **Files:** `assets/brand/social-preview.png` (original, git-ignored) and
  `assets/brand/social-preview.jpg`, 1280×640.
- **Used by:** GitHub repository settings → Social preview (uploaded by hand).
- **Prompt** (attach the current `social-preview.jpg`):

  ```
  Redraw this 1280×640 banner with the same layout and style: on the left the amber (#FFB703)
  logo, a rounded question mark whose top loops like a "q", with the word "quaesito" and the
  tagline "Quizzes among friends." in off-white #F4F1FF; on the right a TV and three phones held
  up by hands, on a deep indigo #14121F background. Change only the answers:
  - The TV shows a question bar and six answer tiles in two rows of three, each a colour with a
    centred symbol: violet #8B5CF6 with an off-white four-pointed sparkle, teal #14B8A6 with a deep
    indigo crescent moon, magenta #E0479E with an off-white heart, orange #FB923C with a deep
    indigo lightning bolt, sky blue #38BDF8 with a deep indigo leaf, lime #A3E635 with a deep
    indigo drop.
  - The three phones show the violet sparkle, the teal moon and the magenta heart.
  - Sleeves use the same accents instead of the old pink, blue and yellow.
  Flat vector, no gradients, no photorealism, no text besides "quaesito" and the tagline.
  ```

### Demo video voice-over (optional)

The demo video is narrated by [Chatterbox](https://github.com/resemble-ai/chatterbox), which runs
locally and has an emotion setting, with each take checked by Whisper and redone if it misses
words. It is still a model: a person, or a voice model directed line by line, would sound more
natural.

- **Files:** `assets/voice/<id>.mp3` (or `.wav`, `.m4a`, `.ogg`, `.flac`), one per line below.
  Any line without a file keeps the generated voice.
- **Used by:** `just demo-video`, which stretches each step of the video to fit its line, mixes the
  voice in, and writes the captions and transcript.
- **Tool:** a voice model with emotion or audio tags, such as ElevenLabs (Eleven v3, whose audio
  tags like `[excited]` and `[laughs]` steer the delivery), or record yourself with your phone.
- **Prompt** (paste the lines one at a time, so each becomes its own file):

  ```
  Voice: a warm, upbeat young presenter hosting a quiz night with friends. Gleeful and playful,
  smiling while speaking, lively pitch, energetic but not shouting, natural pace with small
  pauses at commas. American or British English. Each line is a separate clip with no silence
  before or after it. Say "quaesito" as one word, "kuh-eh-zee-toh" run together (kweh-ZEE-toh).
  ```

  | File | Line |
  |---|---|
  | `intro` | Welcome to quaesito! The questions go up on the TV, and every team answers right from their phone. |
  | `join` | Teams join by scanning the QR code, then pick a name and a picture. Easy peasy! |
  | `slide` | Slides kick off each round. Let's go! |
  | `choice` | First up, multiple choice! The faster you answer correctly, the more points you score. |
  | `truefalse` | True or false? Quick and snappy! |
  | `typed` | Typed answers are grouped on the big screen, so the host can judge them in a flash, even with half points! |
  | `number` | How many keys on a piano? The closest guess wins! |
  | `order` | Put them in order, oldest first! Every item in the right place scores. |
  | `parts` | Several answers in one question, and this one is worth double points! |
  | `pin` | Drop a pin on the picture. The closer you get, the more you win! |
  | `podium` | And the winner is… drumroll, please! |
  | `outro` | Best of all, quaesito is free and open source. Grab it, and host your own quiz night! |

  With Eleven v3, add a tag at the start of a line to steer it, for example
  `[excited] Welcome to quaesito! …` or `[playful] Easy peasy!`.

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
