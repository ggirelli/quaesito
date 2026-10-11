// Checks that moving a question in the editor keeps it in view and focused.
//   tools/browser/run.sh check-move.js
const L = require('./lib.js');

(async () => {
  const questions = Array.from({ length: 10 }, (_, i) => L.choice(`Question ${i + 1}`));
  const ctx = await L.setup({ questions });
  const p = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1280, 800);
  await p.waitForSelector('#question-9');
  // Where each card is on screen, which card holds the focus and which button.
  const state = (n) => p.evaluate((n) => {
    const card = document.getElementById(`question-${n}`);
    const focus = document.activeElement;
    return {
      top: Math.round(card.getBoundingClientRect().top),
      text: card.querySelector('textarea, input')?.value,
      focus: `${focus.closest('section')?.id} ${focus.title}`,
    };
  }, n);
  const move = async (from, to, title) => {
    await p.evaluate((n) => document.getElementById(`question-${n}`).scrollIntoView({ block: 'center' }), from);
    await L.sleep(300);
    const before = await state(from);
    await p.click(`#question-${from} button[title="${title}"]`);
    await L.sleep(500);
    const after = await state(to);
    const ok = after.text === before.text && after.focus.startsWith(`question-${to}`)
      && (Math.abs(after.top - before.top) <= 2 || (after.top >= 0 && after.top < 800));
    console.log(`${title} ${from}→${to}:`, ok ? 'ok' : 'WRONG', JSON.stringify({ before, after }));
    return ok;
  };
  const results = [
    await move(4, 5, 'Move down'),
    await move(5, 4, 'Move up'),
    await move(4, 9, 'Move to the bottom'),
    await move(9, 0, 'Move to the top'),
  ];
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
  if (results.includes(false)) process.exit(1);
})().catch((e) => { console.error(e); process.exit(1); });
