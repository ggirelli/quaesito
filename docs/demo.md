# Demo video

A quiz played on the test page: the big screen on the left, two teams' phones on the right.

<video controls preload="metadata" poster="../media/demo-poster.webp" style="width:100%;border-radius:12px">
  <source src="../media/demo.mp4" type="video/mp4">
  <track kind="captions" src="../media/demo.vtt" srclang="en" label="English" default>
</video>

The voice is generated with [Kokoro](https://huggingface.co/hexgrad/Kokoro-82M), an open-weight
text-to-speech model, and the video is recorded by `just demo-video`, which plays a sample quiz in
a headless browser. Each line starts when its caption appears on screen; recordings placed in
`assets/voice/` replace Kokoro's voice (see `ASSETS.md`).

## Transcript

--8<-- "media/demo-transcript.md"

Captions: [demo.vtt](media/demo.vtt) (WebVTT).
