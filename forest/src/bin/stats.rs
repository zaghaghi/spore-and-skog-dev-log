//! The article's numbers, and a fingerprint of each creature's call, as JSON:
//!
//!     cargo run -p forest --release --bin stats > site/stats.json
//!
//! `tools/sync.py` compares the fingerprints before and after a sync to say
//! which creatures the game changed.

use forest::wildlife::{Kind, VOICES};

fn main() {
    let rate = 48_000.0;
    let mut creatures = Vec::new();
    for kind in Kind::ALL {
        let call = forest::one_call(kind, 0, rate);
        // FNV-1a over the samples' bits: any change to the sound changes it.
        let hash = call.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, x| {
            (h ^ x.to_bits() as u64).wrapping_mul(0x100_0000_01b3)
        });
        creatures.push(format!(
            "    {{\"kind\": \"{kind:?}\", \"ms\": {:.0}, \"hash\": \"{hash:016x}\"}}",
            call.len() as f32 / 2.0 * 1000.0 / rate
        ));
    }
    let lines = include_str!("../wildlife.rs").lines().count();
    println!("{{");
    println!("  \"kinds\": {},", Kind::ALL.len());
    println!("  \"voices\": {VOICES},");
    println!("  \"lines\": {lines},");
    println!("  \"creatures\": [\n{}\n  ]", creatures.join(",\n"));
    println!("}}");
}
