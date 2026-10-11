// The sample quiz used for documentation screenshots and the demo video. It only uses the
// project's own artwork, placed straight into the test server's media folder.
const { execFileSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '../..');
const MEDIA = path.join(ROOT, 'target/e2e/data/media');
const ART = path.join(ROOT, 'assets/brand/lobby.webp');

/** Converts (and optionally crops) the lobby art into a media file; returns its name. */
function media(name, crop) {
  fs.mkdirSync(MEDIA, { recursive: true });
  const file = `${name.padEnd(32, '0').slice(0, 32)}.jpg`;
  const args = [ART, '-background', '#14121f', '-flatten'];
  if (crop) args.push('-crop', crop, '+repage');
  args.push('-quality', '88', path.join(MEDIA, file));
  execFileSync('magick', args);
  return file;
}

/**
 * A soundtrack made here, so it is free to use: a little tune going up the scale twice, as a WAV
 * file in the media folder. Returns its name.
 */
function tune(name) {
  fs.mkdirSync(MEDIA, { recursive: true });
  const rate = 22050;
  const note = 0.3;
  const scale = [0, 2, 4, 5, 7, 9, 11, 12];
  const notes = [...scale, ...scale];
  const samples = new Int16Array(Math.round(rate * note * notes.length));
  notes.forEach((step, n) => {
    const freq = 523.25 * 2 ** (step / 12);
    for (let i = 0; i < rate * note; i++) {
      const t = i / rate;
      const envelope = Math.min(1, t / 0.01) * Math.exp(-4 * t);
      const tone = Math.sin(2 * Math.PI * freq * t) + 0.3 * Math.sin(4 * Math.PI * freq * t) + 0.15 * Math.sin(6 * Math.PI * freq * t);
      samples[Math.round(n * rate * note) + i] = Math.round(9000 * envelope * tone);
    }
  });
  const data = Buffer.from(samples.buffer);
  const header = Buffer.alloc(44);
  header.write('RIFF', 0); header.writeUInt32LE(36 + data.length, 4); header.write('WAVE', 8);
  header.write('fmt ', 12); header.writeUInt32LE(16, 16); header.writeUInt16LE(1, 20); header.writeUInt16LE(1, 22);
  header.writeUInt32LE(rate, 24); header.writeUInt32LE(rate * 2, 28); header.writeUInt16LE(2, 32); header.writeUInt16LE(16, 34);
  header.write('data', 36); header.writeUInt32LE(data.length, 40);
  const file = `${name.padEnd(32, '0').slice(0, 32)}.wav`;
  fs.writeFileSync(path.join(MEDIA, file), Buffer.concat([header, data]));
  return file;
}

/** Team pictures: crops of the lobby illustration. */
function avatars(dir) {
  fs.mkdirSync(dir, { recursive: true });
  const crops = { bears: '170x170+60+345', newton: '170x170+770+385', popcorn: '170x170+395+400' };
  return Object.fromEntries(Object.entries(crops).map(([team, crop]) => {
    const out = path.join(dir, `${team}.png`);
    execFileSync('magick', [ART, '-background', '#14121f', '-flatten', '-crop', crop, '+repage', out]);
    return [team, out];
  }));
}

function quiz() {
  const art = media('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1');
  const snack = media('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2', '480x360+240+300');
  const choice = (text, answers, right) => ({
    text, answers: answers.map((a, i) => ({ text: a, correct: i === right })),
  });
  return {
    title: 'Friday Night Quiz',
    questions: [
      { text: 'Phones out, team names ready: questions about space, history and art.', kind: { Slide: { title: 'Round 1: Warm-up' } }, image: art, answers: [] },
      choice('Which planet has the most known moons?', ['Jupiter', 'Saturn', 'Uranus', 'Neptune'], 1),
      { text: 'Honey found in ancient Egyptian tombs was still edible.', kind: { TrueFalse: { answer: true } }, answers: [] },
      { text: 'What is the capital of Australia?', kind: { Text: { accepted: ['Canberra'] } }, answers: [] },
      { text: 'How many keys does a standard piano have?', kind: { Number: { answer: 88 } }, answers: [] },
      { text: 'Oldest first!', kind: { Order: { items: ['Pyramids of Giza', 'Colosseum', 'Eiffel Tower', 'Burj Khalifa'] } }, answers: [] },
      { text: 'The Mona Lisa: who painted it, and in which city can you see it?', kind: { Parts: { parts: [
        { label: 'Painter', accepted: ['Leonardo da Vinci', 'Leonardo', 'da Vinci'] },
        { label: 'City', accepted: ['Paris'] },
      ] } }, multiplier: 2, answers: [] },
      { text: 'Where is the popcorn?', image: art, kind: { Pin: { answer: { x: 0.5, y: 0.68 }, aspect: 0.75, tolerance: 'Normal' } }, answers: [] },
      { ...choice('What are the friends snacking on?', ['Popcorn', 'Nachos', 'Pretzels', 'Grapes'], 0), image: snack },
      { ...choice('Which way does this tune go?', ['Up the scale', 'Down the scale', 'Up and down', 'Same note'], 0), audio: tune('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa3') },
    ],
  };
}

module.exports = { quiz, avatars, tune };
