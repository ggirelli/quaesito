// The question library: the editor adds library questions one by one and at random, and a new
// quiz can be filled with random questions from ticked categories.
//   tools/browser/run.sh check-library.js
const L = require('./lib.js');

(async () => {
  const title = `Library check ${Date.now()}`;
  const ctx = await L.setup({ title, questions: [L.choice('My own question')] });
  let ok = true;
  const expect = (what, good) => {
    console.log(what, good ? 'ok' : 'WRONG');
    ok = ok && good;
  };
  // Replaces what a field holds (selecting a number field's text needs the keyboard).
  const retype = async (page, field, value) => {
    await field.focus();
    await page.keyboard.down('Control');
    await page.keyboard.press('a');
    await page.keyboard.up('Control');
    await field.type(value);
  };
  const saved = async (id) => {
    await L.sleep(2000); // autosave
    return L.api('/api/admin/quiz/get', { token: ctx.token, id });
  };

  // The editor: browse, filter, add one, then add three at random.
  const editor = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`);
  await editor.waitForSelector('button.add-library');
  await editor.click('button.add-library');
  await editor.waitForSelector('.dialog .list-row');
  await editor.select('select[aria-label="Category"]', 'Science');
  await editor.type('.dialog input[type=search]', 'planet');
  await L.sleep(300);
  const rows = await editor.$$eval('.dialog .list-row', (rs) => rs.map((r) => r.innerText));
  expect(`search: ${rows.length} Science questions about planets`,
    rows.length > 0 && rows.every((r) => r.includes('Science') && /planet/i.test(r)));
  await editor.screenshot({ path: 'out/library-editor.png' });
  const first = rows[0].split('\n')[0];
  await L.clickText(editor, '.dialog .list-row button', 'Add');
  await editor.waitForFunction(() => document.querySelector('.dialog .list-row').innerText.includes('In the quiz'));
  await editor.$eval('.dialog input[type=search]', (i) => i.select());
  await editor.keyboard.press('Backspace');
  const count = await editor.$('.dialog input.short-number');
  await retype(editor, count, '3');
  await L.clickText(editor, '.dialog button', 'Add at random');
  await L.clickText(editor, '.dialog button', 'Close');
  let quiz = await saved(ctx.id);
  const texts = quiz.questions.map((q) => q.text);
  expect(`editor: ${texts.length} questions, the picked one second`, texts.length === 5 && texts[1] === first);
  const issues = await L.api('/api/admin/quiz/save', { token: ctx.token, quiz });
  expect('editor: playable', issues.length === 0);

  // A new quiz from two categories: 3 from the first, the default 5 from the second, with title slides.
  const admin = await L.openAdmin(ctx, '/admin');
  await L.clickText(admin, 'button', 'New quiz');
  await admin.waitForSelector('.dialog input[placeholder]');
  await admin.type('.dialog input[placeholder]', `${title} random`);
  await L.clickText(admin, '.dialog .check-field', 'Fill it with random questions');
  await admin.waitForSelector('.random-categories .list-row');
  const categories = await admin.$$('.random-categories .list-row');
  await (await categories[0].$('input[type=checkbox]')).click();
  await (await categories[1].$('input[type=checkbox]')).click();
  const firstCount = await categories[0].$('input.short-number');
  await retype(admin, firstCount, '3');
  await L.clickText(admin, '.dialog .check-field', 'title slide');
  await L.sleep(300);
  await admin.screenshot({ path: 'out/library-random.png' });
  const summary = await L.text(admin);
  expect('random: 8 questions announced', /(^|\| )8 questions, picked at random/.test(summary));
  await admin.click('.dialog button[type=submit]');
  await admin.waitForFunction(() => location.pathname.startsWith('/admin/quiz/'), { timeout: 30000 });
  const id = await admin.evaluate(() => location.pathname.split('/').pop());
  quiz = await L.api('/api/admin/quiz/get', { token: ctx.token, id });
  const slides = quiz.questions.filter((q) => q.kind && q.kind.Slide);
  expect(`random: ${quiz.questions.length} items, ${slides.length} slides`,
    quiz.questions.length === 10 && slides.length === 2 && quiz.questions[0].kind.Slide && quiz.questions[4].kind.Slide);
  expect('random: playable', (await L.api('/api/admin/quiz/save', { token: ctx.token, quiz })).length === 0);
  await admin.waitForSelector('.editor-list');
  await admin.screenshot({ path: 'out/library-random-editor.png' });

  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
  if (!ok || ctx.errors.length) process.exit(1);
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
