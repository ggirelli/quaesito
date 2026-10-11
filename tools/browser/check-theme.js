// The light theme: the toggle switches it and other pages follow; screenshots of the main screens.
//   tools/browser/run.sh check-theme.js
const L = require('./lib.js');

(async () => {
  const ctx = await L.setup({
    questions: [
      { ...L.choice('Which planet has the most moons?', 1, ['Jupiter', 'Saturn', 'Uranus', 'Neptune']), passage: 'Hint: it has rings.' },
      { text: 'Typed', kind: { Text: { accepted: ['yes'] } }, answers: [] },
    ],
  });
  const browser = await ctx.browser.createBrowserContext();
  const page = async (path, w = 1280, h = 800, light = true) => {
    const p = await browser.newPage();
    ctx.watch(p, path);
    await p.setViewport({ width: w, height: h });
    await p.evaluateOnNewDocument((t, light) => {
      localStorage.setItem('quaesito.admin', t);
      if (light) localStorage.setItem('quaesito.theme', 'light');
    }, ctx.token, light);
    await p.goto(`${L.B}${path}`, { waitUntil: 'networkidle0' });
    await L.sleep(800);
    return p;
  };
  const theme = (p) => p.evaluate(() => document.documentElement.dataset.theme || 'dark');
  const shot = (p, name) => p.screenshot({ path: `out/theme-${name}.png` });

  const home = await page('/');
  console.log('home starts light:', await theme(home));
  await shot(home, 'home');
  const admin = await page('/admin');
  await shot(admin, 'admin');
  const editor = await page(`/admin/quiz/${ctx.id}`);
  await shot(editor, 'editor');
  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  const host = await page(`/host/${code}`, 1600, 900);
  await host.waitForSelector('.join-code');
  const phone = await L.openPhone(ctx, code, 'Owls');
  await shot(host, 'host-lobby');
  await host.keyboard.press('ArrowRight');
  await L.sleep(1000);
  await shot(host, 'host-question');
  await L.clickText(phone, 'button.phone-answer', 'Saturn');
  await L.sleep(1500);
  await shot(host, 'host-reveal');
  // Back to dark from the admin page: the big screen in another tab follows.
  await admin.bringToFront();
  await admin.click('.topbar button[title="Dark mode"]');
  await L.sleep(800);
  console.log('admin after the toggle:', await theme(admin), '| big screen follows:', await theme(host));
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
