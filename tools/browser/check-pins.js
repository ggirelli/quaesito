// Pins must sit exactly where they were dropped: in the editor, on the phone and on the reveal.
const L = require('./lib.js');
const picture = process.argv[2];

/** Screen distance between a pin's centre and where it should be on the drawn picture. */
const offset = (page, pinSel, at) => page.evaluate((pinSel, at) => {
  const pin = document.querySelector(pinSel).getBoundingClientRect();
  const img = document.querySelector('.pin-map image').getBoundingClientRect();
  const want = { x: img.x + at.x * img.width, y: img.y + at.y * img.height };
  return Math.hypot(pin.x + pin.width / 2 - want.x, pin.y + pin.height / 2 - want.y).toFixed(1);
}, pinSel, at);

(async () => {
  const ctx = await L.setup({ questions: [L.choice('Placeholder')] });
  const ed = await L.openAdmin(ctx, `/admin/quiz/${ctx.id}`, 1280, 900);
  await ed.waitForSelector('.question-text');
  await L.clickText(ed, '.segmented button', 'Pin');
  await (await ed.$('.question input[accept="image/*"]')).uploadFile(picture);
  // First click: the picture alone, to learn its shape; second: on the drawing.
  for (const [fx, fy] of [[0.3, 0.3], [0.7, 0.25]]) {
    const map = await ed.waitForSelector('.pin-edit .pin-map');
    await map.scrollIntoView();
    const box = await map.boundingBox();
    const img = await ed.$('.pin-edit .pin-map image, .pin-edit .pin-map img');
    const ib = await img.boundingBox();
    await ed.mouse.click(ib.x + ib.width * fx, ib.y + ib.height * fy);
    await L.sleep(500);
    console.log(`editor click at ${fx},${fy}: pin off by`, await offset(ed, '.pin-edit .map-pin.answer circle', { x: fx, y: fy }), 'px', `(box ${Math.round(box.width)}x${Math.round(box.height)})`);
  }
  await L.sleep(1500);
  const saved = await L.api('/api/admin/quiz/get', { token: ctx.token, id: ctx.id });
  console.log('saved:', JSON.stringify(saved.questions[0].kind));

  const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
  const host = await L.openHost(ctx, code);
  const owls = await L.openPhone(ctx, code, 'Owls');
  await host.keyboard.press('ArrowRight');
  const map = await owls.waitForSelector('.pin-map');
  const ib = await (await owls.$('.pin-map image')).boundingBox();
  await owls.mouse.click(ib.x + ib.width * 0.4, ib.y + ib.height * 0.8);
  await L.sleep(400);
  console.log('phone pin off by', await offset(owls, '.map-pin.own circle', { x: 0.4, y: 0.8 }), 'px');
  await L.clickText(owls, 'button', 'Submit');
  await L.sleep(1500);
  console.log('reveal answer off by', await offset(host, '.map-pin.answer circle', { x: 0.7, y: 0.25 }), 'px');
  console.log('reveal team pin off by', await offset(host, '.map-pin:not(.answer) circle', { x: 0.4, y: 0.8 }), 'px');
  await host.screenshot({ path: 'out/pins-reveal.png' });
  await ed.screenshot({ path: 'out/pins-editor.png' });
  console.log('ERRORS', ctx.errors.length ? ctx.errors.join('\n') : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
