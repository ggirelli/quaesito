# /// script
# requires-python = ">=3.11"
# dependencies = ["jsonschema>=4.23"]
# ///
"""Copy the library edits saved in the app into the library's topic files.

The app's "edit" link on a library question saves the change on that server only, in
<data folder>/library-edits.json. This writes those edits into library/<category>/<category>-<topic>.json,
so they can be reviewed with `git diff` and committed, then checks the library with check.py.

Each edit names the question's category, topic and text as they were, and replaces that question
(and its difficulty). Edits the library cannot hold (pictures, sounds, time limits, points, pin
questions or slides) are reported and left out. Applying the same edits twice changes nothing.
Run with `uv run tools/library/apply-edits.py [path/to/library-edits.json]` (default:
data/library-edits.json).
"""

import json
import sys
from pathlib import Path

from check import ROOT, main as check

LIBRARY = ROOT / "library"
# What the library leaves out: no media, and per-question timing and scoring are the host's choice.
UNFIT = {
    "image": "a picture",
    "explanation_image": "a picture shown with its answer",
    "explanation_audio": "a soundtrack played with its answer",
    "audio": "a soundtrack",
    "time_limit_secs": "its own time limit",
    "multiplier": "its own points",
    "bonus": "a bonus question",
}


def tidy(value):
    """The app writes every field; the library leaves out the empty and default ones."""
    if isinstance(value, dict):
        out = {}
        for key, v in value.items():
            v = tidy(v)
            if v is None or v == [] or (key == "correct" and v is False):
                continue
            defaults = [("image_reveal", "Shown"), ("image_hold_secs", 0), ("passage", ""), ("kind", "Choice")]
            if (key, v) in defaults:
                continue
            out[key] = v
        return out
    if isinstance(value, list):
        return [tidy(v) for v in value]
    if isinstance(value, float) and value.is_integer():
        return int(value)
    return value


def in_order_of(new, old):
    """`new` with its keys in `old`'s order where they share them, so diffs show only real changes."""
    if isinstance(new, dict) and isinstance(old, dict):
        keys = [k for k in old if k in new] + [k for k in new if k not in old]
        return {k: in_order_of(new[k], old.get(k)) for k in keys}
    if isinstance(new, list) and isinstance(old, list):
        return [in_order_of(n, o) for n, o in zip(new, old)] + new[len(old):]
    return new


def problems(question):
    """Why the library cannot hold this question, if it cannot."""
    found = [f"has {what}" for key, what in UNFIT.items() if question.get(key) is not None]
    kind = question.get("kind")
    if isinstance(kind, dict) and ("Pin" in kind or "Slide" in kind):
        found.append("is a pin question or a slide")
    nested = json.dumps([question.get("answers"), question.get("kind")])
    if '"image": "' in nested:
        found.append("has a picture in its answers")
    return found


def main(args):
    edits_path = Path(args[0]) if args else ROOT / "data/library-edits.json"
    if not edits_path.exists():
        print(f"no edits at {edits_path}")
        return 1
    edits = json.loads(edits_path.read_text())
    files = {}  # path -> entries, loaded on demand
    applied = unchanged = skipped = 0
    for id_, edit in edits.items():
        where = f"{edit.get('category')} › {edit.get('topic')}: {edit.get('text')!r}"
        found = None
        for path in sorted(LIBRARY.rglob("*.json")):
            entries = files.get(path) or json.loads(path.read_text())
            for i, entry in enumerate(entries):
                if (entry["category"], entry["topic"], entry["question"]["text"]) == (
                    edit.get("category"), edit.get("topic"), edit.get("text")):
                    found = (path, entries, i)
                    break
            if found:
                break
        question = edit["question"]
        why = problems(question)
        if why:
            print(f"skipped {where}: it {', '.join(why)}")
            skipped += 1
            continue
        if not found:
            # Applied before (with a new text), or the question left the library.
            if any(e["question"] == tidy(question) and e["category"] == edit.get("category")
                   for path in LIBRARY.rglob("*.json")
                   for e in files.get(path) or json.loads(path.read_text())):
                unchanged += 1
            else:
                print(f"not found {where}")
                skipped += 1
            continue
        path, entries, i = found
        new = dict(entries[i], difficulty=edit["difficulty"])
        new["question"] = in_order_of(tidy(question), entries[i]["question"])
        if new == entries[i]:
            unchanged += 1
            continue
        entries[i] = new
        files[path] = entries
        applied += 1
        print(f"applied {where}")
    for path, entries in files.items():
        path.write_text(json.dumps(entries, indent=2, ensure_ascii=False) + "\n")
    print(f"\n{applied} applied, {unchanged} already there, {skipped} skipped, "
          f"{len(files)} files changed\n")
    return check([]) or (1 if skipped else 0)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
