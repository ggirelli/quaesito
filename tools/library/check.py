# /// script
# requires-python = ">=3.11"
# dependencies = ["jsonschema>=4.23"]
# ///
"""Check the question library (library/<category>/<category>-<topic>.json) against the quiz schema
and the library rules.

Prints errors (the entry cannot be used as is) and warnings (worth a look), and exits non-zero on
errors. Run with `uv run tools/library/check.py [files or folders...]`; it checks the whole library
when given none, and any JSON arrays of entries otherwise (a batch to add, say).
"""

import json
import re
import sys
import unicodedata
from collections import Counter, defaultdict
from difflib import SequenceMatcher
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "docs/schema/quiz-3.json"
ENTRY_KEYS = {"category", "topic", "difficulty", "question"}
# Fields a text-only library question must leave out: the library has no media, and per-question
# timing and bonuses are the host's choice. Points are set by the kind of question (see check_kind).
FORBIDDEN = {"image", "explanation_image", "explanation_audio", "audio", "image_reveal", "image_hold_secs", "time_limit_secs", "bonus"}
ALLOWED_KINDS = {"Choice", "TrueFalse", "Text", "Number", "Order", "Parts", "Pairs"}
MAX_TEXT, MAX_ANSWER = 120, 40
MAX_MULTIPLIER = 5  # crates/core/src/model/question.rs


def kind_of(q):
    k = q.get("kind", "Choice")
    return k if isinstance(k, str) else next(iter(k))


def norm(s):
    s = unicodedata.normalize("NFKD", s).encode("ascii", "ignore").decode().lower()
    return re.sub(r"[^a-z0-9 ]+", "", re.sub(r"\s+", " ", s)).strip()


def same_key(s):
    """Answers and items differ when anything but case and spacing differs: symbols count."""
    return " ".join(s.casefold().split())


def content(q):
    """What a question asks for besides its text, so generic texts ("Put these films in order")
    with different items are not taken for duplicates."""
    k = kind_of(q)
    if k in ("Order", "Pairs", "Parts"):
        return json.dumps(q["kind"][k], sort_keys=True, ensure_ascii=False).casefold()
    return ""


def check_kind(q, problems):
    """Rules from the generation prompt and the app's own limits, per kind of question."""
    err = lambda m: problems.append(("error", m))
    warn = lambda m: problems.append(("warn", m))
    k = kind_of(q)
    body = q.get("kind", {}).get(k, {}) if k != "Choice" else None
    if k not in ALLOWED_KINDS:
        return err(f"kind {k} is not allowed in the library")
    if k == "Choice":
        answers = q.get("answers", [])
        texts = [a.get("text", "") for a in answers]
        if not 2 <= len(answers) <= 6:
            err(f"{len(answers)} answers, the app takes 2-6")
        elif len(answers) != 4:
            warn(f"{len(answers)} answers instead of 4")
        correct = sum(bool(a.get("correct")) for a in answers)
        # Teams may pick several answers, so a question can have more than one right.
        if correct == 0:
            err("no correct answer")
        elif correct == len(answers):
            err("every answer is correct")
        if len({same_key(t) for t in texts}) != len(texts):
            err("two answers are the same")
        if any(not t.strip() for t in texts):
            err("an answer is empty")
        for t in texts:
            if len(t) > MAX_ANSWER:
                warn(f"answer longer than {MAX_ANSWER} characters: {t!r}")
            if re.search(r"\b(all|none|both) of (the|these)\b", t, re.I):
                warn(f"'of the above' style answer: {t!r}")
    elif q.get("answers"):
        warn(f"{k} question also has multiple-choice answers")
    if k == "Text":
        accepted = body.get("accepted", [])
        if not accepted or any(not a.strip() for a in accepted):
            err("no accepted answer, or an empty one")
        if len({norm(a) for a in accepted}) != len(accepted):
            warn("accepted spellings repeat once case and punctuation are ignored")
    if k == "Number":
        n, r = body.get("answer"), body.get("range")
        if n is None:
            err("no answer")
        elif r:
            if not r["min"] < n < r["max"]:
                err(f"answer {n} not strictly inside the range {r['min']}-{r['max']}")
            elif abs((n - r["min"]) / (r["max"] - r["min"]) - 0.5) < 0.02:
                warn("answer sits in the exact middle of the range")
        else:
            warn("no range")
    if k == "Order":
        items = body.get("items", [])
        if not 2 <= len(items) <= 6:
            err(f"{len(items)} items, the app takes 2-6")
        if len({same_key(i) for i in items}) != len(items):
            err("two items are the same")
    if k == "Parts":
        parts = body.get("parts", [])
        if not 2 <= len(parts) <= 8:
            err(f"{len(parts)} parts, the app takes 2-8")
        for p in parts:
            if not p.get("label", "").strip() or not p.get("accepted"):
                err("a part has no label or no accepted answer")
        # Each part is judged on its own, so parts sharing an answer let one answer score twice.
        accepted = [{same_key(a) for a in p.get("accepted", [])} for p in parts]
        if any(accepted[i] & accepted[j] for i in range(len(accepted)) for j in range(i)):
            err("two parts accept the same answer, so typing it in both scores twice")
    # A question asking for several typed answers is worth one question per answer.
    points = min(len(body.get("parts", [])), MAX_MULTIPLIER) if k == "Parts" else 1
    if q.get("multiplier", 1) != points:
        err(f"points ×{q.get('multiplier', 1)}, should be ×{points}" + (" (one per part)" if k == "Parts" else ""))
    if k == "Pairs":
        pairs = body.get("pairs", [])
        if not 2 <= len(pairs) <= 6:
            err(f"{len(pairs)} pairs, the app takes 2-6")
        for side in ("left", "right"):
            texts = [same_key(p.get(side, {}).get("text", "")) for p in pairs]
            if any(not t for t in texts):
                err(f"a {side} item is empty")
            if len(set(texts)) != len(texts):
                err(f"two {side} items are the same")


def check_entry(e, problems):
    err = lambda m: problems.append(("error", m))
    if not isinstance(e, dict):
        return err("entry is not an object")
    if set(e) != ENTRY_KEYS:
        err(f"entry fields {sorted(e)} instead of {sorted(ENTRY_KEYS)}")
    if e.get("difficulty") not in (1, 2, 3):
        err(f"difficulty {e.get('difficulty')!r}")
    q = e.get("question")
    if not isinstance(q, dict):
        return err("no question")
    for msg in sorted(QUESTION.iter_errors(q), key=lambda x: x.path):
        err(f"schema: {'/'.join(map(str, msg.path)) or 'question'}: {msg.message[:160]}")
    if problems and any(p[1].startswith("schema") for p in problems):
        return
    if extra := FORBIDDEN & set(q):
        err(f"fields not used in the library: {sorted(extra)}")
    text = q.get("text", "")
    if not text.strip():
        err("empty question text")
    elif len(text) > MAX_TEXT:
        problems.append(("warn", f"question longer than {MAX_TEXT} characters"))
    check_kind(q, problems)


def load_question_validator():
    schema = json.loads(SCHEMA.read_text())
    registry = Registry().with_resource(schema["$id"], Resource.from_contents(schema))
    return Draft202012Validator({"$ref": schema["$id"] + "#/$defs/Question"}, registry=registry)


QUESTION = load_question_validator()


def main(args):
    paths = [Path(a).resolve() for a in args] or [ROOT / "library"]
    files = sorted(f for p in paths for f in ([p] if p.is_file() else p.rglob("*.json")))
    errors = warnings = total = 0
    seen = defaultdict(list)  # normalized question text -> where
    kinds, levels = Counter(), Counter()
    for f in files:
        rel = f.relative_to(ROOT)
        try:
            entries = json.loads(f.read_text())
        except json.JSONDecodeError as e:
            print(f"{rel}: error: not valid JSON: {e}")
            errors += 1
            continue
        if not isinstance(entries, list):
            print(f"{rel}: error: not a JSON array")
            errors += 1
            continue
        topics = Counter((e.get("category"), e.get("topic")) for e in entries if isinstance(e, dict))
        if len(topics) > 1:
            print(f"{rel}: warn: several category/topic pairs: {dict(topics)}")
            warnings += 1
        for i, e in enumerate(entries, 1):
            total += 1
            problems = []
            check_entry(e, problems)
            for level, msg in problems:
                print(f"{rel}#{i}: {level}: {msg}")
                errors += level == "error"
                warnings += level == "warn"
            if isinstance(e, dict) and isinstance(e.get("question"), dict):
                q = e["question"]
                kinds[kind_of(q)] += 1
                levels[e.get("difficulty")] += 1
                seen[(norm(q.get("text", "")), content(q))].append(f"{rel}#{i}")
    # Exact duplicates, then near duplicates among questions sharing a rare word.
    for (text, _), where in seen.items():
        if len(where) > 1:
            print(f"error: same question {len(where)} times: {', '.join(where)}: {text!r}")
            errors += 1
    # Near duplicates are looked for among the texts of questions that do not list their items.
    words = defaultdict(set)
    for text, items in seen:
        if items:
            continue
        for w in set(text.split()):
            if len(w) > 4:
                words[w].add(text)
    pairs = {tuple(sorted((a, b))) for group in words.values() if len(group) <= 40
             for a in group for b in group if a < b}
    for a, b in sorted(pairs):
        if SequenceMatcher(None, a, b).ratio() > 0.85:
            print(f"warn: near duplicates: {seen[a, ""][0]} {a!r} / {seen[b, ""][0]} {b!r}")
            warnings += 1
    print(f"\n{total} entries in {len(files)} files: {errors} errors, {warnings} warnings")
    print("kinds:", dict(kinds.most_common()))
    print("difficulty:", dict(sorted(levels.items())))
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
