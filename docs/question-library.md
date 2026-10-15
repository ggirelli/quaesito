# The question library

Quaesito comes with about 2,700 ready-made questions in 14 categories and 99 topics, each rated
easy, medium or hard. Hosts browse them on the admin page's **Questions** tab, add them from the
editor, or fill a new quiz with them at random (see
[Building a quiz](building-a-quiz.md#the-question-library)). This page is for those who look after
the library itself.

## How the questions were made

1. **Generated** with ChatGPT: 99 batches of 30 questions, one per topic, from the prompt below and
   the [quiz file schema](quiz-file-format.md).
2. **Checked by script** against the schema and the library's rules (see
   [Checking the library](#checking-the-library)).
3. **Fact-checked** by Claude, one reviewer per category, then a second pass trying to refute each
   finding. One question in five had a confirmed problem: duplicates, answers given away by the
   question, ambiguous or wrong answers, poor questions. Typed answers, numbers and several-answer
   questions were the weakest; multiple choice and true/false the best.
4. **Repaired** with ChatGPT, following each finding's fix, and every repair checked again by
   Claude. Questions that could not be fixed, repeats across categories and questions whose answers
   may change over the years were dropped.

The batches, findings and repairs are in the project's git history. The maintainer is reviewing
every question by hand; even so, read the ones you pick before playing.

## Files

Each topic is a file, `library/<category>/<category>-<topic>.json`: a JSON array of entries. The app
combines them when it is built, so a change reaches the app with the next build.

```json
{
  "category": "Science",
  "topic": "Astronomy & space",
  "difficulty": 2,
  "question": { "text": "...", "answers": [{ "text": "...", "correct": true }, { "text": "..." }] }
}
```

- `question` is exactly a question of the [quiz file format](quiz-file-format.md), so it can be
  copied into a quiz as is.
- `difficulty`: 1 most adults know it, 2 keen quizzers, 3 experts.
- Answer types used: multiple choice, true/false, typed answer, number, order, several answers and
  pairs. No pictures, sound, pins or slides, no time limits or points, and no facts that may change
  over the years.

## Checking the library

```sh
uv run tools/library/check.py           # the whole library; must report 0 errors
uv run tools/library/check.py new.json  # a batch before adding it
just test                               # every question must also be playable
```

`check.py` reports errors (the question cannot be used as is) and warnings (worth a look):

- one right answer among 2–6 options, and no repeated options
- number answers strictly inside their range, with a warning when right in its middle
- order: 2–6 distinct items; pairs: 2–6 distinct items per side; several answers: 2–8 parts
- **no two parts accepting the same answer**: each part is judged on its own, so a team could type
  one answer in every part and score full marks ("name three Axis powers, in any order" is
  rejected)
- the same question twice; near-identical wording is a warning, usually harmless ("What is the
  capital of …")

## Adding questions

Attach `docs/schema/quiz-1.json` to a conversation with an AI model, paste the prompt below and fill
in the batch at the end. Ask for number and typed-answer questions separately if quality matters:
they were the most error-prone. Check the result with `check.py`, fact-check it, then save it as a
topic file (or add its entries to one) and check the library again.

??? note "The prompt used for the default questions"

    ```text
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
    - topic: <e.g. Astronomy & space>
    - 30 entries: 12 multiple choice, 5 TrueFalse, 4 typed answer, 4 Number, 2 Order,
      1 Several answers, 2 Pairs
    - difficulty: about 10 each of 1, 2 and 3
    - avoid repeating these existing questions: <paste question texts from the topic, or "none">
    ```
