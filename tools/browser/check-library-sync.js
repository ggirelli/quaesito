// A quiz question saved to the library stays linked to it: the editor says which side changed
// since, and updates the library from the quiz or the quiz from the library once confirmed.
//   tools/browser/run.sh check-library-sync.js
const L = require('./lib.js');

let failed = 0;
function expect(what, ok) {
  console.log(ok ? 'ok  ' : 'FAIL', what);
  if (!ok) failed += 1;
}

const button = (page) => page.$eval('#question-1 .library-sync', (b) => b.getAttribute('aria-label'));
const waitButton = (page, label) => page.waitForFunction(
  (label) => document.querySelector('#question-1 .library-sync')?.getAttribute('aria-label').includes(label),
  { timeout: 10000 },
  label,
).then(() => true, () => false);

(async () => {
  const question = Object.assign(L.choice('Which river flows through Quaesitopolis?', 0, ['The Sync', 'The Drift']),
    { category: 'Geography', topic: 'Capitals' });
  const ctx = await L.setup({ questions: [question, L.choice('Unfiled?')] });

  const ed = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1280, 900);
  await ed.waitForSelector('#question-1 .library-sync');
  expect(`unlinked: ${await button(ed)}`, (await button(ed)).includes('Save to library'));
  const unfiled = await ed.$eval('#question-2 .library-sync', (b) => b.disabled);
  expect('a question without a topic cannot be saved', unfiled);

  await ed.click('#question-1 .library-sync');
  await ed.waitForSelector('.library-sync-dialog');
  await L.clickText(ed, '.library-sync-dialog .segmented button', 'Medium');
  await ed.screenshot({ path: 'out/library-sync-save.png' });
  await L.clickText(ed, '.library-sync-dialog button.primary', 'Save to library');
  expect('saved and linked', await waitButton(ed, 'In the library'));

  // Changed in the quiz: the library can take it, once confirmed.
  await L.sleep(1500); // autosave
  const text = await ed.$('#question-1 textarea.question-text');
  await text.click();
  await ed.keyboard.down('Control');
  await ed.keyboard.press('a');
  await ed.keyboard.up('Control');
  await text.type('Which river flows through Quaesitopolis today?');
  // The time limit is the quiz's own: no change for the library.
  expect('changed here', await waitButton(ed, 'Update library'));
  await (await ed.$('#question-1 .actions')).screenshot({ path: 'out/library-sync-card.png' });
  await ed.click('#question-1 .library-sync');
  await ed.waitForSelector('.library-versions');
  await ed.waitForFunction(() => document.querySelector('.library-sync-dialog').innerText.includes('No other quiz'));
  const dialog = await ed.$eval('.library-sync-dialog', (d) => d.innerText.replace(/\n+/g, ' | '));
  expect(`the differences are named: ${dialog}`, dialog.includes('Differs in: Question'));
  await ed.screenshot({ path: 'out/library-sync-push.png' });
  await L.clickText(ed, '.library-sync-dialog button', 'Cancel');
  expect('cancelling changes nothing', (await button(ed)).includes('Update library'));
  await ed.click('#question-1 .library-sync');
  await ed.waitForSelector('.library-versions');
  await L.clickText(ed, '.library-sync-dialog button.primary', 'Update library');
  expect('pushed', await waitButton(ed, 'In the library'));
  await L.sleep(1500); // autosave

  // Another quiz with the question; then the library changes.
  const saved = await L.api('/api/admin/quiz/get', { token: ctx.token, id: ctx.id });
  const link = saved.questions[0].library;
  expect(`the quiz keeps the link: ${JSON.stringify(link)}`, !!link && link.id.length === 32);
  const other = await L.api('/api/admin/quiz/create', { token: ctx.token, title: 'Other' });
  await L.api('/api/admin/quiz/save', { token: ctx.token, quiz: Object.assign({}, saved, { id: other, title: 'Other' }) });
  const library = await L.api('/api/admin/library', { token: ctx.token });
  const entry = library.find((e) => e.id === link.id);
  expect(`added to the library: ${entry && entry.category} · ${entry && entry.topic}`,
    !!entry && entry.added && entry.difficulty === 2 && entry.topic === 'Capitals');
  entry.question.passage = 'It never stops.';
  await L.api('/api/admin/library/edit', { token: ctx.token, id: entry.id, edit: { difficulty: 2, question: entry.question } });

  await ed.reload({ waitUntil: 'networkidle0' });
  expect('changed in the library', await waitButton(ed, 'Update from library'));
  await ed.click('#question-1 .library-sync');
  await ed.waitForSelector('.library-versions');
  await L.clickText(ed, '.library-sync-dialog button', 'Update from library');
  expect('pulled', await waitButton(ed, 'In the library'));
  const passage = await ed.$eval('#question-1', (c) => c.innerText.includes('It never stops.')
    || [...c.querySelectorAll('textarea')].some((t) => t.value === 'It never stops.'));
  expect('the library version is in the quiz', passage);

  // Changed on both sides; then saved as a new library question.
  entry.question.passage = 'It flows north.';
  await L.api('/api/admin/library/edit', { token: ctx.token, id: entry.id, edit: { difficulty: 2, question: entry.question } });
  await L.sleep(1500);
  await ed.reload({ waitUntil: 'networkidle0' });
  await ed.waitForSelector('#question-1 .library-sync');
  const field = await ed.$('#question-1 textarea.question-text');
  await field.type('!');
  expect('changed in both', await waitButton(ed, 'Changed in both'));
  await ed.click('#question-1 .library-sync');
  await ed.waitForSelector('.library-versions');
  const counted = await ed.waitForFunction(
    () => document.querySelector('.library-sync-dialog').innerText.includes('1 other quiz uses it'),
  ).then(() => true, () => false);
  expect('the other quiz is counted', counted);
  await ed.screenshot({ path: 'out/library-sync-both.png' });
  const addedBefore = (await L.api('/api/admin/library', { token: ctx.token })).filter((e) => e.added).length;
  await L.clickText(ed, '.library-sync-dialog button', 'Save as new');
  await ed.waitForFunction(() => document.querySelector('.library-sync-dialog .segmented'));
  await L.clickText(ed, '.library-sync-dialog button.primary', 'Save to library');
  expect('saved as new', await waitButton(ed, 'In the library'));
  await L.sleep(1500);
  const after = await L.api('/api/admin/library', { token: ctx.token });
  const newLink = (await L.api('/api/admin/quiz/get', { token: ctx.token, id: ctx.id })).questions[0].library;
  expect('one more added question, linked to the new one',
    after.filter((e) => e.added).length === addedBefore + 1 && newLink.id !== link.id);

  // Unlinking keeps the question in the quiz only.
  await ed.click('#question-1 .library-sync');
  await ed.waitForSelector('.library-sync-dialog');
  await L.clickText(ed, '.library-sync-dialog button', 'Unlink');
  expect('unlinked', await waitButton(ed, 'Save to library'));

  expect(`no page errors: ${ctx.errors.join('; ')}`, ctx.errors.length === 0);
  await ctx.browser.close();
  process.exit(failed ? 1 : 0);
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
