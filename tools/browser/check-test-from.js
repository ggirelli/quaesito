// "Test from here" in the editor: the test opens straight at that question or slide.
//   tools/browser/run.sh check-test-from.js
const L = require('./lib.js');

(async () => {
  const ctx = await L.setup({
    questions: [
      L.choice('First', 0),
      { text: 'Round two', kind: { Slide: { title: 'Round 2' } }, answers: [] },
      L.choice('Third', 0),
    ],
  });
  const ed = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1600, 900);
  await ed.waitForSelector('#question-2');
  await ed.click('#question-2 button[title="Test from here"]');
  await ed.waitForFunction(() => location.pathname.startsWith('/admin/test/'), { timeout: 30000 });
  await L.sleep(3000);
  const frames = ed.frames();
  const host = frames.find((f) => f.url().includes('/host/'));
  console.log('big screen:', host ? (await host.evaluate(() => document.body.innerText.replace(/\s+/g, ' '))).slice(0, 120) : 'no frame');
  await ed.screenshot({ path: 'out/test-from.png' });
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
