// A question's category and topic: picked in the editor among the library's (topics added on the
// Questions tab included) or kept from the library, and shown on the big screen next to the
// question number.
//   tools/browser/run.sh check-subject.js
const L = require('./lib.js');

let failed = 0;
function expect(what, ok) {
  console.log(ok ? 'ok  ' : 'FAIL', what);
  if (!ok) failed += 1;
}

/** Adds a topic with the Questions tab's form. */
async function addTopic(page, category, topic) {
  const [c, t] = await page.$$('.library-add-topic input');
  await c.evaluate((i) => i.select());
  await page.keyboard.press('Backspace');
  await c.type(category);
  await t.type(topic);
  await L.clickText(page, '.library-add-topic button', 'Add topic');
  await L.sleep(800);
}

(async () => {
  const ctx = await L.setup({ questions: [L.choice('Who won the 1972 world chess title?', 0, ['Fischer', 'Spassky'])] });

  const tab = await L.openAdmin(ctx, '/admin/questions', 1280, 900);
  await tab.waitForSelector('.library-add-topic input');
  await addTopic(tab, 'Board games', 'Chess');
  await addTopic(tab, 'board games', 'Typo');
  await addTopic(tab, 'Geography', 'Capitals');
  const refused = await tab.$eval('.library-add-topic', (f) => f.innerText);
  expect(`a library topic is refused: ${refused.replace(/\n/g, ' ')}`, refused.includes('already has the topic'));
  await L.clickText(tab, '.library-category', 'Board games');
  await tab.waitForSelector('.library-topic-row');
  let topics = await tab.$$eval('.library-topic-row', (rs) => rs.map((r) => r.innerText.replace(/\n/g, ' ')));
  expect(`new category, one spelling: ${topics.join(' | ')}`, topics.length === 2);
  await tab.screenshot({ path: 'out/subject-library.png' });
  await tab.click('.library-topic-row .remove-topic[aria-label="Remove Typo"]');
  await L.sleep(800);
  topics = await tab.$$eval('.library-topic-row', (rs) => rs.map((r) => r.innerText.replace(/\n/g, ' ')));
  expect(`removed: ${topics.join(' | ')}`, topics.length === 1 && topics[0].startsWith('Chess'));
  await tab.close();

  const ed = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1280, 900);
  await ed.waitForSelector('.subject-select');
  await L.sleep(1000); // the library's categories load
  const options = await ed.$$eval('.subject-select', (ss) => [...ss[0].options].map((o) => o.value));
  expect(`categories offered: ${options.length}`, options.includes('Board games') && options.includes('Geography') && options[0] === '');
  await ed.select('.subject-select', 'Board games');
  await L.sleep(300);
  const [, topicSelect] = await ed.$$('#question-0 .subject-select');
  const topicOptions = await topicSelect.evaluate((s) => [...s.options].map((o) => o.value));
  expect(`topics offered: ${topicOptions.join(', ')}`, topicOptions.join() === ',Chess');
  await topicSelect.select('Chess');
  await L.clickText(ed, 'button', 'From the library');
  await ed.waitForSelector('.dialog .library-list .list-row button');
  const meta = await ed.$eval('.dialog .library-list .list-row .library-meta', (m) => m.innerText);
  await L.clickText(ed, '.dialog .library-list .list-row button', 'Add');
  await ed.waitForFunction(() => document.querySelector('.dialog .library-list .list-row').innerText.includes('In the quiz'));
  await L.clickText(ed, '.dialog button', 'Close');
  await L.sleep(2000); // autosave
  const values = await ed.$$eval('.subject-select', (ss) => ss.map((s) => s.value));
  await ed.screenshot({ path: 'out/subject-editor.png' });
  const saved = await L.api('/api/admin/quiz/get', { token: ctx.token, id: ctx.id });
  const [picked, library] = saved.questions;
  expect(`picked: ${picked.category} / ${picked.topic}`, picked.category === 'Board games' && picked.topic === 'Chess');
  expect(`library: ${library.category} / ${library.topic} (row: ${meta.replace(/\n/g, ' ')})`,
    !!library.category && !!library.topic && meta.includes(library.category) && meta.includes(library.topic));
  expect(`editor selects: ${values.join(' / ')}`, values.length === 4 && values[2] === library.category && values[3] === library.topic);
  await ed.close();

  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  const host = await L.openHost(ctx, code);
  await L.openPhone(ctx, code, 'Owls');
  await host.keyboard.press('ArrowRight');
  await host.waitForSelector('.question-head');
  const shown = await host.$eval('.question-subject', (s) => s.innerText.replace(/\s+/g, ' ')).catch(() => 'none');
  expect(`big screen: ${shown}`, shown === 'Board games · Chess');
  await host.screenshot({ path: 'out/subject-host.png' });
  // Refusing a topic the library has answers 500, as server functions do for errors.
  const errors = ctx.errors.filter((e) => !(e.startsWith('/admin/questions') && e.includes('status of 500')));
  console.log('ERRORS', errors.length ? errors : 'none');
  await ctx.browser.close();
  process.exit(failed || errors.length ? 1 : 0);
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
