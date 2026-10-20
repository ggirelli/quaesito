// Plays a whole game at several big-screen sizes and reports any moment where an avatar shows
// without its team's name, a name is cut short, or the screen's content spills out of view.
//   tools/browser/run.sh check-screen-sizes.js
const L = require('./lib.js');

const TEAMS = ['The Quizzical Owls Club', 'Cats', 'Know-It-Alls Anonymous', 'Dogs', 'Les Encyclopédistes'];
const SIZES = [[1920, 1080], [1600, 900], [1280, 720], [1024, 768], [870, 740], [800, 600], [640, 480]];

(async () => {
  const problems = [];
  for (const [w, h] of SIZES) {
    const ctx = await L.setup({
      questions: [
        L.choice('One?', 0), L.choice('Two?', 1),
        { text: '', kind: { Slide: { title: '', results: true } }, answers: [] },
        L.choice('Three?', 0),
      ],
    }, { reveal: 'AtEnd' });
    const code = await L.api('/api/admin/session/start', { token: ctx.token, quiz_id: ctx.id });
    const host = await L.openHost(ctx, code, w, h);
    const phones = [];
    for (const name of TEAMS) {
      const p = await L.openPhone(ctx, code, name);
      const m = await p.$$('.mascot-choice');
      if (m.length && phones.length % 2) await m[phones.length].click();
      phones.push(p);
    }
    const look = () => host.evaluate(() => {
      const main = document.querySelector('.host-main');
      const box = main.getBoundingClientRect();
      const seen = (e) => {
        const r = e.getBoundingClientRect();
        return r.width > 0 && r.height > 0 && getComputedStyle(e).visibility !== 'hidden'
          && getComputedStyle(e).opacity !== '0';
      };
      const found = [];
      for (const a of main.querySelectorAll('.avatar')) {
        if (!seen(a)) continue;
        if (a.closest('.team-chip')) continue; // a lobby chip: the avatar then the name, as text
        const holder = a.closest('.standing, .podium-spot');
        const name = holder && holder.querySelector('.name, .team-name');
        if (!holder) found.push(`avatar outside a row in ${a.parentElement.className}`);
        else if (!name || !seen(name)) found.push(`avatar without name in ${holder.className}`);
        else if (name.scrollWidth > name.clientWidth + 1) found.push(`name cut: ${name.innerText}`);
        else {
          const r = name.getBoundingClientRect();
          if (r.bottom > box.bottom + 1 || r.top < box.top - 1) found.push(`name off screen: ${name.innerText}`);
        }
        const r = a.getBoundingClientRect();
        if (r.bottom > box.bottom + 1 || r.top < box.top - 1) found.push(`avatar off screen in ${holder && holder.className}`);
      }
      if (main.scrollHeight > main.clientHeight + 1) found.push(`screen overflows by ${main.scrollHeight - main.clientHeight}px`);
      const what = (main.querySelector('h1, .slide-title, .podium') || main).innerText.replace(/\s+/g, ' ').slice(0, 40);
      return { what, found };
    });
    const seenHere = new Set();
    for (let step = 0; step < 16; step++) {
      const t0 = Date.now();
      while (Date.now() - t0 < 1400) {
        const { what, found } = await look();
        for (const f of found) {
          const key = `${what} :: ${f.replace(/\d+px/, 'Npx')}`;
          if (seenHere.has(key)) continue;
          seenHere.add(key);
          const shot = `out/${w}x${h}-${seenHere.size}.png`;
          await host.screenshot({ path: shot });
          problems.push(`${w}x${h} step ${step} +${Date.now() - t0}ms [${what}] ${f} (${shot})`);
        }
        await L.sleep(50);
      }
      const ended = await host.evaluate(() => document.body.innerText.includes('Congratulations'));
      if (ended) break;
      const answering = await phones[0].$('button.phone-answer');
      if (answering) {
        for (const [i, phone] of phones.entries()) await L.clickText(phone, 'button.phone-answer', i % 2 ? 'also' : 'right').catch(() => {});
        await L.sleep(500);
      }
      await host.keyboard.press('ArrowRight');
    }
    console.log(`${w}x${h} done`, ctx.errors.length ? ctx.errors : '');
    await ctx.browser.close();
  }
  console.log(problems.length ? problems.join('\n') : 'ALL OK');
  process.exit(problems.length ? 1 : 0);
})().catch((e) => { console.error('FAILED', e); process.exit(1); });
