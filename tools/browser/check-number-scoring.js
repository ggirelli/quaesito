// A number question scored by distance: set in the editor, points and a bonus on the big screen.
//   tools/browser/run.sh check-number-scoring.js
const L = require('./lib.js');

(async () => {
  const ctx = await L.setup({ questions: [{ text: 'How many keys on a piano?', kind: { Number: { answer: 88 } }, answers: [] }] }, { points: 'Flat' });
  const ed = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1280, 900);
  await ed.waitForSelector('input[aria-label="Range from"]');
  await L.clickText(ed, 'fieldset.choice button', 'Closer earns more');
  await L.sleep(300);
  const field = async (label, value) => {
    const handle = await ed.evaluateHandle((label) => [...document.querySelectorAll('label.field')]
      .find((l) => l.innerText.includes(label)).querySelector('input'), label);
    await handle.evaluate((i) => { i.value = ''; });
    await handle.type(value);
  };
  await field('Full points within', '2');
  await field('No points beyond', '22');
  await L.sleep(2000); // autosave
  let saved = await L.api('/api/admin/quiz/get', { token: ctx.token, id: ctx.id });
  console.log('saved:', JSON.stringify(saved.questions[0].kind));
  await ed.screenshot({ path: 'out/number-scoring-editor.png' });
  // Distances the wrong way round are flagged.
  await field('No points beyond', '1');
  await L.sleep(300);
  console.log('warning:', await ed.$$eval('.field-error', (ps) => ps.map((p) => p.innerText).join(' | ')));
  await field('No points beyond', '22');
  await L.sleep(2000);

  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  const host = await L.openHost(ctx, code);
  const owls = await L.openPhone(ctx, code, 'Owls');
  const cats = await L.openPhone(ctx, code, 'Cats');
  await host.keyboard.press('ArrowRight');
  console.log('prompt:', await host.waitForSelector('.open-prompt').then((p) => p.evaluate((p) => p.innerText)));
  for (const [phone, guess] of [[owls, '87'], [cats, '100']]) {
    const input = 'input[aria-label="Your guess"][type=text]';
    await phone.waitForSelector(input);
    await phone.type(input, guess);
    await L.clickText(phone, 'button', 'Submit');
  }
  await host.waitForSelector('.guesses .guess', { timeout: 15000 });
  const rows = () => host.$$eval('.guesses .guess', (ls) => ls.map((l) => l.innerText.replace(/\s+/g, ' ')));
  console.log('reveal:', await rows());
  // A bonus for the Cats.
  await host.$$eval('.guesses .guess', (ls) => ls[1].querySelector('button[title=Bonus]').click());
  await L.sleep(800);
  console.log('after bonus:', await rows());
  await host.screenshot({ path: 'out/number-scoring-host.png' });
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
