// Checks that the app and the bundled user guide make no request outside the server: every page,
// font, script and picture must come from quaesito itself, so it all works without internet.
// Run it on a release build (debug builds load a font for dx's rebuild notice), with a fresh data
// directory so the demo quiz is added:
//   just docs-build && rm -rf target/e2e/data && RELEASE=1 DEMO_QUIZ=1 tools/browser/run.sh check-offline.js
const L = require('./lib.js');

const GUIDE = ['', 'getting-started/', 'building-a-quiz/', 'question-types/', 'quiz-settings/',
  'testing/', 'running-the-game/', 'after-the-game/', 'troubleshooting/', 'configuration/',
  'demo/', 'development/', 'how-it-was-made/'];

(async () => {
  const token = await L.api('/api/admin/login', { password: 'test' });
  const demo = (await L.api('/api/admin/quizzes', { token })).find((q) => q.title === 'Demo quiz');
  if (!demo) throw new Error('no demo quiz: run with DEMO_QUIZ=1');
  const ctx = await L.setup({ questions: [L.choice('Q', 0)] });
  const origin = new URL(L.B).origin;
  const outside = new Set();
  const local = (url) => url.startsWith('data:') || url.startsWith('blob:')
    || new URL(url.replace(/^ws/, 'http')).origin === origin;
  const track = (p) => {
    p.on('request', (r) => { if (!local(r.url())) outside.add(`${p.url()} → ${r.url()}`); });
  };
  const visit = async (path, setup) => {
    const p = await L.openAdmin(ctx, path);
    track(p);
    if (setup) await setup(p);
    await p.reload({ waitUntil: 'networkidle0' });
    await L.sleep(1500);
    console.log('visited', path);
    return p;
  };

  await visit('/');
  await visit('/join');
  await visit('/admin');
  await visit(`/admin/quiz/${demo.id}`);
  const test = await L.api('/api/admin/test/start', { token, quiz_id: demo.id });
  await visit(`/admin/test/${test}`);
  const code = await L.api('/api/admin/session/start', { token, quiz_id: demo.id });
  const host = await visit(`/host/${code}`);
  const phone = await L.openPhone(ctx, code, 'Offline');
  track(phone);
  // Play through the demo quiz on the big screen, so every question type, picture and sound loads.
  for (let i = 0; i < 40; i += 1) {
    await host.keyboard.press('ArrowRight');
    await L.sleep(700);
  }
  for (const page of GUIDE) await visit(`/docs/${page}`);

  console.log('requests outside the server:', outside.size ? `\n${[...outside].join('\n')}` : 'none');
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
  if (outside.size) process.exit(1);
})().catch((e) => { console.error(e); process.exit(1); });
