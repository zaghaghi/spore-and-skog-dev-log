//! The forest, through CPAL, with no game around it: one stream, the desk in
//! its callback, and this thread telling it what weather it is.
//!
//!     cargo run -p forest-cpal --release            # a minute of each weather
//!     cargo run -p forest-cpal --release -- fog     # just the fog
//!     cargo run -p forest-cpal --release -- Owl     # one creature

use std::{sync::mpsc, thread, time::Duration};

use forest::{desk::Command, wildlife::Kind};

fn main() -> Result<(), String> {
    let (desk, commands) = mpsc::channel();
    // The stream plays for as long as it lives.
    let _stream = forest::device::open(commands)?;

    let weathers = [
        ("clear", [0.0, 0.0, 0.0]),
        ("rain", [1.0, 0.0, 0.0]),
        ("fog", [0.0, 1.0, 0.0]),
        ("mild", [0.0, 0.0, 1.0]),
    ];
    let arg = std::env::args().nth(1);
    if let Some(kind) = Kind::ALL.iter().find(|k| arg.as_deref() == Some(&format!("{k:?}"))) {
        println!("{kind:?}");
        desk.send(Command::Call(*kind)).ok();
        thread::sleep(Duration::from_secs(5));
        return Ok(());
    }
    for (name, levels) in weathers {
        if arg.as_ref().is_some_and(|a| a != name) {
            continue;
        }
        println!("{name}: a creature every 2.5 s or so");
        desk.send(Command::Wildlife { levels, every: 2.5, gain: 1.0 }).ok();
        thread::sleep(Duration::from_secs(if arg.is_some() { 60 } else { 20 }));
    }
    Ok(())
}
