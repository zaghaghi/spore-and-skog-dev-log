//! The forest of *Spore & Skog: The Divine Forage*: its creatures, made as
//! they call, in plain Rust.
//!
//! - [`wildlife`]: the game's own `wildlife.rs`: eight creatures, which one
//!   calls when, and every sample of every call;
//! - [`desk`]: the game's sound desk cut down to the forest: it runs inside
//!   the audio callback, and the game talks to it only through [`desk::Command`]s;
//! - [`device`] (feature `cpal`): one CPAL stream, fed by the desk.
//!
//! The page runs the same code compiled to WebAssembly (`wasm.rs`).

pub mod desk;
#[cfg(feature = "cpal")]
pub mod device;
pub mod wav;
#[cfg(target_arch = "wasm32")]
mod wasm;
pub mod wildlife;

/// Render one call of `kind` offline, as interleaved stereo at `rate`, ended
/// where it falls silent. `variant` picks a different call of the same kind
/// (each is random: its pitch, length, place).
pub fn one_call(kind: wildlife::Kind, variant: u32, rate: f32) -> Vec<f32> {
    let mut w = wildlife::Wildlife::new(rate);
    for _ in 0..variant {
        w.choose();
    }
    w.call(kind);
    let mut out: Vec<f32> = (0..(rate * 4.5) as usize).flat_map(|_| w.next_frame()).collect();
    let end = out.iter().rposition(|x| *x != 0.0).map_or(0, |i| i + 1);
    out.truncate(end + end % 2);
    out
}
