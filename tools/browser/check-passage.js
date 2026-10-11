// Text shown with a question (lyrics…): typed in the editor, shown on the big screen and phones.
//   tools/browser/run.sh check-passage.js
const L = require('./lib.js');

(async () => {
  const passage = "Is this the real life?\nIs this just fantasy?\nCaught in a landslide,\nno escape from reality";
  const ctx = await L.setup({ questions: [{ ...L.choice('Title and artist?', 0, ['Bohemian Rhapsody', 'Imagine']), passage }] });
  const ed = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1280, 900);
  await ed.waitForSelector('.question-passage-input');
  console.log('editor passage:', JSON.stringify(await ed.$eval('.question-passage-input', (t) => t.value)));
  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  const host = await L.openHost(ctx, code);
  const owls = await L.openPhone(ctx, code, 'Owls');
  await host.keyboard.press('ArrowRight');
  await host.waitForSelector('.question-passage');
  await L.sleep(800);
  console.log('big screen lines:', await host.$eval('.question-passage', (p) => p.innerText.split('\n').length));
  console.log('phone:', JSON.stringify(await owls.$eval('.phone-question', (p) => p.innerText)));
  await host.screenshot({ path: 'out/passage-host.png' });
  await owls.screenshot({ path: 'out/passage-phone.png' });
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
