# Demo videos

## Playing a quiz

A quiz played on the test page: the big screen on the left, two teams' phones on the right.

<video controls preload="metadata" poster="../media/demo-poster.webp" style="width:100%;border-radius:12px">
  <source src="../media/demo.mp4" type="video/mp4">
  <track kind="captions" src="../media/demo.vtt" srclang="en" label="English" default>
</video>

??? note "Transcript"

    --8<-- "media/demo-transcript.md"

## Building a quiz

A quiz made from scratch in the editor: questions, answer types, a picture, a slide, questions
from the library, and a test run.

<video controls preload="metadata" poster="../media/demo-editor-poster.webp" style="width:100%;border-radius:12px">
  <source src="../media/demo-editor.mp4" type="video/mp4">
  <track kind="captions" src="../media/demo-editor.vtt" srclang="en" label="English" default>
</video>

??? note "Transcript"

    --8<-- "media/demo-editor-transcript.md"

## The question library

Browsing and searching the library, editing a question, filling a new quiz at random, and saving
a question of your own to the library.

<video controls preload="metadata" poster="../media/demo-library-poster.webp" style="width:100%;border-radius:12px">
  <source src="../media/demo-library.mp4" type="video/mp4">
  <track kind="captions" src="../media/demo-library.vtt" srclang="en" label="English" default>
</video>

??? note "Transcript"

    --8<-- "media/demo-library-transcript.md"

## How they are made

The voice is generated with [Chatterbox](https://github.com/resemble-ai/chatterbox), an
open-weight text-to-speech model that can sound excited, and the videos are recorded by
`just demo-video`, which drives the app in a headless browser. Each line starts when its caption
appears on screen; recordings placed in `assets/voice/` replace the generated voice (see
`planning/ASSETS.md`).
