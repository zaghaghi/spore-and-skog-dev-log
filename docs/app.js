// The page's side of the forest. The game's Rust (forest.wasm) runs twice:
// here, on the page's thread, to render calls for the pictures; and in an
// AudioWorklet, as the desk, where the forest is made live, as it is in the
// game's CPAL callback.

const LABELS = { Song: 'Songbird', Bee: 'Bumblebee', Owl: 'Tawny owl', Crickets: 'Crickets', Frog: 'Frog' };
const label = (name) => LABELS[name] || name;
const WEATHERS = ['Rain', 'Fog', 'Mild'];
// The game's title forest calls at 0.6; the table's at 0.7, or 0.9 looking closer.
const GAIN = 0.9;
// The game keeps its creatures well under the drums. Alone on this page they
// are brought up 12 dB (the desk's level, then its limiter, as in the game).
const MAKEUP = 4;

let wasm = null;
let wasmBytes = null;
let RATE = 48000;
let KINDS = [];

async function loadForest() {
  wasmBytes = await (await fetch('forest.wasm')).arrayBuffer();
  wasm = (await WebAssembly.instantiate(wasmBytes, {})).instance.exports;
  RATE = wasm.rate();
  const names = new TextDecoder();
  KINDS = [];
  for (let k = 0; k < wasm.kind_count(); k++) {
    const len = wasm.kind_name(k);
    KINDS.push(names.decode(new Uint8Array(wasm.memory.buffer, wasm.name_ptr(), len)));
  }
}

/** Call a render export and copy what it left in OUT (interleaved stereo). */
function take(fn, ...args) {
  const len = wasm[fn](...args);
  return new Float32Array(wasm.memory.buffer, wasm.out_ptr(), len).slice();
}

const mono = (stereo) => {
  const m = new Float32Array(stereo.length / 2);
  for (let i = 0; i < m.length; i++) m[i] = stereo[2 * i] + stereo[2 * i + 1];
  return m;
};

// ---- the audio side ---------------------------------------------------------------

const audio = { ctx: null, node: null, analyser: null, starting: null };

function startAudio() {
  if (audio.starting) return audio.starting;
  audio.starting = (async () => {
    const ctx = new (window.AudioContext || window.webkitAudioContext)({ latencyHint: 'interactive' });
    audio.ctx = ctx;
    audio.analyser = ctx.createAnalyser();
    audio.analyser.fftSize = 2048;
    audio.analyser.connect(ctx.destination);
    await ctx.audioWorklet.addModule('forest-worklet.js');
    const node = new AudioWorkletNode(ctx, 'forest-desk', { numberOfInputs: 0, numberOfOutputs: 1, outputChannelCount: [2] });
    node.port.onmessage = (e) => { if (e.data.error) console.error('desk:', e.data.error); };
    node.onprocessorerror = (e) => console.error('desk stopped:', e);
    // The worklet compiles its own instance from the bytes.
    node.port.postMessage({ type: 'bytes', bytes: wasmBytes.slice(0) });
    node.port.postMessage({ type: 'level', level: MAKEUP });
    node.connect(audio.analyser);
    audio.node = node;
    await ctx.resume();
    drawStream();
  })();
  return audio.starting;
}

const tell = (m) => audio.node && audio.node.port.postMessage(m);

/** Play a rendered sound through the desk, so what's drawn is what's heard. */
function playClip(samples, gain = 1) {
  const copy = samples.map((x) => x * gain);
  audio.node.port.postMessage({ type: 'clip', samples: copy }, [copy.buffer]);
}

// ---- the live forest ------------------------------------------------------------------

let inForest = false;

function weather() {
  const levels = [...document.querySelectorAll('[data-weather]')].map((s) => s.value / 100);
  const every = document.getElementById('every').value / 10;
  return { levels, every };
}

function sendForest() {
  const { levels, every } = weather();
  tell({ type: 'wildlife', rain: levels[0], fog: levels[1], mild: levels[2], every: inForest ? every : 0, gain: GAIN });
  const state = document.getElementById('forest-state');
  if (!inForest) {
    state.textContent = audio.ctx ? 'You stepped out. The forest falls quiet.' : state.textContent;
    return;
  }
  const sky = levels.every((l) => l === 0)
    ? 'a clear sky'
    : WEATHERS.map((w, i) => (levels[i] > 0 ? `${w.toLowerCase()} ${Math.round(levels[i] * 100)}%` : null)).filter(Boolean).join(', ');
  state.textContent = `In the forest, under ${sky}. A creature calls every ${every.toFixed(1)} s or so.`;
}

function buildForest() {
  const enter = document.querySelector('[data-action="enter"]');
  enter.addEventListener('click', async () => {
    await startAudio();
    inForest = !inForest;
    if (inForest) track('forest-enter', 'Stepped into the forest');
    enter.textContent = inForest ? 'Step out' : 'Step into the forest';
    sendForest();
  });
  document.querySelectorAll('.weather input').forEach((input) => {
    const out = input.parentElement.querySelector('output');
    const show = () => {
      out.textContent = input.id === 'every' ? `${(input.value / 10).toFixed(1)} s` : `${input.value}%`;
      drawOdds();
    };
    show();
    input.addEventListener('input', show);
    input.addEventListener('change', () => inForest && sendForest());
  });
}

/** Each creature's odds of calling next: Wildlife::choose's weights, from the habitat table. */
function drawOdds() {
  const { levels } = weather();
  const clear = Math.max(0, 1 - Math.max(...levels));
  const suits = [...levels, clear];
  const weights = KINDS.map((_, k) => suits.reduce((sum, s, w) => sum + wasm.habitat(k, w) * s, 0));
  const total = weights.reduce((a, b) => a + b, 0) || 1;
  const box = document.getElementById('odds');
  if (!box.children.length) {
    box.innerHTML = KINDS.map((name) =>
      `<div class="odd"><span>${label(name)}</span><span class="bar"><i></i></span><b></b></div>`).join('');
  }
  [...box.children].forEach((row, k) => {
    const p = weights[k] / total;
    row.querySelector('i').style.width = `${(p * 100).toFixed(1)}%`;
    row.querySelector('b').textContent = `${Math.round(p * 100)}%`;
    row.classList.toggle('none', p === 0);
  });
}

// ---- drawing ----------------------------------------------------------------------------

function css(name) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function fitCanvas(canvas) {
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const w = Math.max(1, Math.round(canvas.clientWidth * dpr));
  const h = Math.max(1, Math.round(canvas.clientHeight * dpr));
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
  }
  return [w, h, dpr];
}

function drawWave(canvas, samples, seconds) {
  const [w, h, dpr] = fitCanvas(canvas);
  const g = canvas.getContext('2d');
  g.clearRect(0, 0, w, h);
  g.strokeStyle = css('--rule');
  g.beginPath();
  g.moveTo(0, h / 2);
  g.lineTo(w, h / 2);
  g.stroke();
  let peak = 1e-9;
  for (const s of samples) peak = Math.max(peak, Math.abs(s));
  const shown = Math.min(samples.length, Math.round(seconds * RATE));
  const per = shown / w;
  g.fillStyle = css('--wave');
  for (let x = 0; x < w; x++) {
    const a = Math.floor(x * per);
    const b = Math.min(shown, Math.floor((x + 1) * per) + 1);
    if (a >= shown) break;
    let lo = 1e9, hi = -1e9;
    for (let i = a; i < b; i++) {
      if (samples[i] < lo) lo = samples[i];
      if (samples[i] > hi) hi = samples[i];
    }
    const y0 = h / 2 - (hi / peak) * h * 0.46;
    const y1 = h / 2 - (lo / peak) * h * 0.46;
    g.fillRect(x, y0, 1, Math.max(1, y1 - y0));
  }
  g.fillStyle = css('--ink-3');
  g.font = `${11 * dpr}px "JetBrains Mono", monospace`;
  g.textAlign = 'right';
  g.fillText(`${seconds < 1 ? `${(seconds * 1000).toFixed(0)} ms` : `${seconds.toFixed(2)} s`}`, w - 4 * dpr, h - 4 * dpr);
}

// A plain radix-2 FFT, for the spectrograms.
function fft(re, im) {
  const n = re.length;
  for (let i = 1, j = 0; i < n; i++) {
    let bit = n >> 1;
    for (; j & bit; bit >>= 1) j ^= bit;
    j ^= bit;
    if (i < j) {
      [re[i], re[j]] = [re[j], re[i]];
      [im[i], im[j]] = [im[j], im[i]];
    }
  }
  for (let len = 2; len <= n; len <<= 1) {
    const ang = (-2 * Math.PI) / len;
    const wr = Math.cos(ang), wi = Math.sin(ang);
    for (let i = 0; i < n; i += len) {
      let cr = 1, ci = 0;
      for (let k = 0; k < len / 2; k++) {
        const a = i + k, b = a + len / 2;
        const tr = re[b] * cr - im[b] * ci;
        const ti = re[b] * ci + im[b] * cr;
        re[b] = re[a] - tr; im[b] = im[a] - ti;
        re[a] += tr; im[a] += ti;
        const ncr = cr * wr - ci * wi;
        ci = cr * wi + ci * wr;
        cr = ncr;
      }
    }
  }
}

const SPEC_LOW = 80;
const SPEC_TOP = 12000;

function drawSpec(canvas, samples, seconds) {
  const [w, h, dpr] = fitCanvas(canvas);
  const g = canvas.getContext('2d');
  const N = seconds < 0.3 ? 256 : seconds < 1.5 ? 1024 : 2048;
  const shown = Math.round(seconds * RATE);
  const cols = Math.min(w, 420);
  const img = g.createImageData(cols, h);
  const lo = css('--spec-lo').split(',').map(Number);
  const hi = css('--spec-hi').split(',').map(Number);
  const win = new Float32Array(N).map((_, i) => 0.5 - 0.5 * Math.cos((2 * Math.PI * i) / (N - 1)));
  const re = new Float32Array(N), im = new Float32Array(N);
  const bins = Math.floor((SPEC_TOP / RATE) * N);
  let peak = 1e-9;
  const mags = [];
  for (let x = 0; x < cols; x++) {
    const centre = Math.floor((x / cols) * shown);
    for (let i = 0; i < N; i++) {
      const j = centre + i - N / 2;
      re[i] = j >= 0 && j < samples.length ? samples[j] * win[i] : 0;
      im[i] = 0;
    }
    fft(re, im);
    const col = new Float32Array(bins);
    for (let b = 0; b < bins; b++) {
      col[b] = Math.hypot(re[b], im[b]);
      if (col[b] > peak) peak = col[b];
    }
    mags.push(col);
  }
  for (let x = 0; x < cols; x++) {
    for (let y = 0; y < h; y++) {
      const u = 1 - y / h;
      const f = SPEC_LOW * Math.pow(SPEC_TOP / SPEC_LOW, u);
      const b = Math.min(bins - 1, Math.round((f / RATE) * N));
      const db = 20 * Math.log10(mags[x][b] / peak + 1e-9);
      const v = Math.pow(Math.max(0, Math.min(1, (db + 60) / 60)), 1.6);
      const o = (y * cols + x) * 4;
      img.data[o] = lo[0] + (hi[0] - lo[0]) * v;
      img.data[o + 1] = lo[1] + (hi[1] - lo[1]) * v;
      img.data[o + 2] = lo[2] + (hi[2] - lo[2]) * v;
      img.data[o + 3] = 255;
    }
  }
  const off = document.createElement('canvas');
  off.width = cols;
  off.height = h;
  off.getContext('2d').putImageData(img, 0, 0);
  g.clearRect(0, 0, w, h);
  g.drawImage(off, 0, 0, w, h);
  g.fillStyle = css('--ink-3');
  g.font = `${11 * dpr}px "JetBrains Mono", monospace`;
  for (const f of [100, 1000, 10000]) {
    const y = h * (1 - Math.log(f / SPEC_LOW) / Math.log(SPEC_TOP / SPEC_LOW));
    g.fillText(f >= 1000 ? `${f / 1000} kHz` : `${f} Hz`, 4 * dpr, Math.max(12 * dpr, y - 2 * dpr));
  }
}

function drawSound(fig, stereo) {
  const samples = mono(stereo);
  const seconds = Math.max(0.05, samples.length / RATE);
  fig._sound = { stereo, seconds };
  drawWave(fig.querySelector('canvas.wave'), samples, seconds);
  drawSpec(fig.querySelector('canvas.spec'), samples, seconds);
}

/** A playhead across a figure's waveform, while its sound plays. */
function sweep(fig) {
  const s = fig._sound;
  if (!s || matchMedia('(prefers-reduced-motion: reduce)').matches) return;
  const canvas = fig.querySelector('canvas.wave');
  let head = fig.querySelector('.head');
  if (!head) {
    head = document.createElement('div');
    head.className = 'head';
    const box = document.createElement('div');
    box.style.position = 'relative';
    canvas.replaceWith(box);
    box.append(canvas, head);
  }
  const start = performance.now();
  // The desk plays the clip at RATE whatever the device's rate, so it lasts as drawn.
  const length = s.seconds * 1000;
  const token = (fig._sweep = {});
  const step = (now) => {
    if (fig._sweep !== token) return;
    const u = (now - start) / length;
    head.style.display = u >= 1 ? 'none' : 'block';
    head.style.left = `${Math.min(1, u) * 100}%`;
    if (u < 1) requestAnimationFrame(step);
  };
  requestAnimationFrame(step);
}

function scopeFigure() {
  const fig = document.createElement('figure');
  fig.className = 'scope';
  fig.innerHTML = '<canvas class="wave"></canvas><canvas class="spec"></canvas>' +
    '<figcaption><span class="scope-name"></span><span class="scope-info"></span></figcaption>';
  return fig;
}

// ---- the creatures ---------------------------------------------------------------------------

const variants = {};

function callInto(fig, k) {
  track('creature-play', 'Played a creature');
  return startAudio().then(() => {
    variants[k] = (variants[k] ?? -1) + 1;
    const stereo = take('render_call', k, variants[k]);
    drawSound(fig, stereo);
    fig.querySelector('.scope-name').textContent = label(KINDS[k]);
    fig.querySelector('.scope-info').textContent =
      `call ${variants[k] + 1} · ${(stereo.length / 2 / RATE).toFixed(2)} s`;
    playClip(stereo, MAKEUP);
    sweep(fig);
  });
}

function creatureButton(k, fig) {
  const b = document.createElement('button');
  b.textContent = label(KINDS[k]);
  b.addEventListener('click', () => callInto(fig, k));
  return b;
}

function buildCreatures() {
  const scope = document.getElementById('call-scope');
  const grid = document.getElementById('creatures');
  KINDS.forEach((_, k) => grid.append(creatureButton(k, scope)));
  // A first picture before anything plays.
  const first = KINDS.indexOf('Cuckoo') >= 0 ? KINDS.indexOf('Cuckoo') : 0;
  drawSound(scope, take('render_call', first, 0));
  scope.querySelector('.scope-name').textContent = `${label(KINDS[first])}, for example`;
  document.querySelectorAll('.cue-row').forEach((row) => {
    const fig = scopeFigure();
    row.after(fig);
    let shown = false;
    for (const name of row.dataset.kinds.split(' ')) {
      const k = KINDS.indexOf(name);
      if (k < 0) {
        console.warn(`The forest has no ${name} any more: take it out of the page.`);
        continue;
      }
      row.append(creatureButton(k, fig));
      if (!shown) {
        drawSound(fig, take('render_call', k, 0));
        fig.querySelector('.scope-name').textContent = `${label(name)}, for example`;
        shown = true;
      }
    }
  });
}

// ---- the filter demo and the bump ------------------------------------------------------------------

function buildSvf() {
  const fig = document.querySelector('figure.demo[data-demo="svf"]');
  if (!fig) return;
  fig.insertAdjacentHTML('beforeend', `
    <div class="demo-controls">
      <label class="slider"><span>Frequency</span><output></output><input type="range" min="0" max="1000" value="560" data-p="hz"></label>
      <label class="slider"><span>Resonance (q)</span><output></output><input type="range" min="0" max="1000" value="400" data-p="q"></label>
      <label class="toggle"><input type="checkbox" checked data-p="band"> Band-pass (off: low-pass)</label>
    </div>
    <div><canvas class="wave"></canvas><canvas class="spec"></canvas></div>
    <div class="demo-buttons"><button class="play">▶ Play</button><span class="meta"></span></div>
    <figcaption>The woodpecker knocks at 1.6 kHz, q 3; the frog's throat rings at 380–520 Hz, q 9;
      the raven's nose at 1.1 kHz, q 3. Higher q: a narrower band, ringing longer.</figcaption>`);
  const hz = fig.querySelector('[data-p="hz"]');
  const q = fig.querySelector('[data-p="q"]');
  const band = fig.querySelector('[data-p="band"]');
  const p = () => ({
    hz: 100 * Math.pow(80, hz.value / 1000),
    q: 0.5 * Math.pow(40, q.value / 1000),
    band: band.checked,
  });
  let stereo = null;
  const redraw = () => {
    const v = p();
    hz.parentElement.querySelector('output').textContent = v.hz >= 1000 ? `${(v.hz / 1000).toFixed(2)} kHz` : `${Math.round(v.hz)} Hz`;
    q.parentElement.querySelector('output').textContent = v.q.toFixed(1);
    stereo = take('render_svf', v.hz, v.q, v.band ? 1 : 0);
    drawSound(fig, stereo);
  };
  const playIt = () => startAudio().then(() => { playClip(stereo); sweep(fig); });
  [hz, q].forEach((s) => { s.addEventListener('input', redraw); s.addEventListener('change', playIt); });
  band.addEventListener('change', () => { redraw(); playIt(); });
  fig.querySelector('.play').addEventListener('click', playIt);
  redraw();
}

function drawBump() {
  const svg = document.getElementById('bump-plot');
  if (!svg) return;
  const X = (t) => 20 + (t / 1.2) * 190 + 0.1 * 190 / 1.2;
  const Y = (v) => 120 - v * 100;
  let d = '';
  for (let i = 0; i <= 120; i++) {
    const t = -0.1 + (i / 120) * 1.2;
    d += `${i ? 'L' : 'M'}${X(t).toFixed(1)} ${Y(wasm.bump(t, 1, 0.3)).toFixed(1)}`;
  }
  svg.innerHTML = `
    <path d="M${X(-0.1)} ${Y(0)}H${X(1.1)}" stroke="var(--ink)" stroke-width="2" fill="none"/>
    <path d="M${X(0.3)} ${Y(0)}V${Y(1)}M${X(0.7)} ${Y(0)}V${Y(1)}" stroke="var(--rule)" stroke-dasharray="3 3"/>
    <path d="${d}" stroke="var(--blue)" stroke-width="3" fill="none"/>
    <text x="${X(0)}" y="${Y(0) + 15}" text-anchor="middle" font-size="11" fill="var(--ink-3)">0</text>
    <text x="${X(1)}" y="${Y(0) + 15}" text-anchor="middle" font-size="11" fill="var(--ink-3)">len</text>
    <text x="${X(0.15)}" y="${Y(1) - 4}" text-anchor="middle" font-size="11" fill="var(--ink-3)">edge</text>`;
}

// ---- the stream, in the forest panel -----------------------------------------------------------------

function drawStream() {
  const canvas = document.getElementById('stream-scope');
  const data = new Float32Array(audio.analyser.fftSize);
  canvas.classList.add('on');
  const tick = () => {
    const [w, h, dpr] = fitCanvas(canvas);
    const g = canvas.getContext('2d');
    audio.analyser.getFloatTimeDomainData(data);
    g.clearRect(0, 0, w, h);
    g.strokeStyle = css('--blue');
    g.lineWidth = 1.5 * dpr;
    g.beginPath();
    for (let x = 0; x < w; x++) {
      const v = data[Math.floor((x / w) * data.length)];
      const y = h / 2 - v * h * 1.5;
      x ? g.lineTo(x, y) : g.moveTo(x, y);
    }
    g.stroke();
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

// ---- start ----------------------------------------------------------------------------------------------

/** Count an event with GoatCounter, if it's on the page (no cookies); once per name per visit. */
const counted = new Set();
function track(name, title) {
  if (counted.has(name)) return;
  counted.add(name);
  try {
    window.goatcounter?.count?.({ path: name, title, event: true });
  } catch {
    // Counting is never worth breaking the page for.
  }
}

/** Each term in the text shows its definition on hover, from "Words used here". */
function defineTerms() {
  document.querySelectorAll('a.term').forEach((a) => {
    const dt = document.getElementById(a.hash.slice(1));
    const dd = dt && dt.nextElementSibling;
    if (dd) a.title = dd.textContent.replace(/\s+/g, ' ').trim();
  });
}

async function main() {
  defineTerms();
  try {
    await loadForest();
  } catch (err) {
    document.getElementById('forest-state').textContent =
      'This page needs WebAssembly to make its sounds, and it could not load. Try a recent Firefox, Chrome or Safari.';
    console.error(err);
    return;
  }
  buildForest();
  drawOdds();
  buildCreatures();
  buildSvf();
  drawBump();
  let resize = 0;
  window.addEventListener('resize', () => {
    clearTimeout(resize);
    resize = setTimeout(() => {
      document.querySelectorAll('.demo, .scope').forEach((fig) => fig._sound && drawSound(fig, fig._sound.stereo));
    }, 150);
  });
}

main();
