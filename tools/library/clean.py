# /// script
# requires-python = ">=3.11"
# dependencies = ["jsonschema>=4.23"]
# ///
"""Build library/checked from library/raw, keeping only entries that passed review.

Drops every entry with a finding in library/review/<category>.json, every entry check.py reports an
error for, and repeats of a question already kept. Repairs in <batch>-repaired.json files join their
batch, and replace the entry named by their `replaces` field or, without one, an entry a fix only
mentioned ("remove the year from entry 10") that they closely resemble. Then fixes what needs no judgement: repeated accepted spellings, and Number ranges with the
answer in their exact middle. Also writes every kept entry to library/library.json, which the app
bundles. Run with `uv run tools/library/clean.py`.
"""

import json
import re
import zlib
from difflib import SequenceMatcher
from pathlib import Path

from check import ROOT, check_entry, content, kind_of, norm, same_key

RAW, REVIEW, CHECKED = ROOT / "library/raw", ROOT / "library/review", ROOT / "library/checked"
# Every checked entry in one file, bundled into the app.
BUNDLE = ROOT / "library/library.json"


def dedupe(accepted):
    seen, kept = set(), []
    for a in accepted:
        if norm(a) not in seen:
            seen.add(norm(a))
            kept.append(a)
    return kept


def off_centre(q):
    """Move a Number range so its answer sits at 30% or 70% of it, keeping its width."""
    body = q["kind"]["Number"]
    n, r = body.get("answer"), body.get("range")
    if n is None or not r or abs((n - r["min"]) / (r["max"] - r["min"]) - 0.5) >= 0.02:
        return
    width = r["max"] - r["min"]
    # Which side depends on the text, so reruns give the same file.
    shares = (0.3, 0.7) if zlib.crc32(q["text"].encode()) % 2 else (0.7, 0.3)
    for share in shares:
        low = n - share * width
        if r["min"] < 0 or low >= 0:  # do not make a range of counts or sizes go negative
            break
    if all(float(v).is_integer() for v in (r["min"], r["max"], n)):
        low = round(low)
    body["range"] = {"min": low, "max": low + width}


def tidy(q):
    k = kind_of(q)
    if k == "Text":
        q["kind"]["Text"]["accepted"] = dedupe(q["kind"]["Text"]["accepted"])
    elif k == "Parts":
        for p in q["kind"]["Parts"]["parts"]:
            p["accepted"] = dedupe(p["accepted"])
    elif k == "Number":
        off_centre(q)


def similarity(a, b):
    return SequenceMatcher(None, json.dumps(a, ensure_ascii=False), json.dumps(b, ensure_ascii=False)).ratio()


def replaced_by_repairs(entries, repairs, mentioned):
    """Which entries the repairs rewrite: each repair goes to the mentioned entry it resembles most."""
    replaced = {r["replaces"] for r in repairs if isinstance(r.get("replaces"), int)}
    for r in repairs:
        if "replaces" in r:
            continue
        best = max(mentioned - replaced, key=lambda i: similarity(entries[i - 1]["question"], r["question"]), default=None)
        if best and similarity(entries[best - 1]["question"], r["question"]) >= 0.5:
            replaced.add(best)
    return replaced


def main():
    flagged, mentioned = set(), {}
    for f in REVIEW.glob("*.json"):
        for x in json.loads(f.read_text()):
            flagged.add((x["file"], x["entry"]))
            # Fixes name other entries of the same batch as "entry 10" or "entries 5 and 9"; ones in
            # another batch name its file first.
            for clause in re.findall(r"(?<!json )entr(?:y|ies) ([\d, and]+)", x["fix"]):
                for n in re.findall(r"\d+", clause):
                    mentioned.setdefault(x["file"], set()).add(int(n))
    seen = set()
    kept = dropped = 0
    bundle = []
    for f in sorted(RAW.rglob("*.json")):
        if f.stem.endswith("-repaired"):
            continue
        entries = json.loads(f.read_text())
        repaired = f.with_name(f.stem + "-repaired.json")
        repairs = json.loads(repaired.read_text()) if repaired.exists() else []
        candidates = {n for n in mentioned.get(f.name, set()) if 0 < n <= len(entries)} - {
            i for (name, i) in flagged if name == f.name}
        replaced = replaced_by_repairs(entries, repairs, candidates)
        out = []
        for i, e in enumerate(entries + repairs, 1):
            if i <= len(entries) and i in replaced:
                dropped += 1
                continue
            if i > len(entries):
                i = None
            problems = []
            check_entry(e, problems)
            key = (norm(e["question"]["text"]), content(e["question"]))
            if (i and (f.name, i) in flagged) or any(p[0] == "error" for p in problems) or key in seen:
                dropped += 1
                continue
            seen.add(key)
            e.pop("replaces", None)
            tidy(e["question"])
            out.append(e)
        target = CHECKED / f.relative_to(RAW)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(out, indent=2, ensure_ascii=False) + "\n")
        kept += len(out)
        bundle += out
    # One entry per line keeps diffs readable.
    lines = ",\n".join(json.dumps(e, ensure_ascii=False, separators=(",", ":")) for e in bundle)
    BUNDLE.write_text(f"[\n{lines}\n]\n")
    print(f"kept {kept}, dropped {dropped}")


if __name__ == "__main__":
    main()
