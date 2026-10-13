// Pairs questions: made in the editor (one item a picture only), matched on phones with the right
// column shuffled, partly matched answers scoring their share.
//   tools/browser/run.sh check-pairs.js
const path = require('path');
const L = require('./lib.js');

const picture = path.resolve(__dirname, '../../assets/brand/icon-192.png');

/** Taps the item of a phone's pairs column (0 left, 1 right) whose text has `t`, or its picture. */
const tap = (p, column, t) => p.evaluate((column, t) => {
  const items = [...document.querySelectorAll('.pairs-pad .pair-column')[column].querySelectorAll('button')];
  const b = t ? items.find((b) => b.innerText.includes(t)) : items.find((b) => b.querySelector('img'));
  if (!b) throw new Error('nothing matches ' + t);
  b.click();
}, column, t);

(async () => {
  const ctx = await L.setup({ questions: [L.choice('Placeholder')] });
  const ed = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1280, 900);
  await ed.waitForSelector('.question-text');
  await ed.click('.question-text');
  await ed.keyboard.down('Control'); await ed.keyboard.press('a'); await ed.keyboard.up('Control');
  await ed.keyboard.type('Match each country with its capital');
  await L.clickText(ed, '.segmented button', 'Pairs');
  await ed.waitForSelector('.pair-edit');
  const pairs = [['France', 'Paris'], ['Italy', ''], ['Spain', 'Madrid']];
  for (const [i, [left, right]] of pairs.entries()) {
    await ed.type(`input[aria-label="Left item ${i + 1}"]`, left);
    if (right) await ed.type(`input[aria-label="Right item ${i + 1}"]`, right);
  }
  // Italy's partner is a picture only.
  const uploads = await ed.$$('.pair-edit input[accept="image/*"]');
  await uploads[3].uploadFile(picture);
  await ed.waitForFunction(() => document.querySelectorAll('.pair-edit .answer-thumb').length === 1, { timeout: 15000 });
  await L.sleep(1500);
  await ed.screenshot({ path: 'out/pairs-editor.png', fullPage: true });
  const saved = await L.api('/api/admin/quiz/get', { token: ctx.token, id: ctx.id });
  const kind = saved.questions[0].kind;
  let ok = !!kind.Pairs && kind.Pairs.pairs.length === 3 && !!kind.Pairs.pairs[1].right.image;
  console.log('saved pairs:', JSON.stringify(kind), ok ? 'ok' : 'WRONG');
  const issues = (await L.api('/api/admin/quizzes', { token: ctx.token }))
    .find((q) => q.id === ctx.id).issues;
  console.log('issues:', issues.length ? issues : 'none');
  ok = ok && !issues.length;

  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  const host = await L.openHost(ctx, code);
  const owls = await L.openPhone(ctx, code, 'Owls');
  const cats = await L.openPhone(ctx, code, 'Cats');
  await host.keyboard.press('ArrowRight');
  await host.waitForSelector('.pair-board');
  for (const p of [owls, cats]) await p.waitForSelector('.pairs-pad');
  await L.sleep(800);
  const right = await host.$$eval('.pair-board .pair-column:last-child .pair-card', (cs) => cs.map((c) => c.innerText.replace(/\s+/g, ' ').trim()));
  console.log('big screen right column:', right.join(' / '));
  await host.screenshot({ path: 'out/pairs-host.png' });

  // Owls: every pair, starting from either column; one changed mind on the way.
  await tap(owls, 0, 'France'); await tap(owls, 1, 'Madrid');
  await tap(owls, 0, 'France'); await tap(owls, 1, 'Paris');
  await tap(owls, 1, null); await tap(owls, 0, 'Italy');
  await tap(owls, 0, 'Spain'); await tap(owls, 1, 'Madrid');
  await L.sleep(300);
  const marks = await owls.$$eval('.pairs-pad .pair-column:first-child .pair-mark', (ms) => ms.map((m) => m.innerText.trim()));
  console.log('owls marks:', marks.join(' '));
  await owls.screenshot({ path: 'out/pairs-phone.png', fullPage: true });
  await L.clickText(owls, 'button', 'Submit');
  // Cats: one right pair, one wrong, one left out.
  await tap(cats, 0, 'France'); await tap(cats, 1, 'Paris');
  await tap(cats, 0, 'Italy'); await tap(cats, 1, 'Madrid');
  await L.clickText(cats, 'button', 'Submit');
  // The question closes once every team has answered.
  await host.waitForSelector('.pairs-reveal');
  await L.sleep(500);
  const reveal = await host.$eval('.pairs-reveal', (r) => r.innerText.replace(/\s+/g, ' '));
  console.log('reveal:', reveal);
  await host.screenshot({ path: 'out/pairs-reveal.png' });
  // Standings come with the reveal. Points depend on speed: Cats matched a third of the pairs.
  const standings = await host.$eval('.standings', (s) => s.innerText.replace(/\s+/g, ' '));
  const points = (team) => Number(standings.match(new RegExp(`${team} \\+(\\d+)`))[1]);
  const scores = points('Owls') > 500 && Math.abs(points('Cats') / points('Owls') - 1 / 3) < 0.05;
  console.log('standings:', standings, scores ? 'ok' : 'WRONG');
  ok = ok && scores;

  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
  if (!ok || ctx.errors.length) process.exit(1);
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
