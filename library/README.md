# Question library

A bank of ready-made quiz questions that hosts will be able to pick from in the editor, instead of
writing every question themselves. Questions were generated with ChatGPT from a prompt and the quiz
schema, then checked by script and fact-checked by Claude.

**Status (2026-10-14): work in progress**, committed locally on the `question-library` bookmark (not
pushed), checked out in the jj workspace `.workspaces/library`. The app does not use the library
yet; see [Next steps](#next-steps).

## Layout

```text
library/
├── raw/<category>/<category>-<topic>.json            ChatGPT batches, 30 entries each, as generated
├── raw/<category>/<category>-<topic>-repaired.json   ChatGPT repairs of reviewed entries
├── review/<category>.json                            confirmed fact-check findings, one per entry
└── checked/<category>/<category>-<topic>.json        the clean library, built by tools/library/clean.py
```

Never edit `checked/` by hand: it is rebuilt from `raw/` and `review/`. Fix things in a
`-repaired.json` file (or in the scripts) and rebuild.

## Entry format

Each file is a JSON array of entries. `question` is exactly a `Question` from the published quiz
schema ([`docs/schema/quiz-1.json`](../docs/schema/quiz-1.json)), so it can be copied into a quiz as is.

```json
{
  "category": "Science",
  "topic": "Astronomy",
  "difficulty": 2,
  "time_sensitive": false,
  "question": { "text": "...", "answers": [{ "text": "...", "correct": true }, { "text": "..." }] }
}
```

- `difficulty`: 1 most adults know it, 2 keen quizzers, 3 experts.
- `time_sensitive`: the answer may change over the years (records, holders); phrased "As of <year>".
- Kinds used: multiple choice (no `kind`), `TrueFalse`, `Text`, `Number`, `Order`, `Parts`, `Pairs`.
  No pictures, audio, `Pin` or `Slide`, and no per-question timing or scoring fields.

There are 14 categories and 99 topics (folders under `raw/`).

## Scripts

Both are self-contained [uv](https://docs.astral.sh/uv/) scripts; run them from the repository root.

```sh
uv run tools/library/clean.py                       # rebuild checked/ from raw/ and review/
uv run tools/library/check.py library/checked      # must report 0 errors
uv run tools/library/check.py library/raw/x/y.json # check single files, e.g. a new repair
```

**`check.py`** validates every question against the quiz schema and the library rules, and reports
errors (unusable) and warnings (worth a look):

- one right answer among 2–6 options, no repeated options
- Number answers strictly inside their range (warns when in its exact middle)
- Order 2–6 distinct items, Pairs 2–6 distinct items per side, Parts 2–8 parts
- **no two Parts accepting the same answer**: each part is graded on its own, so a team could type one
  answer in every part and score full marks ("name three Axis powers, any order" is rejected)
- exact duplicates across the whole library (Order/Pairs/Parts compared by their items, not just
  their often generic text); near-duplicate texts are warnings, mostly harmless shared wording

**`clean.py`** builds `checked/`:

1. Drops every raw entry that has a finding in `review/` (repairs replace them, see below).
2. Adds the entries of each `-repaired.json` file to its batch. A repair replaces the entry named in
   its `replaces` field. Older repairs have none: fixes often say "remove the year from entry 10",
   so ChatGPT returned a rewrite of an entry that was not itself flagged, and when a repair closely
   resembles an entry a fix mentions, it replaces that entry. `replaces` is removed from `checked/`.
3. Drops anything `check.py` reports as an error, and later copies of a question already kept.
4. Tidies what needs no judgement: removes repeated accepted spellings, and moves Number ranges
   whose answer sits in the exact middle (teams would learn to guess the midpoint).

## History so far

1. **Generated** 99 batches × 30 = 2,970 entries with ChatGPT (prompt below).
2. **Checked by script:** all valid against the schema; a few broken Pairs and ranges, duplicates.
3. **Fact-checked** by Claude: one reviewer agent per category, then a second agent per category
   trying to refute each finding (about 2.2M tokens, most of a Claude quota). 599 entries (20%) had
   a confirmed problem: 215 duplicates, 138 misleading (answer given away, wrong unit, missing
   spellings), 107 ambiguous, 87 wrong answers, 49 poor quality, 3 outdated. Typed answers, Number
   and Parts questions were the weakest (about a third to a half flagged), multiple choice and
   true/false the best (10–14%). The findings are in `review/`, each with an explanation and a fix.
4. **Repaired** with ChatGPT for 7 of 14 categories (art-design, film-tv, food-drink, geography,
   history, language-words, literature). Claude reviewed every repair against its fix: most were
   right; it fixed 4 entries by hand and taught `clean.py` and `check.py` about the problems found
   (rewrites of unflagged entries, "any order" Parts).

Current `checked/`: **2,540 entries, 0 errors**, 45 warnings (38 near-duplicates, of which only one is
a real repeat: Pompeii in both geography-cities-landmarks and history-ancient-world).

## Next steps

1. **Repair the remaining 7 categories** with ChatGPT: music, nature, pop-culture, science,
   society-culture, sport, technology (about 180 fixes). For each, attach the category's files from
   `raw/<category>/` and `review/<category>.json`, and paste the repair prompt below. Save the
   results next to their batches as `<batch>-repaired.json`.
2. **Check the repairs**, rebuild and check the library:
   `uv run tools/library/check.py library/raw/<category>/*-repaired.json`, then the two commands in
   [Scripts](#scripts). Ideally have Claude read the repairs against their fixes as was done for the
   first 7 categories: a script lists each repair next to its fix cheaply (no agents needed).
3. **Push** the `question-library` bookmark and open a PR to main. Its first commit (ignoring
   `__pycache__/` and `.workspaces/`) could go to main on its own first.
4. **Library schema**: a small `docs/schema/library-1.json` for entries, whose `question` refers to
   `quiz-1.json#/$defs/Question`.
5. **Editor panel** ("Add from library"): filter by category, difficulty and kind, search, add one by
   one or "10 medium history questions"; remember questions already played. The app bundles one file
   combined from `checked/`.
6. Optional app change: grade Parts so that an answer repeated in another part counts once. Then
   "name three, in any order" questions become possible and the check could be relaxed.

## Prompts

### Generating a batch

Attach `docs/schema/quiz-1.json`, paste this, and fill in the batch at the end. Ask for Number and
typed-answer questions in a separate request if quality matters: those were the most error-prone.

````text
You are writing trivia questions for Quaesito, a pub-quiz app played on a TV with phones as
buzzers. The attached JSON Schema (quiz-1.json) defines a question: use its "Question"
definition exactly, with no other fields.

OUTPUT
Return one JSON array, and nothing else, as a downloadable file named <category>-<topic>.json.
Each element is a library entry:
{
  "category": "<given below>",
  "topic": "<given below>",
  "difficulty": 1 | 2 | 3,          // 1 = most adults know it, 2 = keen quizzers, 3 = experts
  "time_sensitive": true | false,   // true if the answer could change in future years
  "question": { ...a Question from quiz-1.json... }
}

QUESTION TYPES (use only these; never use Pin or Slide, never set image, audio,
image_reveal, image_hold_secs, multiplier, time_limit_secs or bonus)
- Multiple choice: omit "kind". Exactly 4 "answers", exactly one with "correct": true
  (leave "correct" out of the others). Distractors are plausible, the same kind of
  thing as the right answer, and of similar length. Never "all/none of the above".
- TrueFalse: "kind": {"TrueFalse": {"answer": true|false}}. "text" is the statement.
  Use both true and false statements, about half each. Avoid statements that are
  true only on a technicality.
- Typed answer: "kind": {"Text": {"accepted": [...]}}. Answers of 1–3 words. List every
  reasonable spelling: full name, surname only, common abbreviations, with and without
  "The". Only use this type when the answer cannot be mistaken for something else.
- Number: "kind": {"Number": {"answer": N, "range": {"min": A, "max": B}}}, with
  A < N < B and a range wide enough that N is not obviously in the middle. Years,
  heights, counts, distances. Name the unit in the question text.
- Order: "kind": {"Order": {"items": [...]}}, 4–6 short items listed in the correct
  order. The question states the order exactly ("earliest first", "largest first"),
  and no two items may be tied. Items must not reveal the order (no years in them).
- Several answers: "kind": {"Parts": {"parts": [{"label": "...", "accepted": [...]}, ...]}},
  2–5 parts. Each label says what to type ("Title", "Year", "1st", ...), and no two
  parts may accept the same answer.
- Pairs: "kind": {"Pairs": {"pairs": [{"left": {"text": "..."}, "right": {"text": "..."}}, ...]}},
  4–6 pairs, text only, listed matched, each left item with exactly one possible partner.

STYLE
- Question text under 120 characters and read aloud easily; answers under 40 characters.
  Teams read them on small phone screens.
- One fact per question, unambiguous, with a single defensible right answer.
- Never put the answer, or another question's answer, in the question text.
- Prefer interesting over obscure: a "huh, I didn't know that" fact beats a date to recall.
- No lyrics, long quotes, offensive topics or questions about living private people.
- "passage" is optional, for a short clue or a quote under 25 words.

ACCURACY (most important)
- Use only well-established facts you are confident about. If unsure, drop the question
  rather than guess.
- Avoid facts that change (records, populations, "current" holders, counts that grow).
  If one is unavoidable, write "As of <year>, ..." and set "time_sensitive": true.
- Before returning, check each entry again: is the marked answer right, and is no other
  answer also right? Fix or remove any that fail.
- No two entries may ask the same thing, even reworded.

This batch:
- category: <e.g. Science>
- topic: <e.g. Astronomy>
- 30 entries: 12 multiple choice, 5 TrueFalse, 4 typed answer, 4 Number, 2 Order,
  1 Several answers, 2 Pairs
- difficulty: about 10 each of 1, 2 and 3
- avoid repeating these existing questions: <paste question texts from earlier batches, or "none">
````

### Repairing reviewed entries

Attach the category's batch files from `raw/<category>/` and `review/<category>.json`. Compared with
the version used for the first 7 categories, this one asks for a `replaces` field, which `clean.py`
uses to know which entry a repair rewrites (without it, it guesses by resemblance), and forbids the
"any order" Parts loophole.

````text
The attached JSON files are batches of quiz questions (arrays of library entries, each wrapping a
"question" in Quaesito's quiz format). The review file lists problems a fact-check found: each item
names a batch "file", a 1-based "entry" number, the "problem", an "explanation" with the correct
facts, and a "fix".

For every review item whose fix is NOT "drop":
- Take that entry from its batch file and apply the fix exactly as described, using the corrected
  facts from the explanation. Change nothing else in the entry.
- If the fix offers alternatives, pick the first.
- If the fix asks to change a different entry of the batch ("remove the year from entry 10"),
  return that entry rewritten instead.
- If applying the fix would make the question repeat another question in the same batch, skip it.
- When you change a question's subject, update its wrong answers to match (wrong answers must be
  the same kind of thing as the right one).
- Keep every rule of the original format: a multiple-choice question has exactly 4 answers with one
  "correct": true; Number answers lie strictly inside their range and not in its exact middle;
  Pairs items are all different on each side; no two Parts accept the same answer (never accept
  every answer in every part); no image, audio or timing fields.
- Add a "replaces" field to each returned entry, next to "category": the 1-based number of the
  batch entry it rewrites.

Skip review items whose fix is "drop", or that begin with "drop" and give no alternative.

Return one JSON file per batch file that had repairs, named like the batch with "-repaired" added
(e.g. science-physics-repaired.json), holding only the repaired entries, as a JSON array in the
same entry format. No commentary.
````
