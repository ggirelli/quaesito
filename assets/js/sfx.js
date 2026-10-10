// Sound effects for the big screen, synthesised with the Web Audio API (no audio files needed).
// `window.qsfx(name)` plays one; muting is remembered in localStorage.
(() => {
  if (window.qsfx) return;
  let ctx = null;
  const audio = () => {
    if (!ctx) {
      const Context = window.AudioContext || window.webkitAudioContext;
      if (!Context) return null;
      ctx = new Context();
    }
    if (ctx.state === 'suspended') ctx.resume();
    return ctx;
  };

  // A note with a quick attack and an exponential fade.
  const tone = (freq, start, length, type, volume) => {
    const c = audio();
    if (!c) return;
    const t = c.currentTime + start;
    const osc = c.createOscillator();
    const gain = c.createGain();
    osc.type = type;
    osc.frequency.setValueAtTime(freq, t);
    gain.gain.setValueAtTime(0.0001, t);
    gain.gain.exponentialRampToValueAtTime(volume, t + 0.01);
    gain.gain.exponentialRampToValueAtTime(0.0001, t + length);
    osc.connect(gain).connect(c.destination);
    osc.start(t);
    osc.stop(t + length + 0.05);
  };

  // A short burst of filtered noise, like a snare hit.
  const hit = (start, volume) => {
    const c = audio();
    if (!c) return;
    const length = 0.06;
    const buffer = c.createBuffer(1, Math.ceil(c.sampleRate * length), c.sampleRate);
    const data = buffer.getChannelData(0);
    for (let i = 0; i < data.length; i++) data[i] = (Math.random() * 2 - 1) * (1 - i / data.length);
    const source = c.createBufferSource();
    const filter = c.createBiquadFilter();
    const gain = c.createGain();
    source.buffer = buffer;
    filter.type = 'bandpass';
    filter.frequency.value = 1800;
    gain.gain.value = volume;
    source.connect(filter).connect(gain).connect(c.destination);
    source.start(c.currentTime + start);
  };

  const sounds = {
    tick: () => tone(880, 0, 0.09, 'square', 0.06),
    lastTick: () => tone(1320, 0, 0.14, 'square', 0.08),
    timeUp: () => {
      tone(196, 0, 0.6, 'sawtooth', 0.1);
      tone(147, 0, 0.7, 'sawtooth', 0.08);
    },
    reveal: () => [523, 659, 784, 1047].forEach((f, i) => tone(f, i * 0.08, 0.3, 'triangle', 0.16)),
    join: () => {
      tone(660, 0, 0.09, 'sine', 0.14);
      tone(990, 0.07, 0.12, 'sine', 0.12);
    },
    drumroll: () => {
      for (let i = 0; i < 28; i++) hit(i * 0.045, 0.12 + i * 0.012);
      hit(28 * 0.045 + 0.05, 0.6);
    },
    fanfare: () => {
      const notes = [[523, 0], [523, 0.14], [523, 0.28], [659, 0.42], [784, 0.7], [659, 0.92], [1047, 1.06]];
      notes.forEach(([f, t]) => {
        tone(f, t, 0.4, 'triangle', 0.16);
        tone(f / 2, t, 0.4, 'sine', 0.08);
      });
    },
  };

  window.qsfx = (name) => {
    try {
      if (localStorage.getItem('quaesito.muted') === '1') return;
    } catch (e) {}
    const sound = sounds[name];
    if (sound) sound();
  };

  // Browsers only start audio after a user gesture: unlock on the first key or click.
  const unlock = () => audio();
  window.addEventListener('keydown', unlock, { once: true });
  window.addEventListener('pointerdown', unlock, { once: true });
})();
