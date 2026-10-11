// Checks the demo quiz a fresh server adds, the help links, and the user guide at /docs/.
//   just docs-build && DEMO_QUIZ=1 tools/browser/run.sh check-help.js
const L = require('./lib.js');

(async () => {
  const token = await L.api('/api/admin/login', { password: 'test' });
  const list = await L.api('/api/admin/quizzes', { token });
  const demo = list.find((q) => q.title === 'Demo quiz');
  console.log('demo quiz:', demo ? `${demo.question_count} questions, issues ${JSON.stringify(demo.issues)}` : 'MISSING');
  if (demo) {
    const code = await L.api('/api/admin/test/start', { token, quiz_id: demo.id });
    console.log('demo quiz test game:', code);
  }

  const guide = await fetch(`${L.B}/docs/`);
  console.log('/docs/:', guide.status, (await guide.text()).match(/<title>(.*?)<\/title>/)?.[1]);
  const bare = await fetch(`${L.B}/docs`, { redirect: 'manual' });
  console.log('/docs:', bare.status, bare.headers.get('location'));
  const page = await fetch(`${L.B}/docs/getting-started/`);
  console.log('/docs/getting-started/:', page.status);
  const video = await fetch(`${L.B}/docs/media/demo.mp4`, { headers: { Range: 'bytes=0-99' } });
  console.log('/docs/media/demo.mp4 range:', video.status, video.headers.get('content-type'));
  console.log('/docs/missing:', (await fetch(`${L.B}/docs/missing/`)).status);

  const ctx = await L.setup({ questions: [L.choice('Q', 0)] });
  const home = await ctx.browser.newPage();
  ctx.watch(home, 'home');
  await home.goto(`${L.B}/`, { waitUntil: 'networkidle0' });
  console.log('home help link:', await home.$eval('.corner-links a[href="/docs/"]', (a) => a.title).catch(() => 'MISSING'));
  await home.setViewport({ width: 390, height: 300 });
  await home.screenshot({ path: 'out/check-help-home.png' });
  const admin = await L.openAdmin(ctx, '/admin');
  await admin.waitForSelector('.topbar');
  console.log('admin help link:', await admin.$eval('.topbar a[href="/docs/"]', (a) => a.innerText.trim()).catch(() => 'MISSING'));
  console.log('admin list shows demo:', await admin.evaluate(() => document.body.innerText.includes('Demo quiz')));
  await admin.screenshot({ path: 'out/check-help-admin.png' });
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error(e); process.exit(1); });
