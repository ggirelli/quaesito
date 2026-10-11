// The quiz list: a new quiz from the pop-up, search and sorting.
//   tools/browser/run.sh check-quiz-list.js
const L = require('./lib.js');

(async () => {
  const ctx = await L.setup({ title: 'Zoology', questions: [L.choice('Q', 0), L.choice('R', 0)] });
  await L.api('/api/admin/quiz/create', { token: ctx.token, title: 'Animé night' });
  await L.sleep(1100);
  await L.api('/api/admin/quiz/create', { token: ctx.token, title: 'Movie night' });
  const admin = await L.openAdmin(ctx, '/admin');
  const titles = () => admin.$$eval('.list-row .strong', (a) => a.map((x) => x.innerText));
  console.log('last edited first:', await titles());
  await admin.select('.quiz-filters select', 'name');
  console.log('by name:', await titles());
  await admin.select('.quiz-filters select', 'questions');
  console.log('most questions:', await titles());
  await admin.type('.search input', 'anime');
  await L.sleep(300);
  console.log('search "anime":', await titles());
  await admin.$eval('.search input', (i) => { i.value = ''; });
  await admin.type('.search input', 'zz');
  await L.sleep(300);
  console.log('search "zz":', await titles(), await admin.evaluate(() => document.body.innerText.includes('No quiz matches')));
  await admin.screenshot({ path: 'out/quiz-list.png' });
  await L.clickText(admin, 'button', 'New quiz');
  await admin.waitForSelector('.dialog input');
  await admin.keyboard.type('Pub quiz');
  await admin.screenshot({ path: 'out/quiz-new.png' });
  await admin.keyboard.press('Enter');
  await admin.waitForFunction(() => location.pathname.startsWith('/admin/quiz/'), { timeout: 10000 });
  console.log('editor title:', await admin.$eval('input', (i) => i.value));
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
