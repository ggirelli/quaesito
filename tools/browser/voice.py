"""Speaks the demo video's narration, as cheerfully as it can.

Writes out/voice/<id>.wav and out/voice/manifest.json (each line's duration in milliseconds).
A recording in assets/voice/<id>.{wav,mp3,m4a,ogg,flac} (see planning/ASSETS.md) replaces the
generated voice for that line. Run with uv, through `just demo-video`, which picks the engine:

- chatterbox (default): Resemble AI's Chatterbox, which can act excited. Fast on an NVIDIA GPU,
  slow on a CPU. Its model is downloaded from Hugging Face on first use.
- kokoro: Kokoro, small and quick on any computer but flatter. Its model files are downloaded into
  target/kokoro.
"""

import json
import re
import subprocess
import sys
from pathlib import Path

import numpy as np
import soundfile as sf

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
RECORDED = ROOT / "assets" / "voice"
OUT = HERE / "out" / "voice"

# Chatterbox's emotion settings: exaggeration above its 0.5 default makes it livelier, and a lower
# cfg_weight keeps the pace natural (its README's advice for dramatic speech). It sometimes
# swaps or slurs a word, so each take is checked by ear (Whisper): a take missing words is redone
# with the next seed, keeping the best of TAKES. Fixed seeds give the same takes on every run. A
# line in narration.json can set its own "exaggeration" when the default overdoes it.
EXAGGERATION = 0.8
CFG_WEIGHT = 0.3
SEED = 7
TAKES = 4

# Kokoro has no mood setting. Nova and Kore vary their pitch the most of its English voices
# (about twice as much as Bella and Heart, measured with librosa's pyin on a narration line), so an
# even blend, a little faster, sounds the least flat.
BLEND = {"af_nova": 0.5, "af_kore": 0.5}
SPEED = 1.1

# Quieter than this (relative to the loudest sample) counts as silence when trimming each line.
SILENCE = 0.02


def chatterbox():
    """Speaks a line with Chatterbox."""
    import torch
    import torchaudio
    from chatterbox.tts import ChatterboxTTS
    from faster_whisper import WhisperModel

    device = "cuda" if torch.cuda.is_available() else "cpu"
    model = ChatterboxTTS.from_pretrained(device=device)
    ear = WhisperModel("small.en", device="cpu", compute_type="int8")

    def speak(line: dict) -> tuple[np.ndarray, int]:
        words = spoken(line["text"])
        exaggeration = line.get("exaggeration", EXAGGERATION)
        best = None
        for take in range(TAKES):
            torch.manual_seed(SEED + take)
            wav = model.generate(line["say"], exaggeration=exaggeration, cfg_weight=CFG_WEIGHT)
            samples = wav.squeeze(0).cpu().numpy()
            for_ear = torchaudio.functional.resample(wav, model.sr, 16000).squeeze(0).cpu()
            heard, _ = ear.transcribe(for_ear.numpy(), beam_size=5)
            heard = set(spoken(" ".join(part.text for part in heard)))
            missing = [word for word in words if word not in heard]
            if best is None or len(missing) < best[0]:
                best = (len(missing), samples)
            if not missing:
                break
            print(f"  take {take + 1} missed {missing}")
        return best[1], model.sr

    return speak


def kokoro():
    """Speaks a line with Kokoro."""
    from kokoro_onnx import Kokoro

    models = ROOT / "target" / "kokoro"
    model = Kokoro(str(models / "kokoro-v1.0.onnx"), str(models / "voices-v1.0.bin"))
    voice = sum(model.get_voice_style(name) * weight for name, weight in BLEND.items())

    def speak(line: dict) -> tuple[np.ndarray, int]:
        return model.create(line["say"], voice=voice, speed=SPEED, lang="en-us")

    return speak


def spoken(text: str) -> list[str]:
    """The words of a line to listen for, leaving out the app's name (spelled out for the voice)."""
    return [word for word in re.findall(r"[a-z']+", text.lower()) if word != "quaesito"]


def trimmed(samples: np.ndarray, rate: int) -> np.ndarray:
    """The line without silence before and after it (keeping 50 ms), so it starts on its cue."""
    loud = np.flatnonzero(np.abs(samples) > SILENCE * np.abs(samples).max())
    if loud.size == 0:
        return samples
    pad = rate // 20
    return samples[max(loud[0] - pad, 0) : loud[-1] + pad]


def recorded(line_id: str) -> Path | None:
    """A recording of the line in assets/voice, if there is one."""
    for ext in ("wav", "mp3", "m4a", "ogg", "flac"):
        path = RECORDED / f"{line_id}.{ext}"
        if path.exists():
            return path
    return None


def main() -> None:
    engine = sys.argv[1] if len(sys.argv) > 1 else "chatterbox"
    engines = {"chatterbox": chatterbox, "kokoro": kokoro}
    if engine not in engines:
        sys.exit(f"unknown voice {engine!r}: use {' or '.join(engines)}")
    OUT.mkdir(parents=True, exist_ok=True)
    lines = json.loads((HERE / "narration.json").read_text())
    speak = None
    manifest = {}
    for line in lines:
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
        speak = speak or engines[engine]()
        samples, rate = speak(line)
        samples = trimmed(samples, rate)
        sf.write(out, samples, rate)
        manifest[line["id"]] = round(len(samples) / rate * 1000)
        print(f"{line['id']}: {manifest[line['id']]} ms ({engine})")
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2))


if __name__ == "__main__":
    main()
