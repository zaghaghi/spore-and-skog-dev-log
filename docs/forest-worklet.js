// The desk, on the browser's audio thread. process() is to this page what
// CPAL's data callback is to the game: it asks the Rust desk for the next
// block of frames, and the forest is made right there, a sample at a time
// (forest/src/desk.rs, forest/src/wildlife.rs, compiled to WebAssembly).

class Desk extends AudioWorkletProcessor {
  constructor() {
    super();
    this.w = null;
    this.port.onmessage = (e) => {
      try {
        this.command(e.data);
      } catch (err) {
        this.port.postMessage({ error: String(err) });
      }
    };
  }

  command(m) {
    if (m.type === 'bytes') {
      this.w = new WebAssembly.Instance(new WebAssembly.Module(m.bytes), {}).exports;
      this.w.desk_new(sampleRate);
      this.port.postMessage({ ready: true });
      return;
    }
    const w = this.w;
    if (!w) return;
    if (m.type === 'wildlife') w.desk_wildlife(m.rain, m.fog, m.mild, m.every, m.gain);
    else if (m.type === 'call') w.desk_call(m.kind);
    else if (m.type === 'level') w.desk_level(m.level);
    else if (m.type === 'clip') {
      const ptr = w.desk_clip(m.samples.length);
      new Float32Array(w.memory.buffer, ptr, m.samples.length).set(m.samples);
    }
  }

  process(_inputs, outputs) {
    const out = outputs[0];
    const w = this.w;
    if (!w || out.length === 0) return true;
    const frames = out[0].length;
    const ptr = w.desk_render(frames);
    // Read after rendering: a grown memory would detach an older view.
    const block = new Float32Array(w.memory.buffer, ptr, frames * 2);
    if (out.length === 1) {
      for (let i = 0; i < frames; i++) out[0][i] = 0.5 * (block[2 * i] + block[2 * i + 1]);
    } else {
      for (let i = 0; i < frames; i++) {
        out[0][i] = block[2 * i];
        out[1][i] = block[2 * i + 1];
      }
    }
    return true;
  }
}

registerProcessor('forest-desk', Desk);
