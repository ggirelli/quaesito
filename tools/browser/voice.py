"""Speaks the demo video's narration with Kokoro, as cheerfully as it can.

Writes out/voice/<id>.wav and out/voice/manifest.json (each line's duration in milliseconds).
A recording in assets/voice/<id>.{wav,mp3,m4a,ogg,flac} (see ASSETS.md) replaces Kokoro for that
line. Run with uv; the model files are downloaded into target/kokoro by `just demo-video`:

    uvx --python 3.12 --with kokoro-onnx --with soundfile python tools/browser/voice.py
"""

import json
import subprocess
from pathlib import Path

import soundfile as sf
from kokoro_onnx import Kokoro

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
MODELS = ROOT / "target" / "kokoro"
RECORDED = ROOT / "assets" / "voice"
OUT = HERE / "out" / "voice"

# Kokoro has no mood setting. Nova and Kore vary their pitch the most of its English voices
# (about twice as much as Bella and Heart, measured with librosa's pyin on a narration line), so an
# even blend, a little faster, sounds the least flat.
BLEND = {"af_nova": 0.5, "af_kore": 0.5}
SPEED = 1.1


def recorded(line_id: str) -> Path | None:
    """A recording of the line in assets/voice, if there is one."""
    for ext in ("wav", "mp3", "m4a", "ogg", "flac"):
        path = RECORDED / f"{line_id}.{ext}"
        if path.exists():
            return path
    return None


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    kokoro = Kokoro(str(MODELS / "kokoro-v1.0.onnx"), str(MODELS / "voices-v1.0.bin"))
    voice = sum(kokoro.get_voice_style(name) * weight for name, weight in BLEND.items())
    manifest = {}
    for line in json.loads((HERE / "narration.json").read_text()):
        out = OUT / f"{line['id']}.wav"
        if source := recorded(line["id"]):
            subprocess.run(
                ["ffmpeg", "-loglevel", "error", "-y", "-i", str(source), "-ac", "1", str(out)],
                check=True,
            )
            info = sf.info(out)
            manifest[line["id"]] = round(info.frames / info.samplerate * 1000)
            print(f"{line['id']}: {manifest[line['id']]} ms (recorded: {source.name})")
            continue
        samples, rate = kokoro.create(line["say"], voice=voice, speed=SPEED, lang="en-us")
        sf.write(out, samples, rate)
        manifest[line["id"]] = round(len(samples) / rate * 1000)
        print(f"{line['id']}: {manifest[line['id']]} ms")
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2))


if __name__ == "__main__":
    main()
