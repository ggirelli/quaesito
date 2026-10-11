// A number question with a range: set in the editor, a slider and a field on the phone.
//   tools/browser/run.sh check-number-range.js
const L = require('./lib.js');

(async () => {
  const ctx = await L.setup({ questions: [{ text: 'How many keys on a piano?', kind: { Number: { answer: 88 } }, answers: [] }] });
  const ed = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1280, 900);
  await ed.waitForSelector('input[aria-label="Range from"]');
  await ed.type('input[aria-label="Range from"]', '0');
  await ed.type('input[aria-label="Range to"]', '200');
  await L.sleep(2000); // autosave
  const saved = await L.api('/api/admin/quiz/get', { token: ctx.token, id: ctx.id });
  console.log('saved:', JSON.stringify(saved.questions[0].kind));
  await ed.screenshot({ path: 'out/number-range-editor.png' });
  // A range that leaves the correct number out is flagged right under it.
  await ed.$eval('input[aria-label="Range to"]', (i) => { i.value = ''; });
  await ed.type('input[aria-label="Range to"]', '50');
  await L.sleep(300);
  console.log('range warning:', await ed.$eval('.number-edit + .field-error', (p) => p.innerText).catch(() => 'MISSING'));
  await ed.screenshot({ path: 'out/number-range-invalid.png' });
  await ed.$eval('input[aria-label="Range to"]', (i) => { i.value = ''; });
  await ed.type('input[aria-label="Range to"]', '200');
  await L.sleep(2000);

  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  const host = await L.openHost(ctx, code);
  const owls = await L.openPhone(ctx, code, 'Owls');
  await host.keyboard.press('ArrowRight');
  const slider = await owls.waitForSelector('.number-slider input[type=range]');
  // Drag to about three quarters: the field follows.
  const box = await slider.boundingBox();
  await owls.mouse.click(box.x + box.width * 0.75, box.y + box.height / 2);
  await L.sleep(300);
  const field = 'input[aria-label="Your guess"][type=text]';
  console.log('field after the slider:', await owls.$eval(field, (i) => i.value));
  // Typing an exact number moves the slider.
  await owls.$eval(field, (i) => { i.value = ''; });
  await owls.type(field, '88');
  await L.sleep(300);
  console.log('slider after typing 88:', await owls.$eval('.number-slider input', (i) => i.value));
  await owls.screenshot({ path: 'out/number-range-phone.png' });
  await L.clickText(owls, 'button', 'Submit');
  await L.sleep(1000);
  console.log('phone:', (await L.text(owls)).slice(0, 80));
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
