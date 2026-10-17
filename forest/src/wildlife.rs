//! The forest's creatures, made as they call: songbirds, the cuckoo, a
//! woodpecker drumming, a bumblebee passing, crickets, frogs in the rain, the
//! tawny owl in the fog, ravens. Now and then one calls, from somewhere left or
//! right, near or far, chosen to suit the weather (`Command::Wildlife`): busy on
//! the title, sparse at the table. No Bevy: the mixer runs this on the audio thread.
//!
//! This file is the game's `src/audio/wildlife.rs` as it ships, with `pub`
//! added so the devlog can call its parts one at a time. (Copied by
//! tools/sync.py: change it in the game, then sync.)

// The game's own names (`Noise::next`, `Svf::next`) kept, now that they're public.
#![allow(clippy::should_implement_trait)]

use std::f32::consts::{PI, TAU};

/// How loud a creature is at its nearest: under the drums, like the weather.
pub const LOUD: f32 = 0.05;
/// At most this many call at once.
pub const VOICES: usize = 3;
/// A raven's croak, and the time from one croak to the next.
pub const RAVEN_CROAK: f32 = 0.4;
pub const RAVEN_GAP: f32 = 0.75;

/// The creatures, and where they are heard: how much each likes rain, fog,
/// mild weather and a clear sky.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Song,
    Cuckoo,
    Woodpecker,
    Bee,
    Crickets,
    Frog,
    Owl,
    Raven,
}

impl Kind {
    pub const ALL: [Kind; 8] = [
        Kind::Song,
        Kind::Cuckoo,
        Kind::Woodpecker,
        Kind::Bee,
        Kind::Crickets,
        Kind::Frog,
        Kind::Owl,
        Kind::Raven,
    ];

    pub fn habitat(self) -> [f32; 4] {
        match self {
            Kind::Song => [0.1, 0.0, 1.0, 0.8],
            Kind::Cuckoo => [0.0, 0.1, 0.6, 0.5],
            Kind::Woodpecker => [0.1, 0.4, 0.4, 0.6],
            Kind::Bee => [0.0, 0.0, 0.6, 0.2],
            Kind::Crickets => [0.0, 0.0, 0.6, 0.1],
            Kind::Frog => [1.0, 0.3, 0.0, 0.1],
            Kind::Owl => [0.1, 1.0, 0.0, 0.0],
            Kind::Raven => [0.4, 0.6, 0.1, 0.3],
        }
    }
}

/// White noise (xorshift).
pub struct Noise(pub u32);

impl Noise {
    pub fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1 << 23) as f32 - 1.0
    }

    /// 0 to 1.
    pub fn unit(&mut self) -> f32 {
        0.5 + 0.5 * self.next()
    }

    pub fn between(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }
}

/// A state-variable filter (topology-preserving): low and band outputs.
#[derive(Default, Clone, Copy)]
pub struct Svf {
    a: f32,
    b: f32,
}

impl Svf {
    /// One step at `hz` with resonance `q`; returns (low, band).
    pub fn next(&mut self, x: f32, hz: f32, q: f32, rate: f32) -> (f32, f32) {
        let g = (PI * hz.min(rate * 0.45) / rate).tan();
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.b;
        let v1 = a1 * self.a + a2 * v3;
        let v2 = self.b + a2 * self.a + a3 * v3;
        self.a = 2.0 * v1 - self.a;
        self.b = 2.0 * v2 - self.b;
        (v2, v1)
    }
}

/// A smooth bump over `0..len`, rising and falling over `edge`.
pub fn window(t: f32, len: f32, edge: f32) -> f32 {
    if t < 0.0 || t > len {
        return 0.0;
    }
    let e = edge.min(len / 2.0).max(1e-4);
    let up = (t / e).min(1.0);
    let down = ((len - t) / e).min(1.0);
    let w = up.min(down);
    w * w * (3.0 - 2.0 * w)
}

/// One creature calling: its kind, where it is, and how far through.
pub struct Call {
    kind: Kind,
    age: f32,
    len: f32,
    gain: f32,
    pan: f32,
    /// Pan at the end (the bee crosses), and distance as a low-pass.
    pan_to: f32,
    dull: f32,
    phase: f32,
    filter: Svf,
    filter2: Svf,
    air: Svf,
    distance: Svf,
    /// Per call: a pitch, a count, a rate, and notes (start, length, from, to).
    pitch: f32,
    count: f32,
    rate: f32,
    notes: Vec<(f32, f32, f32, f32)>,
    last_pulse: i32,
    /// The raven's: its throat's tremolo (phase), the slow waver in its
    /// rate, and the bright resonance high up.
    tremolo: f32,
    waver: Svf,
    bright: Svf,
}

impl Call {
    pub fn new(kind: Kind, noise: &mut Noise, gain: f32) -> Self {
        let mut call = Call {
            kind,
            age: 0.0,
            len: 1.0,
            gain,
            pan: noise.between(-0.8, 0.8),
            pan_to: 0.0,
            dull: noise.between(0.0, 0.7),
            phase: 0.0,
            filter: Svf::default(),
            filter2: Svf::default(),
            air: Svf::default(),
            distance: Svf::default(),
            pitch: 0.0,
            count: 0.0,
            rate: 0.0,
            notes: Vec::new(),
            last_pulse: -1,
            tremolo: 0.0,
            waver: Svf::default(),
            bright: Svf::default(),
        };
        call.pan_to = call.pan;
        match kind {
            Kind::Song => {
                // A phrase of quick sweeps around one pitch.
                let base = noise.between(2600.0, 4200.0);
                let mut at = 0.0;
                for _ in 0..2 + (noise.unit() * 5.0) as usize {
                    let up = noise.unit() < 0.5;
                    let len = noise.between(0.05, 0.12);
                    let span = noise.between(400.0, 1300.0);
                    let (from, to) = if up { (base, base + span) } else { (base + span, base) };
                    call.notes.push((at, len, from, to));
                    at += len + noise.between(0.03, 0.09);
                }
                call.len = at;
                call.gain *= 0.7;
            }
            Kind::Cuckoo => {
                // "Cu-ckoo", two to four times: a falling third.
                let f = noise.between(600.0, 700.0);
                let times = 2 + (noise.unit() * 3.0) as usize;
                for i in 0..times {
                    let at = i as f32 * 0.78;
                    call.notes.push((at, 0.2, f * 1.01, f));
                    call.notes.push((at + 0.3, 0.32, f * 0.8, f * 0.78));
                }
                call.len = times as f32 * 0.78;
                call.dull = call.dull.max(0.35);
            }
            Kind::Woodpecker => {
                // A drum roll on dead wood: quick knocks, quickening, dying away.
                call.count = noise.between(12.0, 22.0).floor();
                call.rate = noise.between(15.0, 22.0);
                call.pitch = noise.between(600.0, 900.0);
                call.len = call.count / call.rate + 0.05;
            }
            Kind::Bee => {
                // A bumblebee crossing, its buzz falling as it passes.
                call.pitch = noise.between(140.0, 190.0);
                call.len = noise.between(2.2, 3.4);
                let side = if noise.unit() < 0.5 { -1.0 } else { 1.0 };
                call.pan = -0.85 * side;
                call.pan_to = 0.85 * side;
                call.dull = 0.0;
                call.gain *= 0.55;
            }
            Kind::Crickets => {
                // Chirps of three quick pulses on a high tone, for a while.
                call.pitch = noise.between(4100.0, 4700.0);
                call.rate = noise.between(2.6, 3.4);
                call.len = noise.between(2.5, 4.0);
                call.gain *= 0.7;
            }
            Kind::Frog => {
                // A low purring croak, two to four times.
                call.pitch = noise.between(380.0, 520.0);
                call.rate = noise.between(26.0, 38.0);
                call.count = 2.0 + (noise.unit() * 3.0).floor();
                call.len = call.count * 0.62;
                call.gain *= 10.0;
            }
            Kind::Owl => {
                // The tawny owl: "hoooo …… hu, hu-hu-hoooo", far off in the fog.
                let f = noise.between(380.0, 440.0);
                call.notes = vec![
                    (0.0, 0.75, f * 0.97, f),
                    (1.35, 0.16, f * 0.98, f * 0.96),
                    (1.62, 1.15, f, f * 0.97),
                ];
                call.len = 2.8;
                call.dull = call.dull.max(0.45);
            }
            Kind::Raven => {
                // "Kraa", once to three times: a low voice thrown up a fifth
                // and falling back, its throat fluttering fast.
                call.count = 1.0 + (noise.unit() * 3.0).floor();
                call.pitch = noise.between(104.0, 116.0);
                call.len = (call.count - 1.0) * RAVEN_GAP + RAVEN_CROAK;
            }
        }
        call
    }

    /// The next sample, or None when the call is over.
    pub fn next(&mut self, noise: &mut Noise, rate: f32) -> Option<[f32; 2]> {
        let dt = 1.0 / rate;
        let t = self.age;
        if t > self.len {
            return None;
        }
        self.age += dt;
        let x = match self.kind {
            Kind::Song | Kind::Cuckoo | Kind::Owl => {
                let Some(&(start, len, from, to)) =
                    self.notes.iter().find(|n| t >= n.0 && t < n.0 + n.1)
                else {
                    return Some([0.0; 2]);
                };
                let u = (t - start) / len;
                let hz = from + (to - from) * u;
                self.phase = (self.phase + hz * dt).fract();
                let p = self.phase * TAU;
                match self.kind {
                    Kind::Song => p.sin() * (u * PI).sin(),
                    Kind::Cuckoo => (p.sin() + 0.18 * (2.0 * p).sin()) * window(u, 1.0, 0.18),
                    _ => {
                        // The owl's last note quavers at its start.
                        let quaver = if start > 1.5 {
                            1.0 - 0.6 * (1.0 - u).powi(3) * (0.5 + 0.5 * (t * 2.0 * TAU * 6.0).sin())
                        } else {
                            1.0
                        };
                        let breath = self.air.next(noise.next(), hz * 2.0, 2.0, rate).1 * 0.25;
                        (p.sin() + breath) * window(u, 1.0, 0.3) * quaver
                    }
                }
            }
            Kind::Woodpecker => {
                // Knocks come a little faster as the roll goes on.
                let pos = t * self.rate * (1.0 + 0.15 * t);
                let i = pos.floor();
                if i >= self.count {
                    0.0
                } else {
                    let local = (pos - i) / (self.rate * (1.0 + 0.15 * t));
                    let fade = 1.0 - 0.65 * i / self.count;
                    let knock = (-local / 0.005).exp();
                    let wood = self.filter.next(noise.next(), 1600.0, 3.0, rate).1 * 2.0;
                    let ring = (TAU * self.pitch * local).sin() * 0.6;
                    (wood + ring) * knock * fade
                }
            }
            Kind::Bee => {
                let u = t / self.len;
                let hz = self.pitch * (1.08 - 0.16 * u) * (1.0 + 0.03 * (t * TAU * 5.0).sin());
                self.phase = (self.phase + hz * dt).fract();
                let saw = 2.0 * self.phase - 1.0;
                let body = self.filter.next(saw, 700.0, 1.2, rate).0;
                body * (u * PI).sin().powf(1.5)
            }
            Kind::Crickets => {
                let chirp = (t * self.rate).fract() / self.rate;
                let pulse = (chirp / 0.04) as usize;
                let on = pulse < 3 && (chirp % 0.04) < 0.025;
                self.phase = (self.phase + self.pitch * dt).fract();
                let gate = if on { window(chirp % 0.04, 0.025, 0.005) } else { 0.0 };
                (self.phase * TAU).sin() * gate * window(t, self.len, 0.5)
            }
            Kind::Frog => {
                let croak = (t / 0.62).floor();
                let local = t - croak * 0.62;
                let on = croak < self.count && local < 0.34;
                // A train of glottal pulses ringing a throat.
                let pulse = (local * self.rate) as i32;
                let kick = if on && pulse != self.last_pulse {
                    self.last_pulse = pulse;
                    1.0
                } else {
                    0.0
                };
                if !on {
                    self.last_pulse = -1;
                }
                let throat = self.filter.next(kick, self.pitch, 9.0, rate).1;
                let chest = self.filter2.next(kick, self.pitch * 0.5, 5.0, rate).1;
                (throat + chest * 0.6) * window(local, 0.34, 0.04)
            }
            Kind::Raven => {
                let croak = (t / RAVEN_GAP).floor();
                let local = t - croak * RAVEN_GAP;
                if croak >= self.count || local > RAVEN_CROAK {
                    0.0
                } else {
                    // The pitch thrown up a fifth in 15 ms, then falling back
                    // over the next 150 ms; nothing held.
                    let throw = if local < 0.015 {
                        local / 0.015
                    } else {
                        (-(local - 0.015) / 0.15).exp()
                    };
                    let hz = self.pitch * 2f32.powf(7.0 * throw / 12.0);
                    self.phase = (self.phase + hz * dt).fract();
                    let saw = 2.0 * self.phase - 1.0;
                    // The throat: a fast tremolo, 55 times a second and nearly
                    // square, its rate wavering a little so it never ticks.
                    let waver =
                        (self.waver.next(noise.next(), 6.0, 0.7, rate).0 * 40.0).clamp(-1.0, 1.0);
                    self.tremolo = (self.tremolo + 55.0 * (1.0 + 0.15 * waver) * dt).fract();
                    let square = (2.5 * (self.tremolo * TAU).sin()).tanh() / 2.5f32.tanh();
                    let src = saw * (1.0 - 0.8 * (0.5 - 0.5 * square));
                    // "Ah" at 1 kHz, a nasal edge at 1.5 kHz, and a lift at
                    // 2.5 to 4 kHz.
                    let ah = self.filter.next(src, 1000.0, 3.5, rate).1
                        - self.filter2.next(src, 1550.0, 4.0, rate).1 * 0.45
                        + self.bright.next(src, 3200.0, 1.6, rate).1 * 0.55
                        + src * 0.08;
                    ah * window(local, RAVEN_CROAK, 0.012) * (1.0 - 0.5 * local / RAVEN_CROAK)
                }
            }
        };
        // Far creatures are duller (a low-pass), and quieter.
        let near = 1.0 - self.dull;
        let x = if self.dull > 0.01 {
            self.distance.next(x, 9000.0 - 7000.0 * self.dull, 0.7, rate).0
        } else {
            x
        };
        let g = self.gain * (0.35 + 0.65 * near);
        let pan = self.pan + (self.pan_to - self.pan) * (t / self.len);
        let angle = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
        Some([x * g * angle.cos(), x * g * angle.sin()])
    }
}

pub struct Wildlife {
    rate: f32,
    noise: Noise,
    /// Rain, fog, mild: how much of each is about.
    levels: [f32; 3],
    /// Mean seconds between calls (0: none), and the gain for this place.
    every: f32,
    gain: f32,
    wait: f32,
    last: Option<Kind>,
    calls: Vec<Call>,
    /// Calls made, in all.
    called: u32,
}

impl Wildlife {
    pub fn new(rate: f32) -> Self {
        Self {
            rate,
            noise: Noise(0x5eed_f0e5),
            levels: [0.0; 3],
            every: 0.0,
            gain: 1.0,
            wait: 1.0,
            last: None,
            calls: Vec::with_capacity(VOICES),
            called: 0,
        }
    }

    /// The weather about (rain, fog, mild, 0 to 1 each), how often a creature
    /// calls (mean seconds; 0 for none), and how loud they are.
    pub fn set(&mut self, levels: [f32; 3], every: f32, gain: f32) {
        self.levels = levels.map(|l| l.clamp(0.0, 1.0));
        if every > 0.0 && self.every <= 0.0 {
            // Newly in the forest: the first call comes soon.
            self.wait = self.noise.between(0.4, 1.2) * every.min(3.0);
        }
        self.every = every.max(0.0);
        self.gain = gain;
    }

    /// Which creature calls next: weighted by how well the weather suits it,
    /// and seldom the same one twice running.
    pub fn choose(&mut self) -> Kind {
        let [rain, fog, mild] = self.levels;
        let clear = (1.0 - rain.max(fog).max(mild)).max(0.0);
        let suits = [rain, fog, mild, clear];
        let weights = Kind::ALL.map(|k| {
            let w: f32 = k.habitat().iter().zip(suits).map(|(h, s)| h * s).sum();
            if Some(k) == self.last { w * 0.25 } else { w }
        });
        let total: f32 = weights.iter().sum();
        let mut pick = self.noise.unit() * total;
        for (k, w) in Kind::ALL.into_iter().zip(weights) {
            if pick < w {
                return k;
            }
            pick -= w;
        }
        Kind::Song
    }

    /// Start a call of `kind` now (also for tests and listening).
    pub fn call(&mut self, kind: Kind) {
        if self.calls.len() < VOICES {
            let gain = LOUD * self.gain;
            self.calls.push(Call::new(kind, &mut self.noise, gain));
            self.last = Some(kind);
            self.called += 1;
        }
    }

    pub fn next_frame(&mut self) -> [f32; 2] {
        if self.every > 0.0 {
            self.wait -= 1.0 / self.rate;
            if self.wait <= 0.0 {
                // Gaps of a third to twice the mean, so calls neither queue nor stall.
                self.wait = self.every * self.noise.between(0.35, 1.65);
                let kind = self.choose();
                self.call(kind);
            }
        }
        if self.calls.is_empty() {
            return [0.0; 2];
        }
        let mut out = [0.0; 2];
        let (noise, rate) = (&mut self.noise, self.rate);
        self.calls.retain_mut(|c| match c.next(noise, rate) {
            Some([l, r]) => {
                out[0] += l;
                out[1] += r;
                true
            }
            None => false,
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    pub fn one(kind: Kind) -> Vec<f32> {
        let mut w = Wildlife::new(RATE);
        w.call(kind);
        let mut out = Vec::new();
        while !w.calls.is_empty() {
            out.extend(w.next_frame());
        }
        out
    }

    #[test]
    pub fn every_creature_calls_softly_and_then_stops() {
        for kind in Kind::ALL {
            let out = one(kind);
            let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(out.iter().all(|x| x.is_finite()), "{kind:?} stays finite");
            assert!(peak > 0.004, "{kind:?} is heard (peak {peak})");
            assert!(peak < 0.25, "{kind:?} stays under the drums (peak {peak})");
            assert!(out.len() / 2 < RATE as usize * 5, "{kind:?} ends");
        }
    }

    #[test]
    pub fn silent_until_asked() {
        let mut w = Wildlife::new(RATE);
        assert!((0..RATE as usize * 10).all(|_| w.next_frame() == [0.0; 2]));
    }

    #[test]
    pub fn the_weather_picks_the_creatures() {
        let count = |levels: [f32; 3], kind: Kind| {
            let mut w = Wildlife::new(RATE);
            w.set(levels, 1.0, 1.0);
            (0..400).filter(|_| w.choose() == kind).count()
        };
        assert_eq!(count([0.0, 0.0, 1.0], Kind::Owl), 0, "no owls on a sunny day");
        assert_eq!(count([0.0, 0.0, 1.0], Kind::Frog), 0, "nor frogs");
        assert!(count([0.0, 1.0, 0.0], Kind::Owl) > 80, "owls in the fog");
        assert!(count([1.0, 0.0, 0.0], Kind::Frog) > 120, "frogs in the rain");
        assert!(count([0.0, 0.0, 0.0], Kind::Song) > 60, "birds under a clear sky");
    }

    #[test]
    pub fn calls_come_about_as_often_as_asked() {
        let mut w = Wildlife::new(RATE);
        w.set([0.0, 0.0, 1.0], 2.0, 1.0);
        for _ in 0..(RATE as usize * 60) {
            w.next_frame();
        }
        let heard = w.called;
        assert!((15..=45).contains(&heard), "about 30 calls a minute (heard {heard})");
    }

    /// With FOREST_WAV=dir set, each creature is written out to listen to, and
    /// forty seconds of each weather's forest.
    #[test]
    fn wildlife_can_be_written_out_to_listen_to() {
        let Ok(dir) = std::env::var("FOREST_WAV") else {
            return;
        };
        std::fs::create_dir_all(&dir).unwrap();
        for kind in Kind::ALL {
            let mut out = one(kind);
            out.extend(std::iter::repeat_n(0.0, RATE as usize));
            let name = format!("{kind:?}").to_lowercase();
            crate::wav::write_stereo(&format!("{dir}/wildlife-{name}.wav"), &out, RATE as u32);
        }
        for (name, levels) in [
            ("clear", [0.0; 3]),
            ("rain", [1.0, 0.0, 0.0]),
            ("fog", [0.0, 1.0, 0.0]),
            ("mild", [0.0, 0.0, 1.0]),
        ] {
            let mut w = Wildlife::new(RATE);
            w.set(levels, 2.5, 1.0);
            let out: Vec<f32> = (0..RATE as usize * 40).flat_map(|_| w.next_frame()).collect();
            crate::wav::write_stereo(&format!("{dir}/forest-{name}.wav"), &out, RATE as u32);
        }
    }
}
