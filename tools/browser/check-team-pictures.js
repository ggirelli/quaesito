// Team pictures: a game can turn them off; teams pick a background, which shows behind their
// initial and through the transparent parts of their picture, on the phone and the big screen.
//   tools/browser/run.sh check-team-pictures.js
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const L = require('./lib.js');

/** A PNG of a white disc on a transparent background (a cut-out). */
function cutout(file) {
  const size = 64;
  const raw = Buffer.alloc(size * (size * 4 + 1));
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const o = y * (size * 4 + 1) + 1 + x * 4;
      const inside = (x - 32) ** 2 + (y - 32) ** 2 < 20 ** 2;
      raw.writeUInt32BE(inside ? 0xffffffff : 0, o);
    }
  }
  const crcTable = [...Array(256)].map((_, n) => {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc = (buf) => (buf.reduce((c, b) => crcTable[(c ^ b) & 0xff] ^ (c >>> 8), 0xffffffff) ^ 0xffffffff) >>> 0;
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type), data]);
    const out = Buffer.alloc(8);
    out.writeUInt32BE(data.length, 0);
    const tail = Buffer.alloc(4);
    tail.writeUInt32BE(crc(body), 0);
    return Buffer.concat([out.subarray(0, 4), body, tail]);
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(size, 0);
  header.writeUInt32BE(size, 4);
  header.set([8, 6, 0, 0, 0], 8); // 8-bit RGBA
  fs.writeFileSync(file, Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', header), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0)),
  ]));
}

(async () => {
  const ctx = await L.setup({ title: `Team pictures ${Date.now()}`, questions: [L.choice('One?')] });
  let ok = true;
  const retype = async (page, field, value) => {
    await field.focus();
    await page.keyboard.down('Control');
    await page.keyboard.press('a');
    await page.keyboard.up('Control');
    await field.type(value);
  };
  const expect = (what, good) => {
    console.log(what, good ? 'ok' : 'WRONG');
    ok = ok && good;
  };
  const picture = path.join(__dirname, 'out/cutout.png');
  fs.mkdirSync(path.dirname(picture), { recursive: true });
  cutout(picture);

  // A game with pictures: upload a cut-out and pick a gradient background.
  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  const host = await L.openHost(ctx, code);
  const owls = await L.openPhone(ctx, code, 'Owls');
  // "You're in!" comes first; the team picture is folded away until opened.
  const order = await owls.evaluate(() => {
    const title = [...document.querySelectorAll('h1')].find((h) => h.innerText.includes("You're in"));
    const look = document.querySelector('details.team-look');
    return !!title && !!look && !look.open && !!(title.compareDocumentPosition(look) & Node.DOCUMENT_POSITION_FOLLOWING);
  });
  expect('phone: "You\'re in!" first, picture folded', order);
  await owls.click('details.team-look summary');
  // The mascots are centred as a group.
  const centred = await owls.evaluate(() => {
    const cs = [...document.querySelectorAll('.mascot-choice')].map((c) => c.getBoundingClientRect());
    const left = Math.min(...cs.map((r) => r.left)), right = Math.max(...cs.map((r) => r.right));
    return Math.abs(left - (window.innerWidth - right)) < 4;
  });
  expect('mascots centred', centred);
  // Each mascot button is square, so its selection ring is a circle.
  const square = await owls.$$eval('.mascot-choice', (bs) => bs.every((b) => {
    const r = b.getBoundingClientRect();
    return Math.abs(r.width - r.height) < 0.5;
  }));
  expect('mascot buttons square', square);
  await (await owls.$('.avatar-picker input[type=file]')).uploadFile(picture);
  await owls.waitForSelector('img.avatar.large');
  const swatches = await owls.$$('.backdrop-swatch');
  expect(`swatches: ${swatches.length}`, swatches.length === 12);
  await swatches[8].click(); // the 7th background: the first gradient
  await owls.waitForFunction(() => getComputedStyle(document.querySelector('img.avatar.large')).backgroundImage.includes('gradient'));
  const shown = await host.waitForFunction(() => {
    const img = [...document.querySelectorAll('img.avatar')].find((i) => getComputedStyle(i).backgroundImage.includes('gradient'));
    return img && img.src.includes('/media/');
  }, { timeout: 10000 }).then(() => true, () => false);
  expect('big screen: picture on its gradient', shown);
  // The cut-out keeps its transparency, so the background shows through.
  const alpha = await host.evaluate(async () => {
    const img = document.querySelector('img.avatar');
    const bitmap = await createImageBitmap(await (await fetch(img.src)).blob());
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const c = canvas.getContext('2d');
    c.drawImage(bitmap, 0, 0);
    return c.getImageData(1, 1, 1, 1).data[3];
  });
  expect(`stored picture keeps transparency (corner alpha ${alpha})`, alpha === 0);
  // Every background applies, click after click, on the phone and on the big screen.
  const look = (e) => { const s = getComputedStyle(e); return s.backgroundColor + s.backgroundImage; };
  for (let i = 1; i < swatches.length; i++) {
    await (await owls.$$('.backdrop-swatch'))[i].click();
    const want = await owls.$$eval('.backdrop-swatch', (s, i, look) => new Function('e', `return (${look})(e)`)(s[i]), i, look.toString());
    const phone = await owls.waitForFunction((w, look) => new Function('e', `return (${look})(e)`)(document.querySelector('img.avatar.large')) === w, { timeout: 5000 }, want, look.toString()).then(() => true, () => false);
    const big = await host.waitForFunction((w, look) => [...document.querySelectorAll('img.avatar')].some((a) => new Function('e', `return (${look})(e)`)(a) === w), { timeout: 5000 }, want, look.toString()).then(() => true, () => false);
    expect(`background ${i}`, phone && big);
  }
  // A ready-made picture replaces the upload, on the phone and the big screen.
  await (await owls.$$('.mascot-choice'))[0].click(); // owl
  const mascot = await host.waitForFunction(() => [...document.querySelectorAll('.avatar.mascot img')].some((i) => /owl/.test(i.src)), { timeout: 5000 }).then(() => true, () => false);
  const phoneMascot = await owls.waitForFunction(() => /owl/.test(document.querySelector('.avatar.large img')?.src || ''), { timeout: 5000 }).then(() => true, () => false);
  // Its colours turn with the background: unchanged on sky blue, turned on violet.
  const filters = async () => owls.$eval('.avatar.large .mascot-art', (i) => getComputedStyle(i).filter);
  await (await owls.$$('.backdrop-swatch'))[5].click(); // sky blue
  await owls.waitForFunction(() => document.querySelector('.avatar.large').classList.contains('backdrop-4'));
  const sky = await filters();
  await (await owls.$$('.backdrop-swatch'))[1].click(); // violet
  await owls.waitForFunction(() => document.querySelector('.avatar.large').classList.contains('backdrop-0'));
  const violet = await filters();
  expect(`mascot hue: ${sky} on sky blue, ${violet} on violet`, sky === 'hue-rotate(0deg)' && violet === 'hue-rotate(60deg)');
  expect(`mascots: ${(await owls.$$('.mascot-choice')).length}, the owl on both screens`, mascot && phoneMascot);
  await owls.screenshot({ path: 'out/team-pictures-phone.png' });
  await host.screenshot({ path: 'out/team-pictures-host.png' });

  // A game without pictures: no upload on phones, and the server refuses one; backgrounds stay.
  const plain = await L.api('/api/admin/session/start', {
    token: ctx.token, quiz_id: ctx.id, team_pictures: false,
  });
  const cats = await L.openPhone(ctx, plain, 'Cats');
  await cats.click('details.team-look summary');
  expect('no pictures: no upload', !(await cats.$('.avatar-picker')));
  await (await cats.$$('.backdrop-swatch'))[2].click();
  await cats.waitForFunction(() => /backdrop-\d/.test(document.querySelector('span.avatar.large').className));
  expect('no pictures: initial on its background', true);
  // Ready-made pictures stay available without uploads; picking one again drops it.
  await (await cats.$$('.mascot-choice'))[2].click(); // cat
  await cats.waitForFunction(() => /cat/.test(document.querySelector('.avatar.large img')?.src || ''));
  await (await cats.$$('.mascot-choice'))[2].click();
  const back = await cats.waitForFunction(() => !document.querySelector('.avatar.large img'), { timeout: 5000 }).then(() => true, () => false);
  expect('no pictures: a mascot, then back to the initial', back);
  await cats.screenshot({ path: 'out/team-pictures-off.png' });
  // The big screen's Teams panel turns uploads off mid-game: pictures go, mascots stay.
  // A file input fires no change for the same file twice: upload a copy.
  const copy = picture.replace('.png', '-2.png');
  fs.copyFileSync(picture, copy);
  await (await owls.$('.avatar-picker input[type=file]')).uploadFile(copy);
  await owls.waitForSelector('.avatar.large[src*="/media/"]');
  // Teams rename themselves in the lobby; a name another team has is refused.
  await L.openPhone(ctx, code, 'Bats');
  const nameField = await owls.$('.team-name-field input');
  await retype(owls, nameField, 'bats');
  await owls.keyboard.press('Enter');
  await nameField.evaluate((i) => i.blur());
  const refused = await owls.waitForFunction(() => document.body.innerText.includes('Another team already has that name.'), { timeout: 5000 }).then(() => true, () => false);
  const reset = await owls.$eval('.team-name-field input', (i) => i.value);
  expect(`rename: taken name refused, field back to "${reset}"`, refused && reset === 'Owls');
  // The refusal is an error response, which the browser logs: expected here.
  const refusal = ctx.errors.findIndex((e) => e.startsWith('Owls') && e.includes('status of 500'));
  if (refusal >= 0) ctx.errors.splice(refusal, 1);
  await retype(owls, await owls.$('.team-name-field input'), 'Night Owls');
  await (await owls.$('.team-name-field input')).evaluate((i) => i.blur());
  const renamed = await host.waitForFunction(() => document.body.innerText.includes('Night Owls'), { timeout: 5000 }).then(() => true, () => false);
  expect('rename: the big screen shows the new name', renamed);
  // "Done" folds the team picture away again.
  await L.clickText(owls, 'details.team-look button', 'Done');
  const folded = await owls.waitForFunction(() => !document.querySelector('details.team-look').open, { timeout: 3000 }).then(() => true, () => false);
  expect('done: picture folded away', folded);
  await owls.screenshot({ path: 'out/team-pictures-folded.png' });
  await L.clickText(host, 'button', 'Teams (');
  // Found by its label: the panel's first option lets new teams join.
  await host.waitForSelector('.panel .check-field input');
  const uploads = await host.evaluateHandle(() => [...document.querySelectorAll('.panel .check-field')]
    .find((l) => l.textContent.includes('Teams can add a picture'))?.querySelector('input'));
  expect('teams panel: uploads on', await uploads.evaluate((i) => i?.checked === true));
  await uploads.click();
  // The uploaded picture is the avatar itself (`img.avatar`); without it the phone shows the initial.
  const off = await owls.waitForFunction(() => !document.querySelector('.avatar-picker') && document.querySelector('span.avatar.large:not(.mascot)'), { timeout: 5000 }).then(() => true, () => false);
  expect('teams panel: uploads off, the picture gone from the phone', off);
  await host.screenshot({ path: 'out/team-pictures-panel.png' });
  const admin = await L.openAdmin(ctx, '/admin');
  await admin.evaluate(() => {
    [...document.querySelectorAll('button[title="Start a live game"]')][0].click();
  });
  await admin.waitForSelector('.dialog');
  const option = await L.text(admin);
  expect('start dialog: picture option, on', option.includes('Teams can add a picture')
    && await admin.$$eval('.dialog .check-field input', (is) => is[1].checked));

  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
  if (!ok || ctx.errors.length) process.exit(1);
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
