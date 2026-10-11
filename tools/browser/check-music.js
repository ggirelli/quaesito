// Stopping the clock by hand stops the soundtrack too.
//   tools/browser/run.sh check-music.js
const L = require('./lib.js');
const { tune } = require('./sample.js');

(async () => {
  const audio = tune('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa3');
  const ctx = await L.setup({ questions: [{ ...L.choice('Which way does this tune go?'), audio, time_limit_secs: 30 }] });
  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  const host = await L.openHost(ctx, code);
  await L.openPhone(ctx, code, 'Owls');
  const playing = () => host.$eval('#soundtrack', (a) => !a.paused);
  await host.keyboard.press('ArrowRight'); // ready
  await L.sleep(700);
  await host.keyboard.press('ArrowRight'); // start the music and the clock
  await L.sleep(1500);
  console.log('music playing with the clock:', await playing());
  await L.clickText(host, '.host-controls button', 'Stop the clock');
  await L.sleep(800);
  console.log('music playing after stopping the clock:', await playing());
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
