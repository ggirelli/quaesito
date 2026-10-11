// Shuffled answers: "Shuffle the answers" when starting a game reorders the big screen for that
// game only, and each phone lists the answers in its own order with the big screen's colours.
//   tools/browser/run.sh check-shuffle.js
const L = require('./lib.js');

(async () => {
  const title = `Shuffle check ${Date.now()}`;
  const answers = ['A', 'B', 'C', 'D', 'E', 'F'];
  const ctx = await L.setup({ title, questions: [L.choice('Pick A', 0, answers)] });
  const admin = await L.openAdmin(ctx, '/admin');
  await admin.waitForFunction((t) => document.body.innerText.includes(t), {}, title);
  await admin.evaluate((t) => {
    const start = [...document.querySelectorAll('button[title="Start a live game"]')]
      .find((b) => b.closest('li, .card, section')?.innerText.includes(t));
    start.click();
  }, title);
  await admin.waitForSelector('.dialog input[type=checkbox], dialog input[type=checkbox]');
  await admin.click('.check-field input');
  await admin.screenshot({ path: 'out/shuffle-start.png' });
  await L.clickText(admin, 'button[type=submit]', 'Start');
  await admin.waitForFunction(() => location.pathname.startsWith('/host/'), { timeout: 30000 });
  const code = await admin.evaluate(() => location.pathname.split('/').pop());

  const host = await L.openHost(ctx, code);
  const phones = [];
  for (const name of ['Owls', 'Cats', 'Pigs', 'Bats']) phones.push(await L.openPhone(ctx, code, name));
  await host.keyboard.press('ArrowRight');
  await host.waitForSelector('.answer-tile');
  for (const p of phones) await p.waitForSelector('button.phone-answer');
  await L.sleep(500);

  // The big screen: each answer's text and colour class.
  const tiles = await host.$$eval('.answer-tile', (ts) => ts.map((t) => [t.innerText.trim().split('\n').pop().trim(), t.className.match(/\ba\d\b/)[0]]));
  const screen = Object.fromEntries(tiles);
  const screenOrder = tiles.map(([t]) => t).join('');
  console.log('big screen:', screenOrder, screenOrder === 'ABCDEF' ? '(saved order: try again, 1 in 720)' : '(shuffled)');
  const saved = await L.api('/api/admin/quiz/get', { token: ctx.token, id: ctx.id });
  const savedOrder = saved.questions[0].answers.map((a) => a.text).join('');
  console.log('saved quiz:', savedOrder, savedOrder === 'ABCDEF' ? 'ok' : 'WRONG');

  let ok = savedOrder === 'ABCDEF';
  const orders = [];
  for (const p of phones) {
    const buttons = await p.$$eval('button.phone-answer', (bs) => bs.map((b) => [b.getAttribute('aria-label'), b.className.match(/\ba\d\b/)[0]]));
    orders.push(buttons.map(([t]) => t).join(''));
    const colours = buttons.every(([t, c]) => screen[t] === c);
    ok = ok && colours;
    console.log('phone:', orders.at(-1), colours ? 'colours match' : 'COLOURS WRONG');
  }
  const distinct = new Set(orders).size > 1;
  console.log('phones differ:', distinct ? 'ok' : 'WRONG');
  ok = ok && distinct;

  await phones[0].screenshot({ path: 'out/shuffle-phone.png' });
  // Tapping A scores, wherever it is listed.
  await L.clickText(phones[0], 'button.phone-answer', 'A');
  await L.clickText(phones[1], 'button.phone-answer', 'B');
  await host.keyboard.press('ArrowRight');
  await host.waitForSelector('.standings');
  const standings = await host.$eval('.standings', (s) => s.innerText.replace(/\s+/g, ' '));
  const owlsFirst = /^1\s.*Owls/.test(standings);
  console.log('standings:', standings, owlsFirst ? 'ok' : 'WRONG');
  ok = ok && owlsFirst;
  await host.screenshot({ path: 'out/shuffle-host.png' });

  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
  if (!ok || ctx.errors.length) process.exit(1);
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
