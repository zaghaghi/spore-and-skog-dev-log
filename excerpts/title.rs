// From the game's src/screens/title_weather.rs, as it ships: only the parts this devlog
// talks about. Copied by tools/sync.py; shown on the page, not compiled.

/// On the title, a creature calls every this many seconds or so, this loud,
/// and the weather is heard at this much of its strength: a bed under the menu.
const TITLE_CALLS: f32 = 4.5;
const TITLE_CALL_GAIN: f32 = 0.6;

/// Leaving the title, its weather falls quiet.
fn hush(sound: Res<Sound>) {
    sound.ambience([0.0; 3]);
    sound.wildlife([0.0; 3], 0.0, 1.0);
}
