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

/** Adds a topic with the Questions tab's pop-up, picking `look` ([icon, colour]) if given; returns
 * the pop-up's text if it stays open (refused). */
async function addTopic(page, category, topic, look) {
  await L.clickText(page, '.library-nav button', 'Add a topic');
  await page.waitForSelector('.add-topic-dialog input');
  const [c] = await page.$$('.add-topic-dialog input');
  await c.type(category);
  if (look) {
    await page.click(`.look-choice[aria-label="Icon ${look[0]}"]`);
    await page.click(`.look-choice[aria-label="Colour ${look[1]}"]`);
  }
  const [, t] = await page.$$('.add-topic-dialog input');
  await t.type(topic);
  await L.clickText(page, '.add-topic-dialog button', 'Add topic');
  await L.sleep(800);
  const open = await page.$eval('.add-topic-dialog', (d) => d.innerText.replace(/\n/g, ' ')).catch(() => null);
  if (open) await L.clickText(page, '.add-topic-dialog button', 'Close');
  return open;
}

(async () => {
  const ctx = await L.setup({ questions: [L.choice('Who won the 1972 world chess title?', 0, ['Fischer', 'Spassky'])] });

  const tab = await L.openAdmin(ctx, '/admin/questions', 1280, 900);
  await tab.waitForSelector('.library-nav .add-topic');
  // A new category: the library's icons and colours are greyed out.
  await L.clickText(tab, '.library-nav button', 'Add a topic');
  await tab.waitForSelector('.add-topic-dialog input');
  await tab.type('.add-topic-dialog input', 'Board games');
  const greyed = await tab.$eval('.look-choice[aria-label="Icon globe"]', (b) => b.disabled)
    && await tab.$eval('.look-choice[aria-label="Colour #0d9488"]', (b) => b.disabled)
    && !(await tab.$eval('.look-choice[aria-label="Icon grid"]', (b) => b.disabled));
  expect('taken icons and colours are greyed out', greyed);
  await L.clickText(tab, '.add-topic-dialog button', 'Close');
  expect('a new category is added', await addTopic(tab, 'Board games', 'Chess', ['grid', '#f43f5e']) === null);
  await addTopic(tab, 'board games', 'Typo');
  const refused = await addTopic(tab, 'Geography', 'Capitals');
  expect(`a library topic is refused: ${refused}`, !!refused && refused.includes('already has the topic'));
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
  const [, topicSelect] = await ed.$$('#question-1 .subject-select');
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
  const look = await host.$eval('.question-subject', (s) => ({ color: s.style.getPropertyValue('--category'), rects: s.querySelectorAll('svg rect').length }));
  expect(`its look: ${JSON.stringify(look)}`, look.color === '#f43f5e' && look.rects === 4);
  await host.screenshot({ path: 'out/subject-host.png' });
  // Refusing a topic the library has answers 500, as server functions do for errors.
  const errors = ctx.errors.filter((e) => !(e.startsWith('/admin/questions') && e.includes('status of 500')));
  console.log('ERRORS', errors.length ? errors : 'none');
  await ctx.browser.close();
  process.exit(failed || errors.length ? 1 : 0);
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
