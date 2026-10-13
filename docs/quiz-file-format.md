# Quiz file format

An exported quiz is a `.zip` file. You can also write one yourself, or generate one with a script,
and bring it in with **Import** on the admin page.

```text
my-quiz.zip
├── quiz.json        the quiz: settings, questions and slides
└── media/           the pictures, videos and soundtracks it uses
    ├── 3f9c….jpg
    └── 81ab….mp3
```

`quiz.json` names its media by file name, and each one is looked up under `media/`. A quiz without
pictures or soundtracks can also be imported as a bare `.json` file. Files are checked like fresh
uploads; a picture or soundtrack that is missing from the bundle is left out, and the question
keeps the rest.

## The schema

The format is described by a [JSON Schema](https://json-schema.org/):
[`quiz-1.json`](schema/quiz-1.json). Exports point to it from their first line:

```json
{
  "$schema": "https://ggirelli.github.io/quaesito/schema/quiz-1.json",
  "format": 1,
  "id": "4d1f…",
  "title": "Movie night",
  "settings": { "reveal": "AtEnd", "points": "Flat" },
  "questions": [
    {
      "text": "Which planet has the most known moons?",
      "answers": [
        { "text": "Jupiter" },
        { "text": "Saturn", "correct": true }
      ]
    },
    {
      "text": "Who directed Jaws?",
      "kind": { "Text": { "accepted": ["Steven Spielberg", "Spielberg"] } }
    },
    { "text": "", "kind": { "Slide": { "title": "Half time!" } } }
  ]
}
```

Editors that understand JSON Schema, such as VS Code, use it to check the file and suggest field
names as you type. Most fields can be left out: `id` and `title` are required, and each question
needs its `text`. A question without a `kind` is multiple choice. The [question
types](question-types.md) page explains each kind; in the file they are called `Choice`, `Text`,
`Number`, `Order`, `TrueFalse`, `Parts`, `Pin`, `Pairs` and `Slide`.

The schema describes what a file may contain. A quiz can match it and still not be ready to play,
for example a multiple-choice question without a right answer: the editor lists what is missing.

## Versions

`format` says which version of the format a file is written in. Files without it, such as exports
from Quaesito 0.3 and earlier, are version 1. The number goes up whenever a newer Quaesito adds
something an older one would misread or lose, such as a new kind of question, and the schema of each
version stays published next to the others.

When you import a quiz made with a newer version, Quaesito asks before importing it and says what
it cannot read. Questions of kinds it does not know would be left out; **Import without them**
brings in the rest, and **Cancel** imports nothing. To keep everything, update Quaesito first.

!!! note

    Quaesito 0.3 and earlier do not know about newer files: they refuse a quiz with a kind of
    question they do not know, however many questions they could read.
