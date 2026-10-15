// What a question shows with its answer: added in the editor, hidden on the big screen until the
// answer is revealed (after the question, or in the end review).
//   tools/browser/run.sh check-explanation.js picture.png
const L = require('./lib.js');
const [picture] = process.argv.slice(2);

/** Plays the quiz with one phone and logs what the big screen shows before and with the answer. */
async function play(ctx, label) {
  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  const host = await L.openHost(ctx, code);
  const owls = await L.openPhone(ctx, code, 'Owls');
  await host.keyboard.press('ArrowRight');
  await owls.waitForSelector('button.phone-answer');
  await L.sleep(500);
  console.log(`${label} while open:`, await host.$('.explanation') ? 'SHOWN' : 'hidden');
  await L.clickText(owls, 'button.phone-answer', 'Saturn');
  await L.sleep(1500);
  console.log(`${label} closed:`, await host.$eval('.explanation', (a) => a.innerText).catch(() => 'hidden'));
  await host.screenshot({ path: `out/explanation-${label}-closed.png` });
  await host.keyboard.press('ArrowRight'); // the review, when revealing at the end
  await L.sleep(1500);
  const shown = await host.$eval('.explanation', (a) => ({
    text: a.innerText,
    image: !!a.querySelector('img.explanation-image'),
  })).catch(() => 'hidden');
  console.log(`${label} next:`, JSON.stringify(shown), '|', (await L.text(host)).slice(0, 80));
  await host.screenshot({ path: `out/explanation-${label}-next.png` });
}

(async () => {
  const ctx = await L.setup({ questions: [L.choice('Which planet has the most moons?', 1, ['Jupiter', 'Saturn'])] });
  const ed = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1280, 900);
  await ed.waitForSelector('.question');
  await L.sleep(1500); // upload limits load
  await L.clickText(ed, '.question button', 'Add something to show with the answer');
  await ed.waitForSelector('.question fieldset textarea');
  await ed.type('.question fieldset textarea', 'Saturn has 274 known moons.\nJupiter has 97.');
  await (await ed.$('.question fieldset input[type=file]')).uploadFile(picture);
  await ed.waitForSelector('.question fieldset img.thumb', { timeout: 30000 });
  await ed.$eval('.question fieldset', (f) => f.scrollIntoView());
  await L.sleep(2000); // autosave
  await ed.screenshot({ path: 'out/explanation-editor.png' });
  const saved = await L.api('/api/admin/quiz/get', { token: ctx.token, id: ctx.id }).catch((e) => ({ error: String(e) }));
  const q = saved.questions ? saved.questions[0] : saved;
  console.log('saved:', JSON.stringify({ explanation: q.explanation, image: !!q.explanation_image, error: q.error }));
  await ed.close();

  await play(ctx, 'after-each');
  saved.settings.reveal = 'AtEnd';
  await L.api('/api/admin/quiz/save', { token: ctx.token, quiz: saved });
  await play(ctx, 'at-end');
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
