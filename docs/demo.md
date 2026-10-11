# Demo video

A quiz played on the test page: the big screen on the left, two teams' phones on the right.

<video controls preload="metadata" poster="../media/demo-poster.webp" style="width:100%;border-radius:12px">
  <source src="../media/demo.mp4" type="video/mp4">
  <track kind="captions" src="../media/demo.vtt" srclang="en" label="English" default>
</video>

The voice is generated with [Chatterbox](https://github.com/resemble-ai/chatterbox), an
open-weight text-to-speech model that can sound excited, and the video is recorded by `just demo-video`, which plays a sample quiz in
a headless browser. Each line starts when its caption appears on screen; recordings placed in
`assets/voice/` replace the generated voice (see `planning/ASSETS.md`).

## Transcript

--8<-- "media/demo-transcript.md"

Captions: [demo.vtt](media/demo.vtt) (WebVTT).
