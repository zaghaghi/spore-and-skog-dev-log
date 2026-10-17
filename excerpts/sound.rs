// From the game's src/audio/mod.rs, as it ships: only the parts this devlog
// talks about. Copied by tools/sync.py; shown on the page, not compiled.

/// The creatures about: they suit the weather (rain, fog, mild, 0 to 1
/// each); one calls every `every` seconds or so (0: none), at `gain`.
pub fn wildlife(&self, levels: [f32; 3], every: f32, gain: f32) {
    self.send(Command::Wildlife {
        levels,
        every,
        gain,
    });
}

fn send(&self, command: Command) {
    if let Some(desk) = &self.desk {
        let _ = desk.send(command);
    }
}

/// The stream lives here: dropping it would stop the sound.
struct Stream(cpal::Stream);

/// In the background (the iPad's home screen) the stream stops, and the
/// synthesis with it; it starts again when the game comes back.
fn suspend(
    mut lifecycle: MessageReader<bevy::window::AppLifecycle>,
    stream: Option<NonSend<Stream>>,
) {
    use bevy::window::AppLifecycle;
    use cpal::traits::StreamTrait;
    let Some(stream) = stream else {
        lifecycle.clear();
        return;
    };
    for event in lifecycle.read() {
        let result = match event {
            AppLifecycle::WillSuspend | AppLifecycle::Suspended => stream.0.pause(),
            AppLifecycle::WillResume | AppLifecycle::Running => stream.0.play(),
            AppLifecycle::Idle => Ok(()),
        };
        if let Err(e) = result {
            warn!("sound stream: {e}");
        }
    }
}
