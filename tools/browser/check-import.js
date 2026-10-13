// Importing quizzes from a newer version: the dialog naming the questions this version would leave
// out, Cancel, and Import without them.
//   tools/browser/run.sh check-import.js
const fs = require('fs');
const path = require('path');
const L = require('./lib.js');

(async () => {
  const ctx = await L.setup({ title: 'Here already', questions: [L.choice('Q', 0)] });
  const file = path.join(__dirname, 'out', 'future-quiz.json');
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify({
    format: 99,
    id: 'x',
    title: 'From the future',
    questions: [
      { text: 'Which planet has the most moons?', answers: [{ text: 'Saturn', correct: true }, { text: 'Mars' }] },
      { text: 'Point at the moon', kind: { Hotspot: { spots: [] } } },
    ],
  }));
  const admin = await L.openAdmin(ctx, '/admin');
  const titles = () => admin.$$eval('.list-row .strong', (a) => a.map((x) => x.innerText));
  const choose = async () => {
    const input = await admin.$('input[type=file]');
    await input.uploadFile(file);
    await admin.waitForSelector('.dialog', { timeout: 10000 });
  };

  await choose();
  console.log('dialog:', await admin.$eval('.dialog', (d) => d.innerText));
  await admin.screenshot({ path: 'out/import-newer.png' });
  await L.clickText(admin, 'button', 'Cancel');
  await L.sleep(300);
  console.log('after Cancel:', await admin.$('.dialog') ? 'dialog still open' : 'closed', await titles());

  // The same file again: change events need the input cleared first.
  await admin.$eval('input[type=file]', (i) => { i.value = ''; });
  await choose();
  await L.clickText(admin, 'button', 'Import without them');
  await admin.waitForFunction(() => !document.querySelector('.dialog'), { timeout: 10000 });
  await L.sleep(500);
  console.log('after Import without them:', await titles());
  const list = await L.api('/api/admin/quizzes', { token: ctx.token });
  const imported = list.find((q) => q.title === 'From the future');
  console.log('imported question count:', imported && imported.question_count);
  console.log('ERRORS', ctx.errors.length ? ctx.errors : 'none');
  await ctx.browser.close();
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
