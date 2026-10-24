// Checks that moving a question in the editor keeps it in view and focused.
//   tools/browser/run.sh check-move.js
const L = require('./lib.js');

(async () => {
  const questions = Array.from({ length: 10 }, (_, i) => L.choice(`Question ${i + 1}`));
  const ctx = await L.setup({ questions });
  const p = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1280, 800);
  // Positions count from 0, but cards are named after their question number: the first is #question-1.
  const id = (n) => `question-${n + 1}`;
  await p.waitForSelector(`#${id(9)}`);
  // Where each card is on screen, which card holds the focus and which button.
  const state = (n) => p.evaluate((id) => {
    const card = document.getElementById(id);
    const focus = document.activeElement;
    return {
      top: Math.round(card.getBoundingClientRect().top),
      text: card.querySelector('.question-text')?.value,
      focus: `${focus.closest('section')?.id} ${focus.title}`,
    };
  }, id(n));
  const move = async (from, to, title) => {
    // Centre the button, not the card: a card taller than the window would leave its buttons under the
    // sticky header, and the click would land on the header's link instead.
    const button = `#${id(from)} button[title="${title}"]`;
    await p.evaluate((b) => document.querySelector(b).scrollIntoView({ block: 'center' }), button);
    await L.sleep(300);
    const before = await state(from);
    await p.click(button);
    await L.sleep(500);
    const after = await state(to);
    const ok = after.text === before.text && after.focus.startsWith(`${id(to)} `)
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
