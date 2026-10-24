// The big screen of a game that isn't running says so, whether its code is unknown or the game has
// ended. (A big screen already open when its game ends keeps its last view.)
//   tools/browser/run.sh check-host-gone.js
const L = require('./lib.js');

(async () => {
  const ctx = await L.setup({ title: `Host gone ${Date.now()}`, questions: [L.choice('One?')] });
  let ok = true;
  const says = async (what, page) => {
    const good = await page
      .waitForFunction(() => document.body.innerText.includes('This game is not running'), { timeout: 15000 })
      .then(() => true, () => false);
    console.log(what, good ? 'ok' : 'WRONG');
    ok = ok && good;
  };

  await says('unknown code:', await L.openAdmin(ctx, '/host/999999'));

  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  await L.openHost(ctx, code);
  await L.api('/api/admin/session/end', { token: ctx.token, code });
  await says('ended game:', await L.openAdmin(ctx, `/host/${code}`));

  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
  process.exit(ok && !ctx.errors.length ? 0 : 1);
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
