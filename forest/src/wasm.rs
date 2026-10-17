//! The devlog's way in: plain `extern "C"` exports, no wasm-bindgen.
//!
//! Two instances run on the page. One, on the page's own thread, renders
//! calls to draw them; the other runs inside an `AudioWorkletProcessor` and
//! is the desk: the worklet's `process()` is to it what CPAL's data callback
//! is in the game. Results are left in linear memory; JavaScript reads them
//! through `out_ptr` / `out_len`.

use std::cell::RefCell;
use std::sync::mpsc;

use crate::desk::{Command, Desk};
use crate::wildlife::{self, Kind, Svf};

/// The rate the page's pictures are made at.
const RATE: f32 = 48_000.0;

thread_local! {
    static OUT: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    static NAME: RefCell<String> = const { RefCell::new(String::new()) };
    static DESK: RefCell<Option<(Desk, mpsc::Sender<Command>)>> = const { RefCell::new(None) };
    static BLOCK: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    static CLIP: RefCell<Clip> = const { RefCell::new(Clip { samples: Vec::new(), at: 0.0, step: 1.0 }) };
}

fn keep(v: Vec<f32>) -> usize {
    let n = v.len();
    OUT.with(|o| *o.borrow_mut() = v);
    n
}

#[unsafe(no_mangle)]
pub extern "C" fn out_ptr() -> *const f32 {
    OUT.with(|o| o.borrow().as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn rate() -> f32 {
    RATE
}

// ---- the creatures --------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn kind_count() -> usize {
    Kind::ALL.len()
}

/// The creature's name (`Kind`'s Debug), as UTF-8 at `name_ptr`; returns its length.
#[unsafe(no_mangle)]
pub extern "C" fn kind_name(kind: usize) -> usize {
    NAME.with(|n| {
        let mut n = n.borrow_mut();
        *n = format!("{:?}", Kind::ALL[kind]);
        n.len()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn name_ptr() -> *const u8 {
    NAME.with(|n| n.borrow().as_ptr())
}

/// How much the creature likes rain (0), fog (1), mild weather (2), a clear sky (3).
#[unsafe(no_mangle)]
pub extern "C" fn habitat(kind: usize, weather: usize) -> f32 {
    Kind::ALL[kind].habitat()[weather]
}

/// One call, interleaved stereo at `rate()`, into OUT; returns its length.
#[unsafe(no_mangle)]
pub extern "C" fn render_call(kind: usize, variant: u32) -> usize {
    keep(crate::one_call(Kind::ALL[kind], variant, RATE))
}

/// White noise through the creatures' filter, for a moment: `band` 1 its
/// band-pass output, 0 its low-pass. Interleaved stereo.
#[unsafe(no_mangle)]
pub extern "C" fn render_svf(hz: f32, q: f32, band: u32) -> usize {
    let mut filter = Svf::default();
    let mut noise = wildlife::Noise(0x1234_5678);
    let n = (RATE * 0.8) as usize;
    let mut out = Vec::with_capacity(2 * n);
    let mut peak = 0.0f32;
    for i in 0..n {
        let t = i as f32 / RATE;
        let (low, mid) = filter.next(noise.next(), hz, q, RATE);
        let x = if band != 0 { mid } else { low } * wildlife::window(t, 0.8, 0.03);
        peak = peak.max(x.abs());
        out.extend([x, x]);
    }
    if peak > 0.0 {
        out.iter_mut().for_each(|x| *x *= 0.3 / peak);
    }
    keep(out)
}

/// The smooth bump every call is shaped with.
#[unsafe(no_mangle)]
pub extern "C" fn bump(t: f32, len: f32, edge: f32) -> f32 {
    wildlife::window(t, len, edge)
}

// ---- the desk, inside the AudioWorklet ---------------------------------------------

/// A rendered call played back as it was drawn, so the picture on the page
/// is the sound you hear (the game makes each call live instead).
struct Clip {
    samples: Vec<f32>,
    at: f64,
    step: f64,
}

#[unsafe(no_mangle)]
pub extern "C" fn desk_new(rate: u32) {
    let (tx, rx) = mpsc::channel();
    DESK.with(|d| *d.borrow_mut() = Some((Desk::new(rate, rx), tx)));
    CLIP.with(|c| c.borrow_mut().step = RATE as f64 / rate as f64);
}

fn send(command: Command) {
    DESK.with(|d| {
        if let Some((_, tx)) = d.borrow().as_ref() {
            let _ = tx.send(command);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn desk_wildlife(rain: f32, fog: f32, mild: f32, every: f32, gain: f32) {
    send(Command::Wildlife {
        levels: [rain, fog, mild],
        every,
        gain,
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn desk_call(kind: usize) {
    send(Command::Call(Kind::ALL[kind]));
}

#[unsafe(no_mangle)]
pub extern "C" fn desk_level(level: f32) {
    send(Command::Level(level));
}

/// Room for a clip of `len` samples (interleaved stereo at `rate()`); fill
/// it, and it plays from the next block.
#[unsafe(no_mangle)]
pub extern "C" fn desk_clip(len: usize) -> *mut f32 {
    CLIP.with(|c| {
        let mut c = c.borrow_mut();
        c.samples.clear();
        c.samples.resize(len, 0.0);
        c.at = 0.0;
        c.samples.as_mut_ptr()
    })
}

/// The callback: `frames` of stereo, interleaved. Returns where they are.
#[unsafe(no_mangle)]
pub extern "C" fn desk_render(frames: usize) -> *const f32 {
    BLOCK.with(|b| {
        let mut b = b.borrow_mut();
        b.resize(frames * 2, 0.0);
        DESK.with(|d| {
            if let Some((desk, _)) = d.borrow_mut().as_mut() {
                desk.render(&mut b, 2);
            }
        });
        CLIP.with(|c| {
            let mut c = c.borrow_mut();
            let n = c.samples.len() / 2;
            for frame in b.chunks_mut(2) {
                let i = c.at as usize;
                if i + 1 >= n {
                    break;
                }
                let t = (c.at - i as f64) as f32;
                for (ch, out) in frame.iter_mut().enumerate() {
                    let a = c.samples[2 * i + ch];
                    *out += a + (c.samples[2 * i + 2 + ch] - a) * t;
                }
                c.at += c.step;
            }
        });
        b.as_ptr()
    })
}
