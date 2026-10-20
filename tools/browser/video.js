// Shared by the demo videos: records a page with captions on the clock of a narration (spoken by
// voice.py), then mixes the voice in and writes the video, its poster, captions and transcript
// into docs/media.
//
//   const video = await V.start(page, 'editor');
//   await video.step('intro', async () => { ...actions... });
//   await video.finish('demo-editor');
const path = require('path');
const fs = require('fs');
const { execFileSync } = require('child_process');
const L = require('./lib.js');

const OUT = path.resolve(__dirname, '../../docs/media');
const FRAMES = path.join(__dirname, 'out/frames');
/** The videos' width: 1080p (the pages are recorded at a higher pixel density to fill it). */
const WIDTH = 1920;
/** The videos' frame rate. */
const FPS = 30;
/** Pause between a line's caption showing up and its voice, for the picture to sink in. */
const LEAD_MS = 500;
/** Pause after each spoken line. */
const BREATH_MS = 1000;

/** A narration's lines (narration/<name>.json). */
const lines = (name) => JSON.parse(fs.readFileSync(path.join(__dirname, 'narration', `${name}.json`), 'utf8'));

/**
 * Records the page with Chrome's screencast: every frame keeps the time it was painted, and the
 * video shows each one for exactly as long as it was on screen, so it runs on the same clock as
 * the narration. (Puppeteer's recorder stretched the video and dropped its end.)
 * `stop()` returns the first frame's time (ms since the epoch) and the frames at FPS, numbered for
 * ffmpeg: each repeated until the next one was painted. (A concat list with each frame's duration
 * gave frames under 1/25 s a whole 1/25 s, so the picture fell seconds behind the voice.)
 */
async function record(page) {
  fs.rmSync(FRAMES, { recursive: true, force: true });
  fs.mkdirSync(FRAMES, { recursive: true });
  const cdp = await page.createCDPSession();
  const frames = [];
  cdp.on('Page.screencastFrame', ({ data, metadata, sessionId }) => {
    const file = path.join(FRAMES, `${String(frames.length).padStart(6, '0')}.jpg`);
    fs.writeFileSync(file, Buffer.from(data, 'base64'));
    frames.push({ file, at: metadata.timestamp * 1000 });
    cdp.send('Page.screencastFrameAck', { sessionId }).catch(() => {});
  });
  // Frames come in CSS pixels unless asked for more: as wide as the video, at the page's density.
  await cdp.send('Page.startScreencast', { format: 'jpeg', quality: 90, everyNthFrame: 1, maxWidth: WIDTH, maxHeight: WIDTH });
  return {
    async stop() {
      const end = Date.now();
      await cdp.send('Page.stopScreencast');
      const seq = path.join(FRAMES, 'seq');
      fs.mkdirSync(seq);
      const origin = frames[0].at;
      for (let k = 0, i = 0; origin + (k * 1000) / FPS < end; k++) {
        while (i + 1 < frames.length && frames[i + 1].at <= origin + (k * 1000) / FPS) i++;
        fs.symlinkSync(frames[i].file, path.join(seq, `${String(k).padStart(6, '0')}.jpg`));
      }
      return { origin, frames: path.join(seq, '%06d.jpg') };
    },
  };
}

/** Shows `text` in a pill at the top of the page, or at the bottom (none when empty). */
const caption = (page, text, bottom = false) => page.evaluate((text, bottom) => {
  let el = document.getElementById('demo-caption');
  if (!el) {
    el = document.createElement('div');
    el.id = 'demo-caption';
    el.style.cssText = `position:fixed;left:50%;${bottom ? 'bottom:16px' : 'top:8px'};transform:translateX(-50%);z-index:9999;`
      + 'background:#ffb703;color:#14121f;font:700 22px system-ui,sans-serif;padding:10px 22px;'
      + 'border-radius:999px;box-shadow:0 6px 24px rgb(0 0 0/.5);transition:opacity .3s;pointer-events:none;'
      + 'white-space:nowrap';
    document.body.appendChild(el);
  }
  el.textContent = text;
  el.style.opacity = text ? '1' : '0';
}, text, bottom);

/**
 * Draws a mouse pointer that follows the mouse, since screencasts leave it out; on every page the
 * tab opens from now on.
 */
async function pointer(page) {
  const draw = () => {
    const add = () => {
      if (document.getElementById('demo-pointer')) return;
      const el = document.createElement('div');
      el.id = 'demo-pointer';
      el.innerHTML = '<svg width="28" height="28" viewBox="0 0 24 24"><path d="M4 2l16 11-7 1.2L9.5 21z" '
        + 'fill="#fff" stroke="#14121f" stroke-width="1.6" stroke-linejoin="round"/></svg>';
      el.style.cssText = 'position:fixed;left:-40px;top:-40px;z-index:10000;pointer-events:none;'
        + 'filter:drop-shadow(0 2px 3px rgb(0 0 0/.45));transition:transform .12s';
      document.documentElement.appendChild(el);
      const move = (e) => { el.style.left = `${e.clientX - 4}px`; el.style.top = `${e.clientY - 2}px`; };
      document.addEventListener('mousemove', move, true);
      document.addEventListener('mousedown', () => { el.style.transform = 'scale(.82)'; }, true);
      document.addEventListener('mouseup', () => { el.style.transform = ''; }, true);
    };
    if (document.documentElement) add(); else document.addEventListener('DOMContentLoaded', add);
  };
  await page.evaluateOnNewDocument(draw);
  await page.evaluate(draw);
}

/** Glides the mouse to the middle of `target` (a selector or an element) and clicks it. */
async function click(page, target, { steps = 25, pause = 250 } = {}) {
  const el = typeof target === 'string' ? await page.waitForSelector(target, { visible: true, timeout: 15000 }) : target;
  await el.evaluate((e) => e.scrollIntoView({ block: 'nearest', behavior: 'instant' }));
  // Where it stops, should the page still be scrolling.
  let b = await el.boundingBox();
  for (let i = 0; i < 40; i++) {
    await L.sleep(50);
    const now = await el.boundingBox();
    if (!now || (b && now.x === b.x && now.y === b.y)) break;
    b = now;
  }
  if (!b) throw new Error('nothing to click: not on screen');
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps });
  await L.sleep(pause);
  await page.mouse.click(b.x + b.width / 2, b.y + b.height / 2);
}

/** The first element matching `sel` whose text contains `text`. */
async function byText(page, sel, text) {
  const handle = await page.waitForFunction((sel, text) => [...document.querySelectorAll(sel)]
    .find((e) => e.offsetParent !== null && e.innerText.includes(text)), { timeout: 15000 }, sel, text);
  return handle.asElement();
}

/** Starts recording `page` along the narration `name`, with captions at the top or `bottom`. */
async function start(page, name, { bottom = false } = {}) {
  const LINES = lines(name);
  const voice = path.join(__dirname, 'out/voice', name);
  const manifest = path.join(voice, 'manifest.json');
  // Without a voice yet (DRY=1, to try a script), each line lasts as long as it might take to say.
  const durations = process.env.DRY
    ? Object.fromEntries(LINES.map((l) => [l.id, l.text.split(' ').length * 380]))
    : JSON.parse(fs.readFileSync(manifest, 'utf8'));
  const recorder = await record(page);
  const started = Date.now();
  const cues = [];
  const sounds = [];
  const line = (id) => LINES.find((l) => l.id === id) || (() => { throw new Error(`no line ${id}`); })();
  return {
    durations,
    /** Milliseconds since recording started. */
    now: () => Date.now() - started,
    /**
     * Shows a line's caption, runs `actions`, and lasts at least as long as the line is spoken
     * (it starts LEAD_MS after the caption) and a breath after.
     */
    async step(id, actions = async () => {}, extraMs = 0) {
      const at = Date.now() - started;
      cues.push({ id, at: at + LEAD_MS, ms: durations[id] });
      await caption(page, line(id).caption, bottom);
      try {
        await actions();
      } catch (e) {
        // What the page looked like, to see what went wrong.
        await page.screenshot({ path: path.join(__dirname, 'out', `failed-${id}.png`) });
        throw e;
      }
      const left = at + LEAD_MS + durations[id] + BREATH_MS + extraMs - (Date.now() - started);
      if (left > 0) await L.sleep(left);
    },
    /** A sound the screencast cannot hear (say the big screen's music), mixed in at `at`. */
    sound(file, at, volume = 0.6) { sounds.push({ file, at, volume }); },
    /**
     * Stops recording and writes docs/media/<out>.mp4, its poster (the frame at `posterAt`
     * seconds), captions (.vtt) and transcript (-transcript.md).
     */
    async finish(out, { posterAt = 9 } = {}) {
      const { origin, frames } = await recorder.stop();
      if (process.env.DRY) {
        // A silent draft, to look over.
        const draft = path.join(__dirname, 'out', `${out}-dry.mp4`);
        execFileSync('ffmpeg', ['-loglevel', 'error', '-y', '-framerate', String(FPS), '-i', frames, '-vf', `fps=10,scale=${WIDTH}:-2`,
          '-c:v', 'libx264', '-preset', 'veryfast', '-pix_fmt', 'yuv420p', draft]);
        console.log('draft:', draft, cues.map((c) => `${c.id}@${c.at}`).join(' '));
        return;
      }
      // Cues relative to the first frame, which is where the video starts. A still page may paint
      // its first frame a little after the first cue.
      const shift = started - origin;
      for (const cue of cues) cue.at = Math.max(cue.at + shift, 0);

      // The voice-over: each line placed where its step started, mixed into one track.
      const inputs = ['-framerate', String(FPS), '-i', frames];
      const filters = [];
      const tracks = [];
      cues.forEach((cue, k) => {
        inputs.push('-i', path.join(voice, `${cue.id}.wav`));
        filters.push(`[${k + 1}:a]adelay=${cue.at}|${cue.at}[v${k}]`);
        tracks.push(`[v${k}]`);
      });
      sounds.forEach((sound, k) => {
        const at = Math.max(sound.at + shift, 0);
        inputs.push('-i', sound.file);
        filters.push(`[${cues.length + k + 1}:a]volume=${sound.volume},adelay=${at}|${at}[s${k}]`);
        tracks.push(`[s${k}]`);
      });
      const mix = `${filters.join(';')};${tracks.join('')}amix=inputs=${tracks.length}:normalize=0[voice]`;
      fs.mkdirSync(OUT, { recursive: true });
      const mp4 = path.join(OUT, `${out}.mp4`);
      // H.264 and AAC in MP4: plays everywhere and stays small.
      execFileSync('ffmpeg', ['-loglevel', 'error', '-y', ...inputs, '-filter_complex', mix,
        '-map', '0:v', '-map', '[voice]', '-vf', `scale=${WIDTH}:-2`, '-c:v', 'libx264', '-preset', 'slow', '-crf', '30',
        '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-b:a', '96k', '-movflags', '+faststart', mp4]);
      execFileSync('ffmpeg', ['-loglevel', 'error', '-y', '-ss', String(posterAt), '-i', mp4, '-frames:v', '1',
        '-q:v', '80', path.join(OUT, `${out}-poster.webp`)]);

      // Captions (WebVTT) and a transcript of what is said.
      const stamp = (ms) => new Date(ms).toISOString().slice(11, 23);
      const vtt = ['WEBVTT', ''];
      for (const [k, cue] of cues.entries()) {
        vtt.push(`${k + 1}`, `${stamp(cue.at)} --> ${stamp(cue.at + cue.ms + BREATH_MS)}`, line(cue.id).text, '');
      }
      fs.writeFileSync(path.join(OUT, `${out}.vtt`), vtt.join('\n'));
      // A Markdown fragment, included by docs/demo.md.
      const transcript = [...cues.map((cue) => `- **${stamp(cue.at).slice(3, 8)}** ${line(cue.id).text}`), ''];
      fs.writeFileSync(path.join(OUT, `${out}-transcript.md`), transcript.join('\n'));
      console.log(`${out}.mp4:`, fs.statSync(mp4).size, 'bytes');
    },
  };
}

/** The pixel density that fills the video with a page `width` CSS pixels wide. */
const density = (width) => WIDTH / width;

module.exports = { start, caption, pointer, click, byText, density, LEAD_MS };
