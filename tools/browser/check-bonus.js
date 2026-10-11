// A bonus question: typed on the phone with the answer, shown and judged on the big screen.
//   tools/browser/run.sh check-bonus.js
const L = require('./lib.js');

(async () => {
  const bonus = { text: 'Who sings it?', accepted: ['Freddie Mercury'], percent: 50 };
  const ctx = await L.setup({ questions: [{ ...L.choice('Which band?', 0, ['Queen', 'ABBA']), bonus }] }, { points: 'Flat' });
  const ed = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1280, 900);
  await ed.waitForSelector('.bonus-edit');
  console.log('editor:', await ed.$eval('.bonus-edit', (f) => f.innerText.replace(/\s+/g, ' ')));
  await ed.$eval('.bonus-edit', (f) => f.scrollIntoView());
  await ed.screenshot({ path: 'out/bonus-editor.png' });

  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  const host = await L.openHost(ctx, code);
  const owls = await L.openPhone(ctx, code, 'Owls');
  const cats = await L.openPhone(ctx, code, 'Cats');
  await host.keyboard.press('ArrowRight');
  await owls.waitForSelector('.bonus-field input');
  await L.sleep(800);
  console.log('big screen:', (await L.text(host)).slice(0, 200));
  console.log('bonus line:', await host.$eval('.bonus-line', (p) => p.innerText).catch(() => 'MISSING'));
  await host.screenshot({ path: 'out/bonus-question.png' });
  // Tapping an answer no longer sends it: the bonus comes first.
  await L.clickText(owls, 'button.phone-answer', 'Queen');
  await L.sleep(500);
  console.log('locked after a tap:', await owls.evaluate(() => document.body.innerText.includes('locked in')));
  await owls.type('.bonus-field input', 'freddie mercury');
  await owls.screenshot({ path: 'out/bonus-phone.png' });
  await L.clickText(owls, 'button', 'Submit');
  await L.clickText(cats, 'button.phone-answer', 'ABBA');
  await cats.type('.bonus-field input', 'Freddy');
  await L.clickText(cats, 'button', 'Submit');
  await L.sleep(1500);
  console.log('reveal:', await host.$eval('.bonus-reveal', (s) => s.innerText.replace(/\s+/g, ' ')));
  console.log('standings:', await host.$eval('.standings', (s) => s.innerText.replace(/\s+/g, ' ')));
  await host.screenshot({ path: 'out/bonus-reveal.png' });
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
