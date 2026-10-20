// Records the question library demo video: browsing and searching the library on the admin
// page, editing a question, filling a new quiz with random questions and saving a question of
// one's own to the library, with captions and a voice-over (narration/library.json, spoken by
// voice.py). Writes docs/media/demo-library.mp4, its captions (demo-library.vtt) and a transcript.
//
//   just demo-video
//   DRY=1 tools/browser/run.sh library-video.js   # try it without a voice; writes no video
const L = require('./lib.js');
const V = require('./video.js');

(async () => {
  // A quiz with a question of its own, filed under a category and topic, ready to be saved.
  const ctx = await L.setup({
    title: 'Movie night',
    questions: [{
      text: 'Which film first said "May the Force be with you"?',
      category: 'Film & TV',
      topic: 'Blockbusters',
      answers: [{ text: 'Star Wars', correct: true }, { text: 'Dune' }, { text: 'Alien' }, { text: 'Tron' }],
    }],
  }, {}, { scale: V.density(1440) });

  const page = await L.openAdmin(ctx, '/admin', 1440, 900);
  await V.pointer(page);
  await page.waitForSelector('.list-row');
  await page.mouse.move(700, 450);
  await L.sleep(1000);
  const tab = (name) => V.byText(page, '.admin-tabs a', name);

  // Captions at the bottom, clear of the page's tabs and search.
  const video = await V.start(page, 'library', { bottom: true });
  const { step } = video;

  await step('library-intro', async () => {
    await V.click(page, await tab('Questions'));
    await page.waitForSelector('.library-category');
  });

  await step('library-browse', async () => {
    await V.click(page, await V.byText(page, '.library-category', 'Geography'));
    await L.sleep(900);
    await V.click(page, await V.byText(page, '.library-topic', 'Capitals'));
  }, 800);

  await step('library-search', async () => {
    await V.click(page, await V.byText(page, '.library-category', 'All categories'));
    await L.sleep(500);
    await V.click(page, '.library-search input', { steps: 15 });
    await page.keyboard.type('gold', { delay: 90 });
    await L.sleep(800);
    await V.click(page, await V.byText(page, '.library-option', 'Hard'));
    await L.sleep(800);
    await V.click(page, await V.byText(page, '.library-option', 'Choices'));
  }, 800);

  await step('library-edit', async () => {
    await V.click(page, await V.byText(page, 'a, button', 'edit'));
    await page.waitForSelector('.edit-library-dialog');
    await L.sleep(1200);
    await V.click(page, await V.byText(page, '.edit-library-dialog .segmented button', 'Medium'));
    await L.sleep(1000);
    await V.click(page, await V.byText(page, '.edit-library-dialog button', 'Close'));
    await L.sleep(400);
    // To the report link, without opening GitHub.
    const report = await V.byText(page, 'a', 'report');
    const b = await report.boundingBox();
    await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 20 });
  }, 800);

  await step('library-fill', async () => {
    await V.click(page, await tab('Quizzes'));
    await V.click(page, await V.byText(page, 'button', 'New quiz'));
    await L.sleep(500);
    await page.keyboard.type('Nature night', { delay: 70 });
    await V.click(page, await V.byText(page, '.dialog .check-field', 'Fill it with random'));
    await L.sleep(500);
    for (const category of ['Nature', 'Science']) {
      await V.click(page, await V.byText(page, '.random-categories .check-field', category), { steps: 15 });
      await L.sleep(250);
    }
    await V.click(page, await V.byText(page, 'button', 'Create quiz'));
    await page.waitForSelector('.question');
    await L.sleep(800);
    await page.evaluate(() => window.scrollTo({ top: document.querySelector('.question').offsetTop - 90, behavior: 'smooth' }));
  }, 1500);

  await step('library-save', async () => {
    await V.click(page, 'header a[href="/admin"]');
    await page.waitForSelector('.list-row');
    const row = await V.byText(page, '.list-row', 'Movie night');
    await V.click(page, await row.$('[aria-label="Edit"]'));
    await page.waitForSelector('.library-sync');
    await L.sleep(800);
    await V.click(page, '.library-sync');
    await page.waitForSelector('.library-sync-dialog');
    await L.sleep(500);
    await V.click(page, await V.byText(page, '.library-sync-dialog .segmented button', 'Medium'));
    await V.click(page, await V.byText(page, '.library-sync-dialog button', 'Save to library'));
    await page.waitForSelector('.library-sync.in-step', { timeout: 15000 });
    await L.sleep(1000);
  }, 500);

  await step('library-outro', async () => {
    await V.click(page, 'header a[href="/admin"]');
    await V.click(page, await tab('Questions'));
    await V.click(page, await V.byText(page, '.library-option', 'Added here'));
  }, 2000);

  await video.finish('demo-library', { posterAt: 4 });
  console.log('ERRORS', ctx.errors.length ? ctx.errors.join('\n') : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
