// A question's category and topic: typed in the editor or kept from the library, and shown on the
// big screen next to the question number.
//   tools/browser/run.sh check-subject.js
const L = require('./lib.js');

let failed = 0;
function expect(what, ok) {
  console.log(ok ? 'ok  ' : 'FAIL', what);
  if (!ok) failed += 1;
}

(async () => {
  const ctx = await L.setup({ questions: [L.choice('Capital of France?', 0, ['Paris', 'Lyon'])] });
  const ed = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1280, 900);
  await ed.waitForSelector('.question-subject-edit input');
  const [category, topic] = await ed.$$('.question-subject-edit input');
  await category.type('Geography');
  await topic.type('Capitals');
  await L.clickText(ed, 'button', 'From the library');
  await ed.waitForSelector('.dialog .library-list .list-row button');
  const meta = await ed.$eval('.dialog .library-list .list-row .library-meta', (m) => m.innerText);
  await L.clickText(ed, '.dialog .library-list .list-row button', 'Add');
  await ed.waitForFunction(() => document.querySelector('.dialog .library-list .list-row').innerText.includes('In the quiz'));
  await L.clickText(ed, '.dialog button', 'Close');
  await L.sleep(2000); // autosave
  const inputs = await ed.$$eval('.question-subject-edit input', (is) => is.map((i) => i.value));
  await ed.$eval('#question-1', (q) => q.scrollIntoView());
  await ed.screenshot({ path: 'out/subject-editor.png' });
  const saved = await L.api('/api/admin/quiz/get', { token: ctx.token, id: ctx.id });
  const [typed, library] = saved.questions;
  expect(`typed: ${typed.category} / ${typed.topic}`, typed.category === 'Geography' && typed.topic === 'Capitals');
  expect(`library: ${library.category} / ${library.topic} (row: ${meta.replace(/\n/g, ' ')})`,
    !!library.category && !!library.topic && meta.includes(library.category) && meta.includes(library.topic));
  expect(`editor fields: ${inputs.join(' / ')}`, inputs.length === 4 && inputs[2] === library.category);
  await ed.close();

  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  const host = await L.openHost(ctx, code);
  await L.openPhone(ctx, code, 'Owls');
  await host.keyboard.press('ArrowRight');
  await host.waitForSelector('.question-head');
  const shown = await host.$eval('.question-subject', (s) => s.innerText).catch(() => 'none');
  expect(`big screen: ${shown}`, shown === 'Geography · Capitals');
  await host.screenshot({ path: 'out/subject-host.png' });
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
  process.exit(failed || ctx.errors.length ? 1 : 0);
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
