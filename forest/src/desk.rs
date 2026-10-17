//! The game's sound desk (`src/audio/mixer.rs`), cut down to the forest. In
//! the game it also plays the music, the weather and the effects; the
//! creatures are mixed in the same way: made a frame at a time, inside the
//! audio callback, at the effects' level, then through the limiter.
//!
//! The game talks to it only through [`Command`]s over an `mpsc` channel:
//! the callback never waits on a lock.

use std::sync::mpsc::Receiver;

use crate::wildlife::{Kind, Wildlife};

/// How quickly the level follows a change, per second (no clicks).
const GLIDE: f32 = 12.0;
/// Above this the limiter starts to round the peaks off.
const KNEE: f32 = 0.8;

/// What the game asks of the desk.
pub enum Command {
    /// The creatures about: the weather (rain, fog, mild, 0 to 1 each), how
    /// often one calls (mean seconds, 0 for none), and how loud.
    Wildlife {
        levels: [f32; 3],
        every: f32,
        gain: f32,
    },
    /// One creature, now (the game uses this only in its tests; the devlog
    /// and the examples use it to call one up).
    Call(Kind),
    /// The bus's gain, 0 to 1 (the game's effects volume).
    Level(f32),
}

pub struct Desk {
    commands: Receiver<Command>,
    wildlife: Wildlife,
    level: f32,
    level_to: f32,
    k: f32,
}

impl Desk {
    pub fn new(rate: u32, commands: Receiver<Command>) -> Self {
        let rate = rate as f32;
        Self {
            commands,
            wildlife: Wildlife::new(rate),
            // Silent at first, fading in: no click as the stream opens.
            level: 0.0,
            level_to: 1.0,
            k: 1.0 - (-GLIDE / rate).exp(),
        }
    }

    fn take_commands(&mut self) {
        while let Ok(command) = self.commands.try_recv() {
            match command {
                Command::Wildlife {
                    levels,
                    every,
                    gain,
                } => self.wildlife.set(levels, every, gain),
                Command::Call(kind) => self.wildlife.call(kind),
                Command::Level(to) => self.level_to = to,
            }
        }
    }

    /// Fill `out`, interleaved with `channels` to a frame: what CPAL's data
    /// callback calls.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        self.take_commands();
        for frame in out.chunks_mut(channels.max(1)) {
            let [l, r] = self.next_frame();
            match frame {
                [mono] => *mono = 0.5 * (l + r),
                [a, b, rest @ ..] => {
                    *a = l;
                    *b = r;
                    rest.fill(0.0);
                }
                [] => {}
            }
        }
    }

    fn next_frame(&mut self) -> [f32; 2] {
        self.level += (self.level_to - self.level) * self.k;
        let life = self.wildlife.next_frame();
        life.map(|x| limit(x * self.level))
    }
}

/// Leaves the signal alone below `KNEE` and rounds it toward 1 above.
pub fn limit(x: f32) -> f32 {
    let a = x.abs();
    if a <= KNEE {
        x
    } else {
        x.signum() * (KNEE + (1.0 - KNEE) * ((a - KNEE) / (1.0 - KNEE)).tanh())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn the_forest_is_silent_until_told_and_calls_when_asked() {
        let (tx, rx) = mpsc::channel();
        let mut desk = Desk::new(48_000, rx);
        let mut out = vec![0.0; 2 * 48_000];
        desk.render(&mut out, 2);
        assert!(out.iter().all(|x| *x == 0.0));
        tx.send(Command::Call(Kind::Cuckoo)).unwrap();
        desk.render(&mut out, 2);
        assert!(out.iter().any(|x| *x != 0.0));
    }

    #[test]
    fn a_weather_brings_its_creatures_in() {
        let (tx, rx) = mpsc::channel();
        let mut desk = Desk::new(48_000, rx);
        tx.send(Command::Wildlife { levels: [0.0, 1.0, 0.0], every: 1.0, gain: 1.0 }).unwrap();
        let mut out = vec![0.0; 2 * 48_000 * 4];
        desk.render(&mut out, 2);
        assert!(out.iter().any(|x| *x != 0.0));
    }
}
