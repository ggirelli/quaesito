# Question library

Ready-made quiz questions that hosts browse on the admin page's **Questions** tab, add from the
editor (**＋ From the library**), or fill a new quiz with at random. See the user guide's
[Building a quiz](../docs/building-a-quiz.md#the-question-library).

`library.json` is the library: the server bundles it at build time (`src/server/library.rs`), and
nothing else in this folder ships. Edit it directly, then check it:

```sh
uv run tools/library/check.py                  # must report 0 errors
just test                                      # every question must also be playable
```

## Entry format

A JSON array with one entry per line (easy to diff and grep). `question` is exactly a `Question`
from the published quiz schema ([`docs/schema/quiz-1.json`](../docs/schema/quiz-1.json)), so it can
be copied into a quiz as is.

```json
{"category":"Science","topic":"Astronomy & space","difficulty":2,"question":{"text":"...","answers":[{"text":"...","correct":true},{"text":"..."}]}}
```

- `difficulty`: 1 most adults know it, 2 keen quizzers, 3 experts.
- Kinds used: multiple choice (no `kind`), `TrueFalse`, `Text`, `Number`, `Order`, `Parts`, `Pairs`.
  No pictures, audio, `Pin` or `Slide`, no per-question timing or scoring fields, and no facts that
  may change over the years.

**`check.py`** validates every question against the quiz schema and the library rules, and reports
errors (unusable) and warnings (worth a look):

- one right answer among 2–6 options, no repeated options
- Number answers strictly inside their range (warns when in its exact middle)
- Order 2–6 distinct items, Pairs 2–6 distinct items per side, Parts 2–8 parts
- **no two Parts accepting the same answer**: each part is graded on its own, so a team could type one
  answer in every part and score full marks ("name three Axis powers, any order" is rejected)
- exact duplicates; near-duplicate texts are warnings, mostly harmless shared wording ("What is the
  capital of …")

## How it was made

2,970 questions in 99 topic batches across 14 categories were generated with ChatGPT (prompt below),
checked by script, and fact-checked by Claude with a second pass trying to refute each finding.
599 (20%) had a confirmed problem; ChatGPT repaired most of them, Claude checked every repair, and
the rest were dropped, as were repeats and questions whose answers may change. The batches,
findings, repairs and scripts are in the git history (before the commit that left only
`library.json` here).

## Adding questions

Attach `docs/schema/quiz-1.json`, paste this prompt and fill in the batch at the end. Ask for Number
and typed-answer questions in a separate request if quality matters: those were the most
error-prone. Check the result with `uv run tools/library/check.py <batch>.json`, have it
fact-checked, then append its entries to `library.json` and check that again.

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
- Never use facts that may change (records, populations, "current" holders, counts that
  grow, borders and sovereignty in dispute).
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


## Next steps

- **Review** the questions by hand (in progress), fixing `library.json` directly.
- **Library schema**: a small `docs/schema/library-1.json` for entries, whose `question` refers to
  `quiz-1.json#/$defs/Question`.
- **Remember questions already played**, so random picks prefer fresh ones.
- Optional app change: grade Parts so that an answer repeated in another part counts once. Then
  "name three, in any order" questions become possible and the check could be relaxed.
