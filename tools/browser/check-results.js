// A results slide halfway: the answers so far, then the standings, on the big screen and phones.
//   tools/browser/run.sh check-results.js
const L = require('./lib.js');

(async () => {
  const ctx = await L.setup({
    questions: [
      L.choice('First half, one', 0), L.choice('First half, two', 1),
      { text: '', kind: { Slide: { title: 'Half time!', results: true } }, answers: [] },
      L.choice('Second half', 0),
    ],
  }, { reveal: 'AtEnd' });
  const ed = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1280, 900);
  await ed.waitForSelector('#question-2');
  console.log('editor results slide:', await ed.$eval('#question-2', (c) => c.innerText.replace(/\s+/g, ' ')));
  console.log('add button shown:', await ed.evaluate(() => [...document.querySelectorAll('button')].some((b) => b.innerText.includes('Add results slide'))));

  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  const host = await L.openHost(ctx, code);
  const owls = await L.openPhone(ctx, code, 'Owls');
  const cats = await L.openPhone(ctx, code, 'Cats');
  const next = async () => { await host.keyboard.press('ArrowRight'); await L.sleep(700); };
  const button = () => host.$eval('.host-controls .button.primary.big', (b) => b.innerText);
  for (const right of [0, 1]) {
    await next(); // open the question
    await L.clickText(owls, 'button.phone-answer', right ? 'also' : 'right');
    await L.clickText(cats, 'button.phone-answer', right ? 'right' : 'also');
    await L.sleep(1000);
  }
  console.log('after the first half, next is:', await button());
  await next();
  console.log('review:', (await L.text(host)).slice(0, 80), '| next:', await button());
  await next();
  console.log('next after the last review:', await button());
  await next();
  console.log('big screen:', (await host.$eval('.results-slide', (s) => s.innerText.replace(/\s+/g, ' '))));
  console.log('phone:', (await L.text(owls)).slice(0, 120));
  await host.screenshot({ path: 'out/results-slide.png' });
  console.log('next after the standings:', await button());
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
