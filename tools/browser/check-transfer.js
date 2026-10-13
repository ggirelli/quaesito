// Import and export progress: a bundle with many large pictures, sent over a throttled
// connection, shows its upload, processing and download steps; the export is saved.
//   tools/browser/run.sh check-transfer.js
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const L = require('./lib.js');

const PICTURES = 24;

/** A PNG of random noise, slow to process and impossible to compress. */
function noisePng(width, height) {
  const chunk = (type, data) => {
    const head = Buffer.alloc(8);
    head.writeUInt32BE(data.length);
    head.write(type, 4);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(zlib.crc32(Buffer.concat([Buffer.from(type), data])));
    return Buffer.concat([head, data, crc]);
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width);
  header.writeUInt32BE(height, 4);
  header.set([8, 2, 0, 0, 0], 8);
  const rows = Buffer.alloc((width * 3 + 1) * height);
  for (let i = 0; i < rows.length; i++) rows[i] = (Math.random() * 256) | 0;
  for (let y = 0; y < height; y++) rows[y * (width * 3 + 1)] = 0;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', header),
    chunk('IDAT', zlib.deflateSync(rows, { level: 1 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

/** A zip of stored (uncompressed) files. */
function zip(files) {
  const locals = [];
  const central = [];
  let offset = 0;
  for (const [name, data] of files) {
    const fileName = Buffer.from(name);
    const crc = zlib.crc32(data);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50);
    local.writeUInt16LE(20, 4);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(fileName.length, 26);
    const entry = Buffer.alloc(46);
    entry.writeUInt32LE(0x02014b50);
    entry.writeUInt16LE(20, 4);
    entry.writeUInt16LE(20, 6);
    entry.writeUInt32LE(crc, 16);
    entry.writeUInt32LE(data.length, 20);
    entry.writeUInt32LE(data.length, 24);
    entry.writeUInt16LE(fileName.length, 28);
    entry.writeUInt32LE(offset, 42);
    locals.push(local, fileName, data);
    central.push(entry, fileName);
    offset += 30 + fileName.length + data.length;
  }
  const directory = Buffer.concat(central);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50);
  end.writeUInt16LE(files.length, 8);
  end.writeUInt16LE(files.length, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, directory, end]);
}

/** Every distinct step the progress bar shows, with whether its length was known, until it goes. */
async function follow(page, file) {
  const steps = [];
  const start = Date.now();
  await page.waitForSelector('.transfer', { timeout: 10000 });
  while (await page.$('.transfer')) {
    const now = await page.evaluate(() => {
      const t = document.querySelector('.transfer');
      return t && `${t.querySelector('strong').innerText} | ${t.querySelector('span').innerText}` +
        ` | ${t.querySelector('progress').hasAttribute('value') ? 'known' : 'unknown'}`;
    });
    if (now && !(steps.length && steps[steps.length - 1].endsWith(now))) {
      steps.push(`${((Date.now() - start) / 1000).toFixed(1)} s | ${now}`);
      if (file && now.includes('Processing') && !fs.existsSync(file)) {
        await page.screenshot({ path: file });
      }
    }
    await L.sleep(40);
  }
  return steps;
}

(async () => {
  const ctx = await L.setup({ title: 'Small', questions: [L.choice('Q', 0)] });
  const out = path.join(__dirname, 'out');
  fs.mkdirSync(out, { recursive: true });
  const picture = noisePng(1600, 1200);
  const names = Array.from({ length: PICTURES }, (_, i) => `${String(i).padStart(32, '0')}.png`);
  const quiz = {
    format: 1,
    id: 'x',
    title: 'Big pictures',
    questions: names.map((image, i) => ({
      text: `Picture ${i + 1}`,
      image,
      answers: [{ text: 'A', correct: true }, { text: 'B' }],
    })),
  };
  const bundle = path.join(out, 'big-pictures.zip');
  fs.writeFileSync(bundle, zip([['quiz.json', Buffer.from(JSON.stringify(quiz))],
    ...names.map((n) => [`media/${n}`, picture])]));
  console.log('bundle MB:', (fs.statSync(bundle).size / 1e6).toFixed(0));

  const admin = await L.openAdmin(ctx, '/admin');
  const cdp = await admin.createCDPSession();
  // About 40 Mbit/s, so the upload and download take a few seconds.
  await cdp.send('Network.emulateNetworkConditions', {
    offline: false, latency: 5, downloadThroughput: 5e6, uploadThroughput: 5e6,
  });
  const downloads = path.join(out, 'downloads');
  fs.rmSync(downloads, { recursive: true, force: true });
  fs.mkdirSync(downloads);
  await cdp.send('Page.setDownloadBehavior', { behavior: 'allow', downloadPath: downloads });

  const started = Date.now();
  const shot = path.join(out, 'transfer-import.png');
  fs.rmSync(shot, { force: true });
  await (await admin.$('#import-file')).uploadFile(bundle);
  const imported = await follow(admin, shot);
  console.log(`import steps (${((Date.now() - started) / 1000).toFixed(1)} s):`);
  // Percentages are many: show the first and last of each step.
  const brief = (steps) => steps.filter((s, i) => {
    const kind = (x) => x && x.split(' | ')[2].replace(/[\d%]+/g, '#');
    return kind(s) !== kind(steps[i - 1]) || kind(s) !== kind(steps[i + 1]);
  });
  brief(imported).forEach((s) => console.log('  ', s));
  const list = await L.api('/api/admin/quizzes', { token: ctx.token });
  const big = list.find((q) => q.title === 'Big pictures');
  console.log('imported questions:', big && big.question_count);

  const exportShot = path.join(out, 'transfer-export.png');
  await admin.evaluate(() => {
    const row = [...document.querySelectorAll('.list-row')].find((r) => r.innerText.includes('Big pictures'));
    row.querySelector('button[title^="Export"]').click();
  });
  const exported = await follow(admin, null);
  await admin.screenshot({ path: exportShot });
  console.log('export steps:');
  brief(exported).forEach((s) => console.log('  ', s));
  await L.sleep(1500);
  const saved = fs.readdirSync(downloads);
  console.log('saved:', saved.map((f) => `${f} ${(fs.statSync(path.join(downloads, f)).size / 1e6).toFixed(1)} MB`));
  console.log('error box:', await admin.$eval('main', (m) => {
    const e = m.querySelector('.error');
    return e ? e.innerText : 'none';
  }));
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
