// Leaving a test: the big screen's "Back to admin" ends it and leaves the test page; so does
// "End test". The login page has Home and Help links.
//   tools/browser/run.sh check-test-exit.js
const L = require('./lib.js');

(async () => {
  const ctx = await L.setup({ questions: [L.choice('Only question', 0)] });
  const login = await (await ctx.browser.createBrowserContext()).newPage();
  await login.goto(`${L.B}/admin`, { waitUntil: 'networkidle0' });
  console.log('login page links:', await login.$$eval('.corner-links a', (a) => a.map((x) => x.getAttribute('aria-label'))));

  const start = async () => {
    const code = await L.api('/api/admin/test/start', { token: ctx.token, quiz_id: ctx.id });
    const p = await L.openAdmin(ctx, `/admin/test/${code}`, 1600, 900);
    await L.sleep(2500);
    return { p, code, frame: () => p.frames().find((f) => f.url().includes('/host/')) };
  };
  const live = async () => (await L.api('/api/admin/sessions', { token: ctx.token })).map((s) => s.code);

  // Play to the end on the framed big screen, then "Back to admin" there.
  const a = await start();
  for (let i = 0; i < 6; i += 1) {
    await a.frame().evaluate(() => document.querySelector('.host-controls .button.primary.big')?.click());
    await L.sleep(700);
  }
  await a.frame().evaluate(() => [...document.querySelectorAll('button')].find((b) => b.innerText === 'Back to admin').click());
  await L.sleep(2000);
  console.log('after Back to admin in the frame:', new URL(a.p.url()).pathname, '| test still live:', (await live()).includes(a.code));

  // "End test" from the test page, after the frame has moved around.
  const b = await start();
  await b.frame().evaluate(() => document.querySelector('.host-controls .button.primary.big')?.click());
  await L.sleep(700);
  await L.clickText(b.p, 'button', 'End test');
  await L.sleep(1500);
  console.log('after End test:', new URL(b.p.url()).pathname, '| test still live:', (await live()).includes(b.code));
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
