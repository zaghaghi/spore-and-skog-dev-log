// From the game's src/audio/hooks.rs, as it ships: only the parts this devlog
// talks about. Copied by tools/sync.py; shown on the page, not compiled.

/// At the table, a creature calls every this many seconds or so, this loud: the
/// forest is about, not in the way.
const TABLE_CALLS: f32 = 9.0;
const TABLE_GAIN: f32 = 0.7;
/// Looking at a forest card, its creatures call this often, this loud.
const LOOK_CALLS: f32 = 3.0;
const LOOK_GAIN: f32 = 0.9;

/// The forest heard at the table: its creatures suit the weather set over the
/// three forests (as the land shows it), and hush behind a menu and once the
/// game is over. Looking at one forest's card, they are that forest's, and
/// closer.
fn forest(
    app: Res<State<AppState>>,
    overlay: Res<State<Overlay>>,
    game: Option<Res<Match>>,
    shown: Option<Res<Shown>>,
    peek: Res<Peek>,
    mut told: Local<Option<([f32; 3], bool, bool)>>,
    sound: Res<Sound>,
) {
    let (Some(game), Some(shown)) = (game, shown) else {
        return;
    };
    let looking = match peek.look {
        Some(Look::Forest(f)) => shown.state.forests.get(f),
        _ => None,
    };
    let mut levels = [0.0; 3];
    if let Some(forest) = looking {
        levels = crate::table::land::amounts(forest);
    } else {
        for forest in &shown.state.forests {
            for (l, a) in levels.iter_mut().zip(crate::table::land::amounts(forest)) {
                *l += a / 3.0;
            }
        }
    }
    let on = !game.demo && !shown.state.over && *overlay.get() == Overlay::None;
    let close = looking.is_some();
    // Back at the table, tell it afresh: leaving hushed it.
    let same = !app.is_changed()
        && told.is_some_and(|(was, was_on, was_close)| {
            was_on == on
                && was_close == close
                && was.iter().zip(levels).all(|(a, b)| (a - b).abs() < 0.01)
        });
    if !same {
        *told = Some((levels, on, close));
        let (every, gain) = if close {
            (LOOK_CALLS, LOOK_GAIN)
        } else {
            (TABLE_CALLS, TABLE_GAIN)
        };
        sound.wildlife(levels, if on { every } else { 0.0 }, gain);
    }
}

/// Leaving the table, its creatures fall quiet (the title sets its own), and
/// the drums and pad go back to their usual balance.
fn leave_forest(sound: Res<Sound>) {
    sound.wildlife([0.0; 3], 0.0, 1.0);
    sound.blend(1.0, 1.0);
}
