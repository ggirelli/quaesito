# Assets

Assets that would improve Quaesito but that I cannot produce myself. Each entry has a prompt to
paste into ChatGPT (or another image generator), the file name and where to put it. None of them
is required: the app works without them.

Brand colours for every prompt: amber `#FFB703`, deep indigo `#14121F`, off-white `#F4F1FF`.
Answer colours and shapes, always paired: violet `#8B5CF6` sparkle, teal `#14B8A6` crescent moon,
magenta `#E0479E` heart, orange `#FB923C` lightning bolt, sky blue `#38BDF8` leaf, lime `#A3E635`
drop. Shapes are drawn in off-white on violet and magenta, in deep indigo on the other colours.

## Wanted

### Demo video voice-over (optional)

The demo videos are narrated by [Chatterbox](https://github.com/resemble-ai/chatterbox), which runs
locally and has an emotion setting, with each take checked by Whisper and redone if it misses
words. It is still a model: a person, or a voice model directed line by line, would sound more
natural.

- **Files:** `assets/voice/<id>.mp3` (or `.wav`, `.m4a`, `.ogg`, `.flac`), one per line below
  (the lines are in `tools/browser/narration/`).
  Any line without a file keeps the generated voice.
- **Used by:** `just demo-video`, which stretches each step of the videos to fit its line, mixes the
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
  | `picture` | Any question can show a picture, as a clue or just for fun! |
  | `music` | Or add a soundtrack: the clock starts with the music. Which way does this tune go? |
  | `podium` | And the winner is… drumroll, please! |
  | `outro` | Best of all, quaesito is free and open source. Grab it, and host your own quiz night! |
  | `editor-intro` | Making a quiz is half the fun! Press New quiz, give it a name, and off you go. |
  | `editor-question` | Type a question and its answers, then tick the right one. Everything saves as you type! |
  | `editor-types` | Pick an answer type: true or false, typed answers, numbers, order, pins and pairs. |
  | `editor-picture` | Add a picture, and reveal it bit by bit while the clock runs. |
  | `editor-slide` | Slides open each round, with a title and a picture. |
  | `editor-library` | Short on ideas? Grab questions from the library, one by one or at random! |
  | `editor-navigate` | The strip on the side jumps anywhere in a long quiz, and the table view shows it all at once. |
  | `editor-test` | Then test it: the big screen and two phones, side by side. Ready for quiz night! |
  | `library-intro` | Quaesito comes with about 2,700 ready-made questions, in 14 categories. |
  | `library-browse` | Pick a category, then a topic, to see just those questions. |
  | `library-search` | Search them, and filter by difficulty, question type, or where they come from. |
  | `library-edit` | Found a mistake? Fix it right here, or report it with one click. |
  | `library-fill` | Starting a quiz? Fill it with random questions from the categories you like. |
  | `library-save` | Your own questions can join the library too, and stay in step with your quizzes. |
  | `library-outro` | Less time writing, more time playing. Happy quizzing! |

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

- `assets/teams/<name>.png`: 16 ready-made team pictures (owl, fox, cat, dog, bear, penguin, frog,
  octopus, robot, alien, ghost, dragon, unicorn, sloth, llama, panda), 256×256 cut-outs made by
  `uv run tools/assets/team-mascots.py` from the 1254×1254 originals (kept locally in
  `assets/teams/originals/`, git-ignored). The script removes the checkerboard some generators draw
  in place of transparency, and centres each mascot with room for its round frame.
- `assets/brand/icon.png`: the app icon (1254×1254 source). The app uses `icon-192.png`, a cropped
  192×192 copy, as favicon and logo mark.
- `assets/brand/logo-{dark,light}.{png,svg}`: logo with word mark (README header).
- `assets/brand/lobby.webp`: lobby illustration on the big screen, shown until five teams have joined,
  and at the top of the documentation home page (`docs/assets/lobby.webp`), with the answer colours
  and shapes on the phones. Compressed to 960×720 from the 1447×1087 PNG original (kept locally as
  `lobby.png`, git-ignored).
- `assets/brand/social-preview.jpg`: GitHub social preview, 1280×640, flattened onto deep indigo from
  the transparent 1774×887 PNG original (kept locally as `social-preview.png`, git-ignored). Not used by the app: upload it
  by hand under the repository's Settings → General → Social preview (GitHub has no API for it).
