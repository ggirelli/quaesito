"""Speaks the demo video's narration with Kokoro, in a gleeful voice.

Writes out/voice/<id>.wav and out/voice/manifest.json (each line's duration in milliseconds).
Run with uv; the model files are downloaded into target/kokoro by `just demo-video`:

    uvx --python 3.12 --with kokoro-onnx --with soundfile python tools/browser/voice.py
"""

import json
from pathlib import Path

import soundfile as sf
from kokoro_onnx import Kokoro

HERE = Path(__file__).resolve().parent
MODELS = HERE.parent.parent / "target" / "kokoro"
OUT = HERE / "out" / "voice"

# Kokoro has no mood setting: a blend of its liveliest American voice (Bella) with its clearest
# (Heart), spoken a little faster, sounds bright and cheerful.
BLEND = {"af_bella": 0.6, "af_heart": 0.4}
SPEED = 1.1


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    kokoro = Kokoro(str(MODELS / "kokoro-v1.0.onnx"), str(MODELS / "voices-v1.0.bin"))
    voice = sum(kokoro.get_voice_style(name) * weight for name, weight in BLEND.items())
    manifest = {}
    for line in json.loads((HERE / "narration.json").read_text()):
        samples, rate = kokoro.create(line["say"], voice=voice, speed=SPEED, lang="en-us")
        sf.write(OUT / f"{line['id']}.wav", samples, rate)
        manifest[line["id"]] = round(len(samples) / rate * 1000)
        print(f"{line['id']}: {manifest[line['id']]} ms")
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2))


if __name__ == "__main__":
    main()
