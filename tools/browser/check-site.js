// Screenshots the documentation site (from `just docs` or `mkdocs serve`) at desktop and phone
// widths, and reports broken images and sideways scrolling.
//
//   SITE=http://127.0.0.1:8000/quaesito node tools/browser/check-site.js
const path = require('path');
const puppeteer = require('puppeteer-core');

const SITE = process.env.SITE || 'http://127.0.0.1:8000/quaesito';
const PAGES = ['', 'getting-started/', 'question-types/', 'running-the-game/'];

(async () => {
  const browser = await puppeteer.launch({ executablePath: process.env.CHROME || '/usr/bin/google-chrome', headless: 'new', args: ['--no-sandbox'] });
  const broken = [];
  for (const page of PAGES) {
    for (const [w, h, tag] of [[1280, 900, 'desktop'], [390, 844, 'phone']]) {
      const p = await browser.newPage();
      p.on('response', (r) => { if (r.status() >= 400) broken.push(`${r.status()} ${r.url()}`); });
      await p.setViewport({ width: w, height: h });
      await p.goto(`${SITE}/${page}`, { waitUntil: 'load' });
      await new Promise((r) => setTimeout(r, 800));
      const overflow = await p.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
      const name = (page.replace(/\/$/, '') || 'home') + '-' + tag;
      console.log(name, 'sideways overflow:', overflow, 'px');
      await p.screenshot({ path: path.join(__dirname, `out/site-${name}.png`), fullPage: tag === 'desktop' && page === '' });
      await p.close();
    }
  }
  console.log('broken:', broken.length ? broken.join('\n') : 'none');
  await browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
