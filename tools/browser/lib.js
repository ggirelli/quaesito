// Shared helpers for the browser checks, documentation screenshots and demo video.
//
// Scripts run against a server at $BASE (see run.sh) with the admin password "test".
const puppeteer = require('puppeteer-core');

const B = process.env.BASE || 'http://127.0.0.1:8093';
const CHROME = process.env.CHROME || '/usr/bin/google-chrome';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Calls a server function and returns its JSON result. */
async function api(path, body) {
  const r = await fetch(B + path, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  const j = await r.json();
  if (!r.ok) throw new Error(path + ': ' + JSON.stringify(j));
  return j;
}

/** Visible text of a page on one line, without the dev build's hidden rebuild notice. */
const text = (p) => p.evaluate(() => document.body.innerText
  .replace(/Your app is being rebuilt\.\n+A non-hot-reloadable change occurred and we must rebuild\.\n+/, '')
  .replace(/\n+/g, ' | '));

/** A two-answer multiple-choice question; `right` is the index of the correct answer. */
const choice = (t, right = 0, answers = ['right', 'also']) => ({
  text: t,
  answers: answers.map((a, i) => ({ text: a, correct: i === right })),
});

/** Logs in, creates and saves a quiz, and launches a browser. */
async function setup(quizBody, settings = {}) {
  const token = await api('/api/admin/login', { password: 'test' });
  const id = await api('/api/admin/quiz/create', { token, title: quizBody.title || 'Test' });
  const quiz = Object.assign({
    id,
    title: 'Test',
    settings: Object.assign({
      default_time_limit_secs: 20, reveal: 'AfterEachQuestion', multi_scoring: 'AllOrNothing',
      advance: 'Manual', phone_display: 'FullText',
    }, settings),
  }, quizBody, { id });
  const issues = await api('/api/admin/quiz/save', { token, quiz });
  if (issues.length) throw new Error('issues ' + JSON.stringify(issues));
  const browser = await puppeteer.launch({
    executablePath: CHROME,
    headless: process.env.HEADFUL ? false : 'new',
    args: ['--no-sandbox', '--autoplay-policy=no-user-gesture-required'],
  });
  const errors = [];
  const watch = (p, name) => {
    p.on('pageerror', (e) => errors.push(`${name} pageerror ${e.message}`));
    p.on('console', (m) => {
      // Debug builds: the font dx's rebuild notice asks for is blocked by the content security policy.
      const devFont = m.text().includes('Content Security Policy') && m.text().includes('fonts.googleapis.com');
      if (m.type() === 'error' && !devFont && !m.text().includes('_dioxus') && !m.text().includes('WebSocket')) {
        errors.push(`${name} ${m.text()}`);
      }
    });
  };
  return { token, id, browser, errors, watch };
}

/** An admin page (logged in) in its own browser context. */
async function openAdmin(ctx, path, width = 1280, height = 900) {
  const p = await (await ctx.browser.createBrowserContext()).newPage();
  ctx.watch(p, path);
  await p.setViewport({ width, height });
  await p.evaluateOnNewDocument((t) => localStorage.setItem('quaesito.admin', t), ctx.token);
  await p.goto(`${B}${path}`, { waitUntil: 'networkidle0' });
  return p;
}

/** The big screen of a game, in the lobby. */
async function openHost(ctx, code, width = 1600, height = 900) {
  const host = await openAdmin(ctx, `/host/${code}`, width, height);
  await host.waitForSelector('.join-code', { timeout: 30000 });
  return host;
}

/** A phone that joins `code` as team `name`. */
async function openPhone(ctx, code, name) {
  const p = await (await ctx.browser.createBrowserContext()).newPage();
  ctx.watch(p, name);
  await p.setViewport({ width: 390, height: 844, isMobile: true, hasTouch: true });
  await p.goto(`${B}/join/${code}`, { waitUntil: 'networkidle0' });
  await p.waitForSelector('input[placeholder="The Quizzly Bears"]', { timeout: 30000 });
  await p.type('input[placeholder="The Quizzly Bears"]', name);
  await p.click('button[type=submit]');
  await p.waitForFunction(() => document.body.innerText.includes("You're in!"), { timeout: 30000 });
  return p;
}

/** Clicks the first element matching `sel` whose text contains `t`. */
const clickText = (p, sel, t) => p.evaluate((sel, t) => {
  const b = [...document.querySelectorAll(sel)].find((b) => b.innerText.includes(t));
  if (!b) throw new Error('nothing matches ' + t);
  b.click();
}, sel, t);

module.exports = { B, sleep, api, text, choice, setup, openAdmin, openHost, openPhone, clickText };
