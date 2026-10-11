// Starting a game from the quiz list: the dialog asks how many teams may join; the lobby shows it.
//   tools/browser/run.sh check-start.js
const L = require('./lib.js');

(async () => {
  const ctx = await L.setup({ questions: [L.choice('Q', 0)] });
  // Leaving the limit out keeps the default.
  const plain = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  console.log('started without a limit:', /^\d{6}$/.test(plain));
  const admin = await L.openAdmin(ctx, '/admin');
  await L.clickText(admin, 'button', 'Start');
  await admin.waitForSelector('.dialog input[type=number]');
  console.log('default limit:', await admin.$eval('.dialog input[type=number]', (i) => i.value));
  await admin.screenshot({ path: 'out/start-dialog.png' });
  await admin.$eval('.dialog input[type=number]', (i) => { i.value = ''; });
  await admin.type('.dialog input[type=number]', '2');
  await L.clickText(admin, '.dialog button', 'Start');
  await admin.waitForSelector('.join-code', { timeout: 30000 });
  const code = await admin.$eval('.join-code', (c) => c.innerText.trim());
  console.log('lobby:', await admin.$eval('.lobby-limit', (p) => p.innerText));
  await L.openPhone(ctx, code, 'Owls');
  await L.openPhone(ctx, code, 'Cats');
  await L.sleep(800);
  console.log('lobby when full:', await admin.$eval('.lobby-limit', (p) => p.innerText));
  const bats = await (await ctx.browser.createBrowserContext()).newPage();
  await bats.goto(`${L.B}/join/${code}`, { waitUntil: 'networkidle0' });
  await bats.waitForSelector('input[placeholder="The Quizzly Bears"]');
  await bats.type('input[placeholder="The Quizzly Bears"]', 'Bats');
  await bats.click('button[type=submit]');
  await L.sleep(1500);
  console.log('third team:', (await L.text(bats)).includes('This game is full') ? 'refused: full' : await L.text(bats));
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
