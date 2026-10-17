// A soundtrack played with the answer: it takes over from the question's own when the answer is
// revealed (after the question, or in the end review), and is silent before.
//   tools/browser/run.sh check-answer-music.js
const L = require('./lib.js');
const { tune } = require('./sample.js');

/** Plays the quiz with one phone and logs which soundtrack the big screen plays, and whether. */
async function play(ctx, label) {
  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  const host = await L.openHost(ctx, code);
  await L.openPhone(ctx, code, 'Owls');
  const music = () => host.$eval('#soundtrack', (a) => `${a.src.split('/').pop()} ${a.paused ? 'paused' : 'playing'}`)
    .catch(() => 'none');
  await host.keyboard.press('ArrowRight'); // ready
  await L.sleep(700);
  await host.keyboard.press('ArrowRight'); // start the music and the clock
  await L.sleep(1500);
  console.log(`${label} open:`, await music());
  await host.keyboard.press('ArrowRight'); // stop the clock
  await L.sleep(1500);
  console.log(`${label} closed:`, await music());
  await host.keyboard.press('ArrowRight'); // the review, when revealing at the end
  await L.sleep(1500);
  console.log(`${label} next:`, await music(), '|', (await L.text(host)).slice(0, 60));
  await host.close();
}

(async () => {
  const question = tune('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa3');
  const answer = tune('bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb4');
  const ctx = await L.setup({ questions: [{
    ...L.choice('Which way does this tune go?'), audio: question, explanation_audio: answer, time_limit_secs: 30,
  }] });
  const ed = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1280, 900);
  await ed.waitForSelector('.question fieldset audio.audio-preview');
  await ed.$eval('.question fieldset', (f) => f.scrollIntoView());
  await ed.screenshot({ path: 'out/answer-music-editor.png' });
  await ed.close();

  await play(ctx, 'after-each');
  const saved = await L.api('/api/admin/quiz/get', { token: ctx.token, id: ctx.id });
  saved.settings.reveal = 'AtEnd';
  await L.api('/api/admin/quiz/save', { token: ctx.token, quiz: saved });
  await play(ctx, 'at-end');
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
