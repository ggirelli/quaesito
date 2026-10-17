// Records the editor demo video: a quiz built from scratch on the admin page, from New quiz to
// testing it, with captions and a voice-over (narration/editor.json, spoken by voice.py). Writes
// docs/media/demo-editor.mp4, its captions (demo-editor.vtt) and a transcript.
//
//   just demo-video
//   DRY=1 tools/browser/run.sh editor-video.js   # try it without a voice; writes no video
const path = require('path');
const { execFileSync } = require('child_process');
const L = require('./lib.js');
const V = require('./video.js');
const sample = require('./sample.js');

const PICTURE = path.join(__dirname, 'out/editor-picture.jpg');

(async () => {
  // Another quiz on the list, so the admin page is not empty.
  const ctx = await L.setup(Object.assign(sample.quiz(), { title: 'Friday Night Quiz' }));
  execFileSync('magick', [path.resolve(__dirname, '../../assets/brand/lobby.webp'), '-background', '#14121f', '-flatten',
    '-quality', '88', PICTURE]);

  const page = await L.openAdmin(ctx, '/admin', 1440, 900);
  await V.pointer(page);
  await page.waitForSelector('.list-row');
  await page.mouse.move(700, 450);
  await L.sleep(1000);

  /** The last question or slide card. */
  const last = async () => (await page.$$('.question')).at(-1);
  /** Clicks a field and types into it, at a typing pace. */
  const write = async (target, text, delay = 45) => {
    await V.click(page, target, { steps: 15, pause: 120 });
    await page.keyboard.type(text, { delay });
  };
  /** Scrolls a card to the top of the list, under the pinned header. */
  const show = (el) => el.evaluate((e) => {
    const top = e.getBoundingClientRect().top + window.scrollY - document.querySelector('header').offsetHeight - 16;
    window.scrollTo({ top, behavior: 'smooth' });
  });
  /** Adds a question with its text and answers, the first one right. */
  const question = async (text, answers) => {
    await V.click(page, '.editor-add .button.primary');
    await L.sleep(500);
    const q = await last();
    await show(q);
    await L.sleep(700);
    await write(await q.$('.question-text'), text);
    for (const [i, answer] of answers.entries()) {
      if (i >= (await q.$$('.answer-edit')).length) {
        const add = await q.evaluateHandle((e) => [...e.querySelectorAll('button')].find((b) => b.innerText.includes('Add answer')));
        await V.click(page, add, { steps: 12, pause: 100 });
      }
      await write((await q.$$('.answer-edit input.grow'))[i], answer, 30);
    }
    return q;
  };

  // Captions at the bottom, clear of the editor's header and its save state.
  const video = await V.start(page, 'editor', { bottom: true });
  const { step } = video;

  await step('editor-intro', async () => {
    await V.click(page, await V.byText(page, 'button', 'New quiz'));
    await L.sleep(500);
    await page.keyboard.type('Movie night', { delay: 70 });
    await L.sleep(300);
    await V.click(page, await V.byText(page, 'button', 'Create quiz'));
    await page.waitForSelector('.editor-add');
  });

  let first;
  await step('editor-question', async () => {
    first = await question('Who directed Jaws?', ['Steven Spielberg', 'George Lucas', 'Ridley Scott']);
    await V.click(page, (await first.$$('.correct-toggle'))[0]);
    // Up to the header, where the save state shows.
    await page.mouse.move(720, 34, { steps: 20 });
  });

  await step('editor-types', async () => {
    const q = await question('In what year did Jaws come out?', []);
    await V.click(page, await q.$('.segmented button:nth-child(4)'));
    await L.sleep(400);
    await write(await q.$('input[placeholder="330"]'), '1975', 90);
  });

  await step('editor-picture', async () => {
    await show(first);
    await L.sleep(700);
    const input = await first.$('.question-media input[type=file]');
    await V.click(page, await first.$('.question-media label.button'));
    await input.uploadFile(PICTURE);
    const reveal = await first.waitForSelector('.question-media select', { timeout: 30000 });
    await L.sleep(600);
    await V.click(page, reveal, { steps: 15 });
    await reveal.select('zoom');
  });

  await step('editor-slide', async () => {
    await V.click(page, '.editor-add .add-slide');
    await L.sleep(500);
    const slide = await last();
    await show(slide);
    await L.sleep(600);
    await write(await slide.$('.slide-title-input'), 'Round 1: Blockbusters');
    await V.click(page, await slide.$('button[title="Move to the top"]'));
    await L.sleep(400);
    await show(await page.$('.question'));
  });

  await step('editor-library', async () => {
    await V.click(page, '.editor-add .add-library');
    await page.waitForSelector('.library-dialog .library-category');
    await L.sleep(400);
    await V.click(page, await V.byText(page, '.library-dialog .library-category', 'Film & TV'));
    await L.sleep(800);
    await V.click(page, await V.byText(page, '.library-dialog button', 'Add at random'));
    await L.sleep(1200);
    await V.click(page, await V.byText(page, '.library-dialog button', 'Close'));
  });

  await step('editor-navigate', async () => {
    // Along the strip, which opens up under the pointer, then a jump to the last question.
    const links = await page.$$('.editor-nav a');
    const a = await links[0].boundingBox();
    await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2, { steps: 20 });
    for (const link of links) {
      const b = await link.boundingBox();
      await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 4 });
      await L.sleep(60);
    }
    await V.click(page, links.at(-1), { steps: 6 });
    await L.sleep(1200);
    await V.click(page, await V.byText(page, 'button', 'Table view'));
    await L.sleep(2500);
    await V.click(page, await V.byText(page, 'button', 'Card view'));
  });

  await step('editor-test', async () => {
    await V.click(page, await V.byText(page, 'button', 'Test quiz'));
    await page.waitForFunction(() => document.querySelectorAll('.test-phone iframe').length === 2, { timeout: 60000 });
  }, 3000);

  await video.finish('demo-editor', { posterAt: 6 });
  console.log('ERRORS', ctx.errors.length ? ctx.errors.join('\n') : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
