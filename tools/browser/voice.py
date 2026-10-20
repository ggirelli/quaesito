"""Speaks the demo videos' narrations (narration/<video>.json), as cheerfully as it can.

Each sentence is spoken on its own and the sentences are joined with a pause, so the voice takes
its time instead of rushing a whole line in one breath. Writes out/voice/<video>/<id>.wav and
out/voice/<video>/manifest.json (each line's duration in milliseconds), for every video or those named after the engine. A recording in
assets/voice/<id>.{wav,mp3,m4a,ogg,flac} (see planning/ASSETS.md) replaces the generated voice for
that line. Run with uv, through `just demo-video`, which picks the engine:

- chatterbox (default): Resemble AI's Chatterbox, which can act excited. Fast on an NVIDIA GPU,
  slow on a CPU. Its model is downloaded from Hugging Face on first use.
- kokoro: Kokoro, small and quick on any computer but flatter. Its model files are downloaded into
  target/kokoro.
"""

import itertools
import json
import re
import subprocess
import sys
from pathlib import Path

import numpy as np
import soundfile as sf

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
NARRATION = HERE / "narration"
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

# The app's name, spelled for the voice: kweh-ZEE-toh. Chatterbox still gets it wrong in about half
# its takes (an extra syllable, or no k when it opens a sentence), so a sentence with the name is
# also checked phoneme by phoneme (wav2vec2's espeak phonemes, close sounds counted as the same),
# with up to NAME_TAKES takes. Mid-sentence it always glides "kweh" into "kway", so that counts too.
NAME = "Kweh-zee-toe"
NAME_PHONEMES = ["k", "w", "ɛ", "z", "iː", "t", "oʊ"]
SAME_PHONEME = {"e": "ɛ", "eɪ": "ɛ", "i": "iː", "ɪ": "iː", "o": "oʊ", "ou": "oʊ", "oː": "oʊ", "əʊ": "oʊ", "s": "z"}
PHONEME_MODEL = "facebook/wav2vec2-lv-60-espeak-cv-ft"
NAME_TAKES = 12

# Kokoro has no mood setting. Nova and Kore vary their pitch the most of its English voices
# (about twice as much as Bella and Heart, measured with librosa's pyin on a narration line), so an
# even blend, a little faster, sounds the least flat.
BLEND = {"af_nova": 0.5, "af_kore": 0.5}
SPEED = 1.1

# The pause between the sentences of a line, in seconds.
PAUSE = 0.45

# Quieter than this (relative to the loudest sample) counts as silence when trimming each line.
SILENCE = 0.02


def chatterbox():
    """Speaks a line with Chatterbox."""
    import torch
    import torchaudio
    from chatterbox.tts import ChatterboxTTS
    from faster_whisper import WhisperModel
    from huggingface_hub import hf_hub_download
    from transformers import Wav2Vec2FeatureExtractor, Wav2Vec2ForCTC

    device = "cuda" if torch.cuda.is_available() else "cpu"
    model = ChatterboxTTS.from_pretrained(device=device)
    ear = WhisperModel("small.en", device="cpu", compute_type="int8")
    # Its tokenizer needs espeak to load, but reading the phonemes back only needs the vocabulary.
    features = Wav2Vec2FeatureExtractor.from_pretrained(PHONEME_MODEL)
    phonemizer = Wav2Vec2ForCTC.from_pretrained(PHONEME_MODEL).eval().to(device)
    vocab = json.loads(Path(hf_hub_download(PHONEME_MODEL, "vocab.json")).read_text())
    symbols = {index: symbol for symbol, index in vocab.items()}

    def phonemes(audio: np.ndarray) -> list[str]:
        inputs = features(audio, sampling_rate=16000, return_tensors="pt").input_values.to(device)
        with torch.no_grad():
            best = phonemizer(inputs).logits.argmax(-1)[0].tolist()
        return [symbols[i] for i, _ in itertools.groupby(best) if not symbols[i].startswith("<")]

    def speak(line: dict, sentence: str) -> tuple[np.ndarray, int]:
        # The sentence's words that the caption has too (not the name, spelled out for the voice).
        known = set(spoken(line["text"]))
        words = [word for word in spoken(sentence) if word in known]
        exaggeration = line.get("exaggeration", EXAGGERATION)
        named = NAME in sentence
        best = None
        for take in range(NAME_TAKES if named else TAKES):
            torch.manual_seed(SEED + take)
            wav = model.generate(sentence, exaggeration=exaggeration, cfg_weight=CFG_WEIGHT)
            samples = wav.squeeze(0).cpu().numpy()
            for_ear = torchaudio.functional.resample(wav, model.sr, 16000).squeeze(0).cpu().numpy()
            heard, _ = ear.transcribe(for_ear, beam_size=5)
            heard = set(spoken(" ".join(part.text for part in heard)))
            missing = [word for word in words if word not in heard]
            off = name_off(phonemes(for_ear)) if named else 0
            if best is None or (len(missing), off) < best[0]:
                best = ((len(missing), off), samples)
            if not missing and not off:
                break
            print(f"  take {take + 1} missed {missing}" + (f", name {off} phonemes off" if off else ""))
        return best[1], model.sr

    return speak


def kokoro():
    """Speaks a line with Kokoro."""
    from kokoro_onnx import Kokoro

    models = ROOT / "target" / "kokoro"
    model = Kokoro(str(models / "kokoro-v1.0.onnx"), str(models / "voices-v1.0.bin"))
    voice = sum(model.get_voice_style(name) * weight for name, weight in BLEND.items())

    def speak(line: dict, sentence: str) -> tuple[np.ndarray, int]:
        return model.create(sentence, voice=voice, speed=SPEED, lang="en-us")

    return speak


def name_off(heard: list[str]) -> int:
    """How many phonemes the name is off by where it fits best among those heard (edit distance)."""
    heard = [SAME_PHONEME.get(symbol, symbol) for symbol in heard]
    row = [0] * (len(heard) + 1)
    for i, want in enumerate(NAME_PHONEMES, 1):
        prev, row = row, [i] + [0] * len(heard)
        for j, got in enumerate(heard, 1):
            row[j] = min(prev[j] + 1, row[j - 1] + 1, prev[j - 1] + (want != got))
    return min(row)


def spoken(text: str) -> list[str]:
    """The words of a line to listen for, leaving out the app's name (spelled out for the voice)."""
    return [word for word in re.findall(r"[a-z']+", text.lower()) if word != "quaesito"]


def sentences(text: str) -> list[str]:
    """A line's sentences ("And the winner is..." counts as one)."""
    return [part for part in re.split(r"(?<=[.!?…])\s+", text.strip()) if part]


def said(line: dict, speak) -> tuple[np.ndarray, int]:
    """A line spoken a sentence at a time, with a pause between sentences."""
    clips = []
    for sentence in sentences(line["say"]):
        samples, rate = speak(line, sentence)
        clips.append(trimmed(samples, rate))
    gap = np.zeros(round(PAUSE * rate), dtype=clips[0].dtype)
    joined = [clips[0]]
    for clip in clips[1:]:
        joined += [gap, clip]
    return np.concatenate(joined), rate


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
    videos = sys.argv[2:] or sorted(path.stem for path in NARRATION.glob("*.json"))
    speak = None
    for video in videos:
        speak = narrate(video, speak, engines[engine])


def narrate(video: str, speak, engine):
    """Speaks one video's lines; returns the voice, loaded on first use."""
    out_dir = OUT / video
    out_dir.mkdir(parents=True, exist_ok=True)
    lines = json.loads((NARRATION / f"{video}.json").read_text())
    manifest = {}
    for line in lines:
        out = out_dir / f"{line['id']}.wav"
        if source := recorded(line["id"]):
            subprocess.run(
                ["ffmpeg", "-loglevel", "error", "-y", "-i", str(source), "-ac", "1", str(out)],
                check=True,
            )
            info = sf.info(out)
            manifest[line["id"]] = round(info.frames / info.samplerate * 1000)
            print(f"{line['id']}: {manifest[line['id']]} ms (recorded: {source.name})")
            continue
        speak = speak or engine()
        samples, rate = said(line, speak)
        sf.write(out, samples, rate)
        manifest[line["id"]] = round(len(samples) / rate * 1000)
        print(f"{line['id']}: {manifest[line['id']]} ms ({engine.__name__})")
    (out_dir / "manifest.json").write_text(json.dumps(manifest, indent=2))
    return speak


if __name__ == "__main__":
    main()
