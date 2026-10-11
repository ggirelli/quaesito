// Uploads videos in the editor and checks they play on the big screen and stream in slices.
//   run.sh check-video.js small.mp4 big.mp4 [expect-refusal]
const L = require('./lib.js');
const [small, big, refusal] = process.argv.slice(2);

(async () => {
  const ctx = await L.setup({ questions: [L.choice('Name this test card', 0)] });
  const ed = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1280, 900);
  await ed.waitForSelector('.question');
  await L.sleep(1500); // upload limits load
  const input = await ed.$('.question input[accept^="image/*"]');
  await input.uploadFile(refusal ? big : small);
  if (refusal) {
    await L.sleep(800);
    console.log('refused before upload:', await ed.evaluate(() => document.querySelector('.question .error')?.innerText));
    await ctx.browser.close();
    return;
  }
  await ed.waitForSelector('.question video.thumb', { timeout: 30000 });
  console.log('label hint:', await ed.$eval('.question video.thumb', () => 'video preview shown'));
  // Replace it with the big one: removing first.
  await L.clickText(ed, '.question button', 'Remove video');
  const t0 = Date.now();
  await (await ed.$('.question input[accept^="image/*"]')).uploadFile(big);
  await ed.waitForSelector('.question video.thumb', { timeout: 120000 });
  console.log('big upload took', ((Date.now() - t0) / 1000).toFixed(1), 's');
  await L.sleep(1500);
  const quiz = await L.api('/api/admin/quiz/get', { token: ctx.token, id: ctx.id });
  const name = quiz.questions[0].image;
  console.log('saved media:', name);
  const r = await fetch(`${L.B}/media/${name}`, { headers: { range: 'bytes=0-' } });
  console.log('range bytes=0-:', r.status, r.headers.get('content-type'), r.headers.get('content-range'), (await r.arrayBuffer()).byteLength, 'bytes');

  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  const host = await L.openHost(ctx, code);
  await L.openPhone(ctx, code, 'Owls');
  await host.keyboard.press('ArrowRight');
  await host.waitForSelector('video.question-video', { timeout: 15000 });
  await L.sleep(3000);
  console.log('host video:', JSON.stringify(await host.$eval('video.question-video', (v) => ({ playing: !v.paused, t: +v.currentTime.toFixed(1), w: v.videoWidth }))));
  await host.screenshot({ path: 'out/video-host.png' });
  const state = () => host.$eval('video.question-video', (v) => (v.paused ? 'paused' : 'playing') + ' at ' + v.currentTime.toFixed(1));
  await host.keyboard.press('p'); await L.sleep(1500);
  const a = await state(); await L.sleep(1500);
  console.log('after pause:', a, '->', await state());
  await host.keyboard.press('p'); await L.sleep(1500);
  console.log('after resume:', await state());
  console.log('ERRORS', ctx.errors.length ? ctx.errors.join('\n') : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
