# Building a quiz

On the admin page, type a title and press **Create quiz**. The editor saves as you type and says
when the quiz is ready to play, or what is still missing. The quiz list flags quizzes that are not
ready yet.

![The quiz editor with a multiple-choice question](screenshots/editor-choices.webp)

## Questions and slides

- **Add** questions with **＋ Add question**, and slides with **＋ Add slide**: a title, some text
  and a picture to introduce a round or explain the rules.
- **Move** an item with ↑ ↓, or straight to the top or bottom with ⤒ ⤓. Duplicate it or delete
  it with the icons next to them.
- **Shuffle** with :material-shuffle: **Shuffle questions**: questions are shuffled within each
  round, slides stay where they are, and you can undo it until you change something else.

Each question has an **answer type** (see [question types](question-types.md)), a time limit
(the quiz's default unless you set one) and a **points** multiplier: a ×2 question earns twice
the usual points.

## Pictures

Add a picture to any question. For a guessing game, choose how it appears while the clock runs:

- **Show it all**, as it is;
- **Zoom out from a close-up**: it starts five times larger and widens to the whole picture;
- **Uncover tile by tile**: tiles drop away at an even pace.

With a gradual reveal, set how many seconds the whole picture stays visible before the buzzer.
Pictures are resized and re-encoded when uploaded, up to 15 MB.

![A multiple-choice question with a picture on the big screen](screenshots/host-picture.webp)

## Videos

Instead of a picture, a question or a slide can show a video (MP4 with H.264, WebM or MOV, up to
200 MB). It plays on the big screen when the question opens, pauses when you pause the game, and
plays again from the start when you click it. Pin questions need a picture.

The upload limits can be changed; see [configuration](configuration.md).

## Soundtracks

A question can have a soundtrack (MP3, M4A/AAC, Ogg/Opus, WebM, FLAC or WAV, up to 40 MB). The
question then waits on the big screen with a full clock until you press **▶ Start the music**:
the music plays from the big screen and the countdown starts with it. **Stop the clock** stops the
music too; when time runs out instead, it plays on while the answers are shown.

<div class="screen-and-phone" markdown>
<figure markdown>
![A soundtrack question waiting for the host to start the music](screenshots/host-music.webp)
<figcaption>The question waits, with a full clock, for the music.</figcaption>
</figure>
<figure markdown>
![A phone saying "Listen up!" until the music starts](screenshots/phone-music.webp)
<figcaption>Phones listen until the music starts.</figcaption>
</figure>
</div>

## Import and export

The export icon on the admin page downloads a quiz as a `.zip` with its pictures and
soundtracks. **Import** on another computer brings it back as a new quiz; older `.json` exports
import too.
