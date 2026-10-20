// Records the demo video: the test page (big screen and two phones) while a sample quiz is played,
// with captions and a voice-over (narration/play.json, spoken by voice.py), and the soundtrack
// question's music. Writes docs/media/demo.mp4, its captions (demo.vtt) and a transcript
// (demo-transcript.md).
//
//   just demo-video
const path = require('path');
const L = require('./lib.js');
const V = require('./video.js');
const sample = require('./sample.js');

/** The sample quiz's soundtrack, as the test server stored it. */
const TUNE = path.resolve(__dirname, '../../target/e2e/data/media', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa3.wav');

(async () => {
  const ctx = await L.setup(sample.quiz(), { joker: true });
  const pictures = sample.avatars(path.join(__dirname, 'out/avatars'));

  // A test game played by two named teams instead of "Phone 1" and "Phone 2".
  const code = await L.api('/api/admin/test/start', { token: ctx.token, quiz_id: ctx.id });
  for (const phone of (await L.api('/api/admin/test/info', { token: ctx.token, code })).phones) {
    await L.api('/api/admin/test/remove-phone', { token: ctx.token, code, ticket: phone.ticket });
  }
  const teams = [['The Quizzly Bears', 'bears'], ['Trivia Newton-John', 'newton']];
  for (const [name] of teams) await L.api('/api/game/join', { code, name });

  const page = await L.openAdmin(ctx, `/admin/test/${code}`, 1600, 900);
  await page.waitForFunction(() => document.querySelectorAll('.test-phone iframe').length === 2, { timeout: 90000 });
  const host = () => page.frames().find((f) => f.url().includes('/host/'));
  const phones = () => page.frames().filter((f) => f.url().includes('/join/'));
  for (const [i, [, picture]] of teams.entries()) {
    const phone = phones()[i];
    await phone.waitForSelector('input[type=file]', { timeout: 60000 });
    await (await phone.$('input[type=file]')).uploadFile(pictures[picture]);
    await phone.waitForSelector('img.avatar', { timeout: 30000 });
  }
  await L.sleep(1500);

  const next = async (wait = 1800) => { await host().evaluate(() => document.querySelector('.host-controls .button.primary')?.click()); await L.sleep(wait); };
  const tap = async (i, n) => { const f = phones()[i]; await f.waitForSelector('.phone-answer', { timeout: 15000 }); (await f.$$('.phone-answer'))[n].click(); await L.sleep(700); };
  const type = async (i, texts) => {
    const f = phones()[i];
    await f.waitForSelector('.open-form input', { timeout: 15000 });
    const fields = await f.$$('.open-form input');
    for (const [k, t] of [texts].flat().entries()) await fields[k].type(t, { delay: 60 });
    await L.sleep(300);
    await (await f.$('.open-form button[type=submit]')).click();
    await L.sleep(700);
  };

  const video = await V.start(page, 'play');
  const step = video.step;
  const DURATIONS = video.durations;
  const submit = (f) => f.evaluate(() => [...document.querySelectorAll('button')].find((b) => b.innerText.includes('Submit')).click());

  await L.sleep(800);
  await step('intro');
  await step('join');
  await step('slide', () => next(0));
  await step('choice', async () => { await next(1200); await tap(0, 1); await tap(1, 0); }, 1200);
  await step('truefalse', async () => { await next(800); await tap(1, 0); await tap(0, 0); }, 1500);
  await step('typed', async () => { await next(800); await type(0, 'Canberra'); await type(1, 'Sydney'); }, 1200);
  await step('number', async () => { await next(800); await type(0, '88'); await type(1, '76'); }, 1500);
  await step('order', async () => {
    await next(800);
    for (const [i, order] of [[0, ['Pyramids of Giza', 'Colosseum', 'Eiffel Tower', 'Burj Khalifa']], [1, ['Colosseum', 'Pyramids of Giza', 'Eiffel Tower', 'Burj Khalifa']]]) {
      const f = phones()[i];
      await f.waitForSelector('.order-choice', { timeout: 15000 });
      const items = await f.$$eval('.order-choice span:last-child', (e) => e.map((x) => x.innerText));
      for (const item of order) { (await f.$$('.order-choice'))[items.indexOf(item)].click(); await L.sleep(250); }
      await submit(f);
      await L.sleep(500);
    }
  }, 1500);
  await step('parts', async () => { await next(800); await type(0, ['Leonardo da Vinci', 'Paris']); await type(1, ['Michelangelo', 'Paris']); }, 1500);
  await step('pin', async () => {
    await next(800);
    for (const [i, at] of [[0, [0.52, 0.66]], [1, [0.32, 0.42]]]) {
      const f = phones()[i];
      await f.waitForSelector('.pin-map image', { timeout: 15000 });
      const img = await f.$('.pin-map image');
      const b = await img.boundingBox();
      await img.click({ offset: { x: b.width * at[0], y: b.height * at[1] } });
      await L.sleep(400);
      await submit(f);
      await L.sleep(500);
    }
  }, 2000);
  // Answers wait until the line is nearly spoken, so the picture stays up while it is talked about.
  await step('picture', async () => { await next(Math.max(V.LEAD_MS + DURATIONS.picture - 300, 1200)); await tap(0, 0); await tap(1, 2); }, 1500);
  // The question asks its riddle first; the music starts once the line is spoken.
  await step('music', async () => {
    await next(0);
    await L.sleep(Math.max(V.LEAD_MS + DURATIONS.music - 1500, 0));
    video.sound(TUNE, video.now());
    await next(3000);
    await tap(0, 0);
    await tap(1, 1);
  }, 3500);
  await step('podium', async () => {
    for (let i = 0; i < 4; i++) {
      if (await host().$('.host-controls a.button')) break;
      await next(2200);
    }
  }, 1500);
  await step('outro', () => {}, 1500);
  await video.finish('demo');
  console.log('ERRORS', ctx.errors.length ? ctx.errors.join('\n') : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
