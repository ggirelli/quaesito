// Plays the Playground's demo quiz from start to finish on its pretend phone, against its static
// build served under the Pages path, and checks that it only ever loads its own files.
//
//   just playground-check
//
// $PLAYGROUND is the Playground's address (default http://127.0.0.1:8094/quaesito/playground/).
// $SHOTS names a folder to save screenshots of each step in; $WIDTH and $HEIGHT size the window.
const puppeteer = require('puppeteer-core');
const L = require('./lib.js');

const URL = process.env.PLAYGROUND || 'http://127.0.0.1:8094/quaesito/playground/';
const CHROME = process.env.CHROME || '/usr/bin/google-chrome';
const SHOTS = process.env.SHOTS;

/** How to answer each question of the demo quiz, in order (slides have none). */
const ANSWERS = [
  { pick: [1] }, // Saturn
  { pick: [0, 1] }, // Dolphin and bat
  { pick: [0] }, // True
  { type: ['Canberra'] },
  { type: ['88'] },
  { order: ['Pyramids of Giza', 'Colosseum', 'Eiffel Tower', 'Burj Khalifa'] },
  { type: ['Leonardo da Vinci', 'Paris'] },
  { pairs: [['Kangaroo', 'Joey'], ['Swan', 'Cygnet'], ['Owl', 'Owlet'], ['Hare', 'Leveret']] },
];

function check(ok, what) {
  if (!ok) throw new Error(what);
}

(async () => {
  const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, args: ['--no-sandbox'] });
  const page = await browser.newPage();
  await page.setViewport({ width: +(process.env.WIDTH || 1440), height: +(process.env.HEIGHT || 900), hasTouch: true });
  const shot = async (name) => SHOTS && page.screenshot({ path: `${SHOTS}/${name}.png` });
  const foreign = [];
  const problems = [];
  page.on('request', (r) => {
    // Chrome asks every site for /favicon.ico on its own.
    if (!r.url().startsWith(URL) && !r.url().endsWith('/favicon.ico') && !r.url().startsWith('data:')) foreign.push(r.url());
  });
  page.on('response', (r) => {
    if (r.status() >= 400 && !r.url().endsWith('/favicon.ico')) problems.push(`HTTP ${r.status()} ${r.url()}`);
  });
  page.on('console', (m) => {
    if (m.type() === 'error' && !m.text().includes('favicon')) problems.push(m.text());
  });
  page.on('pageerror', (e) => problems.push(e.message));

  await page.goto(URL, { waitUntil: 'networkidle0' });
  await page.waitForSelector('.playground .host', { timeout: 15000 });
  const screen = () => page.$eval('.playground-screen', (e) => e.innerText);
  const phone = () => page.$eval('.playground-phone', (e) => e.innerText);
  const score = () => page.$eval('.playground-phone .player-score', (e) => parseInt(e.innerText, 10));
  /** The label of the big screen's main button, if any. */
  const mainButton = () => page.evaluate(() =>
    [...document.querySelectorAll('.host-controls button.primary')].pop()?.innerText ?? '');
  const next = async () => {
    await page.evaluate(() => [...document.querySelectorAll('.host-controls button.primary')].pop().click());
    await L.sleep(400);
  };

  await shot('0-intro');
  check((await phone()).includes("You're in!"), 'the phone is not in the game');
  check((await mainButton()).includes('Start the quiz'), 'the big screen cannot start');
  await next();
  check((await phone()).includes('Eyes on the big screen'), 'the phone does not follow the welcome slide');
  await next();

  let last = 0;
  for (const [i, how] of ANSWERS.entries()) {
    await page.waitForSelector('.playground-phone :is(.phone-answer, .open-form, .order-choice, .pair-choice)', { timeout: 5000 });
    check((await screen()).includes(`Question ${i + 1} of ${ANSWERS.length}`), `the big screen is not on question ${i + 1}`);
    await shot(`${i + 1}-question`);
    if (how.pick) {
      // Taps, as on a phone's touch screen.
      for (const n of how.pick) await (await page.$(`.playground-phone .phone-answer.a${n}`)).tap();
      if (how.pick.length > 1) await L.clickText(page, '.playground-phone button', 'Submit');
    } else if (how.type) {
      const fields = await page.$$('.playground-phone .open-form input');
      for (const [n, text] of how.type.entries()) await fields[n].type(text);
      await page.click('.playground-phone .open-form button[type=submit]');
    } else if (how.order) {
      for (const item of how.order) {
        await L.clickText(page, '.playground-phone .order-choice', item);
        await L.sleep(100);
      }
      await L.clickText(page, '.playground-phone button', 'Submit');
    } else if (how.pairs) {
      for (const [left, right] of how.pairs) {
        await L.clickText(page, '.playground-phone .pair-column:first-child .pair-choice', left);
        await L.clickText(page, '.playground-phone .pair-column:last-child .pair-choice', right);
        await L.sleep(100);
      }
      await L.clickText(page, '.playground-phone button', 'Submit');
    }
    // The only team answered: the question closes and both screens reveal it.
    await page.waitForFunction(() => {
      const b = [...document.querySelectorAll('.host-controls button.primary')].pop();
      return b && !b.innerText.includes('Stop the clock');
    }, { timeout: 5000 });
    await L.sleep(300);
    await shot(`${i + 1}-reveal`);
    const now = await score();
    check(now > last, `question ${i + 1} scored nothing (${last} → ${now}): ${await phone()}`);
    console.log(`question ${i + 1}: ${now - last} points`);
    last = now;
    await next();
  }

  // The closing slide, then the podium, one step at a time.
  for (let step = 0; step < 10 && !(await mainButton()).includes('Play again'); step++) await next();
  check((await mainButton()).includes('Play again'), 'the quiz did not finish');
  await L.sleep(800);
  await shot('9-podium');
  check((await phone()).includes('You finished #1 of 1'), `the phone does not show the result: ${await phone()}`);
  check((await screen()).includes(String(last)), 'the podium does not show the score');

  // Playing again starts over.
  await next();
  check((await phone()).includes("You're in!") && (await score()) === 0, 'playing again does not start over');

  check(foreign.length === 0, `requests outside the Playground:\n  ${foreign.join('\n  ')}`);
  check(problems.length === 0, `errors:\n  ${problems.join('\n  ')}`);
  console.log(`The Playground's quiz played to the end: ${last} points, no other requests, no errors.`);
  await browser.close();
})().catch((e) => {
  console.error(e.message);
  process.exit(1);
});
