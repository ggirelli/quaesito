// Plays the sample quiz and saves the documentation screenshots into docs/screenshots.
//
//   tools/browser/run.sh screenshots.js
const path = require('path');
const fs = require('fs');
const L = require('./lib.js');
const sample = require('./sample.js');

const OUT = path.resolve(__dirname, '../../docs/screenshots');
fs.mkdirSync(OUT, { recursive: true });
const shot = (page, name, opts = {}) => page.screenshot({ path: path.join(OUT, `${name}.webp`), type: 'webp', quality: 80, ...opts });
const next = async (host, wait = 1200) => { await host.keyboard.press('ArrowRight'); await L.sleep(wait); };

/** Answers the open question on a phone. */
async function answer(phone, how) {
  await phone.waitForSelector('.phone-answer, .open-form, .order-choice, .pin-map', { timeout: 15000 });
  if (how.pick !== undefined) {
    (await phone.$$('.phone-answer'))[how.pick].tap();
  } else if (how.type !== undefined) {
    const fields = await phone.$$('.open-form input');
    const texts = Array.isArray(how.type) ? how.type : [how.type];
    for (let i = 0; i < texts.length; i++) await fields[i].type(texts[i]);
    if (how.shoot) await shot(phone, how.shoot);
    await phone.click('.open-form button[type=submit]');
  } else if (how.order) {
    const items = await phone.$$eval('.order-choice span:last-child', (e) => e.map((x) => x.innerText));
    for (const item of how.order) { (await phone.$$('.order-choice'))[items.indexOf(item)].tap(); await L.sleep(120); }
    if (how.shoot) await shot(phone, how.shoot);
    await L.clickText(phone, 'button', 'Submit');
  } else if (how.pin) {
    const map = await phone.$('.pin-map');
    const b = await (await phone.$('.pin-map image')).boundingBox();
    await phone.mouse.click(b.x + b.width * how.pin[0], b.y + b.height * how.pin[1]);
    await L.sleep(300);
    if (how.shoot) await shot(phone, how.shoot);
    await L.clickText(phone, 'button', 'Submit');
  }
  await L.sleep(400);
}

(async () => {
  const quiz = sample.quiz();
  const ctx = await L.setup(quiz, { joker: true });
  const pictures = sample.avatars(path.join(__dirname, 'out/avatars'));

  // Admin pages.
  const home = await L.openAdmin(ctx, '/', 1280, 800);
  await L.sleep(800);
  await shot(home, 'home');
  const admin = await L.openAdmin(ctx, '/admin', 1280, 800);
  await admin.waitForSelector('.list-row');
  await L.sleep(500);
  await shot(admin, 'dashboard');
  const editor = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1280, 1000);
  await editor.waitForSelector('.question');
  for (const [i, name] of [[1, 'editor-choices'], [6, 'editor-parts'], [7, 'editor-pin']]) {
    await editor.evaluate((i) => document.querySelectorAll('.question')[i].scrollIntoView({ block: 'start' }), i);
    await editor.evaluate(() => window.scrollBy(0, -80));
    await L.sleep(500);
    await shot(editor, name);
  }

  // Test mode.
  const testCode = await L.api('/api/admin/test/start', { token: ctx.token, quiz_id: ctx.id });
  const test = await L.openAdmin(ctx, `/admin/test/${testCode}`, 1600, 900);
  await test.waitForFunction(() => document.querySelectorAll('.test-phone iframe').length === 2, { timeout: 60000 });
  await L.sleep(4000);
  await shot(test, 'test-mode');
  await L.api('/api/admin/session/end', { token: ctx.token, code: testCode });

  // A game with three teams.
  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  const host = await L.openHost(ctx, code);
  const teams = [['The Quizzly Bears', 'bears'], ['Trivia Newton-John', 'newton'], ['Les Quizérables', 'popcorn']];
  const phones = [];
  for (const [name, picture] of teams) {
    const phone = await L.openPhone(ctx, code, name);
    await (await phone.$('input[type=file]')).uploadFile(pictures[picture]);
    await phone.waitForSelector('img.avatar', { timeout: 15000 });
    phones.push(phone);
    await L.sleep(500);
  }
  await L.sleep(800);
  await shot(host, 'host-lobby');
  await shot(phones[0], 'phone-lobby');

  await next(host); // slide
  await shot(host, 'host-slide');

  await next(host); // choices
  await shot(host, 'host-question');
  await shot(phones[1], 'phone-choices');
  await answer(phones[0], { pick: 1 });
  await answer(phones[1], { pick: 0 });
  await answer(phones[2], { pick: 1 });
  await L.sleep(1200);
  await shot(host, 'host-reveal');
  await shot(phones[0], 'phone-result');

  await next(host); // true/false
  await answer(phones[0], { pick: 0 });
  await answer(phones[1], { pick: 0 });
  await answer(phones[2], { pick: 1 });
  await L.sleep(1000);

  await next(host); // typed
  await answer(phones[0], { type: 'Canberra', shoot: 'phone-typed' });
  await answer(phones[1], { type: 'Sydney' });
  await answer(phones[2], { type: 'canbera' });
  await L.sleep(1200);
  await shot(host, 'host-judging');

  await next(host); // number
  await answer(phones[0], { type: '88' });
  await answer(phones[1], { type: '76' });
  await answer(phones[2], { type: '100' });
  await L.sleep(1200);
  await shot(host, 'host-number');

  await next(host); // order
  await answer(phones[0], { order: ['Pyramids of Giza', 'Colosseum', 'Eiffel Tower', 'Burj Khalifa'], shoot: 'phone-order' });
  await answer(phones[1], { order: ['Colosseum', 'Pyramids of Giza', 'Eiffel Tower', 'Burj Khalifa'] });
  await answer(phones[2], { order: ['Pyramids of Giza', 'Colosseum', 'Burj Khalifa', 'Eiffel Tower'] });
  await L.sleep(1200);

  await next(host); // several answers
  await shot(host, 'host-parts');
  await answer(phones[0], { type: ['Leonardo da Vinci', 'Paris'], shoot: 'phone-parts' });
  await answer(phones[1], { type: ['Michelangelo', 'Paris'] });
  await answer(phones[2], { type: ['Leonardo', 'Florence'] });
  await L.sleep(1200);
  await shot(host, 'host-parts-reveal');

  await next(host); // pin
  await answer(phones[0], { pin: [0.52, 0.66], shoot: 'phone-pin' });
  await answer(phones[1], { pin: [0.3, 0.4] });
  await answer(phones[2], { pin: [0.6, 0.75] });
  await L.sleep(1200);
  await shot(host, 'host-pin');

  await next(host); // picture
  await shot(host, 'host-picture');
  for (const [i, pick] of [0, 2, 0].entries()) await answer(phones[i], { pick });
  await L.sleep(1200);

  await next(host); // soundtrack: waits for the host to start the music
  await shot(host, 'host-music');
  await shot(phones[0], 'phone-music');
  await next(host);
  for (const [i, pick] of [0, 1, 0].entries()) await answer(phones[i], { pick });
  await L.sleep(1200);

  for (let i = 0; i < 5; i++) {
    if (await host.$('.host-controls a.button')) break;
    await next(host, 1500);
  }
  await L.sleep(1500);
  await shot(host, 'host-podium');
  await shot(phones[0], 'phone-podium');
  await L.clickText(host, '.host-controls button', 'Results');
  await host.waitForSelector('table.results');
  await L.sleep(500);
  await shot(host, 'host-results');

  console.log('saved', fs.readdirSync(OUT).length, 'screenshots to', OUT);
  console.log('ERRORS', ctx.errors.length ? ctx.errors.join('\n') : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
