//! The forest in a Bevy app, wired the way Spore & Skog wires it:
//!
//! - the sound lives outside Bevy (no `bevy_audio`): one CPAL stream, opened
//!   at start-up and kept as a non-send resource;
//! - systems reach it through a `Sound` resource holding the channel's sender;
//! - one system works out what the forest should sound like from the game's
//!   state, and tells the desk only when that changes.
//!
//! Here the "game state" is a sky you set with the keys:
//! 1 rain, 2 fog, 3 mild, 0 clear, Space to lean in close, M for a menu.
//!
//!     cargo run -p bevy-forest --release

use std::sync::mpsc::{self, Sender};

use bevy::{prelude::*, window::AppLifecycle};
use forest::desk::Command;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "A forest".into(),
                resolution: (900u32, 520u32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(SoundPlugin)
        .init_resource::<Sky>()
        .add_systems(Startup, setup)
        .add_systems(Update, (keys, forest, show).chain())
        .run();
}

// ---- the sound: outside Bevy, reached through a resource -------------------------

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Sound>()
            .add_systems(Startup, open)
            .add_systems(Update, suspend);
    }
}

/// The way to the desk; silent when there is no audio device.
#[derive(Resource, Default)]
pub struct Sound {
    desk: Option<Sender<Command>>,
}

impl Sound {
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
}

/// The stream lives here: dropping it would stop the sound.
struct Stream(cpal::Stream);

/// Open the stream at start-up. An exclusive system: the stream isn't
/// `Send`, so it goes in as a non-send resource.
fn open(world: &mut World) {
    let (tx, rx) = mpsc::channel();
    match forest::device::open(rx) {
        Ok(stream) => world.insert_non_send(Stream(stream)),
        Err(e) => {
            warn!("no sound: {e}");
            return;
        }
    }
    world.insert_resource(Sound { desk: Some(tx) });
}

/// In the background (an iPad's home screen) the stream stops, and the
/// synthesis with it; it starts again when the app comes back.
fn suspend(mut lifecycle: MessageReader<AppLifecycle>, stream: Option<NonSend<Stream>>) {
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

// ---- the "game": a sky, set from the keyboard --------------------------------------

/// Stands in for the game's state: the weather over the forest, whether you
/// are looking closely at it, and whether a menu is open.
#[derive(Resource, Default)]
struct Sky {
    levels: [f32; 3],
    close: bool,
    menu: bool,
}

fn keys(input: Res<ButtonInput<KeyCode>>, mut sky: ResMut<Sky>) {
    for (key, levels) in [
        (KeyCode::Digit1, [1.0, 0.0, 0.0]),
        (KeyCode::Digit2, [0.0, 1.0, 0.0]),
        (KeyCode::Digit3, [0.0, 0.0, 1.0]),
        (KeyCode::Digit0, [0.0, 0.0, 0.0]),
    ] {
        if input.just_pressed(key) {
            sky.levels = levels;
        }
    }
    if input.just_pressed(KeyCode::Space) {
        sky.close = !sky.close;
    }
    if input.just_pressed(KeyCode::KeyM) {
        sky.menu = !sky.menu;
    }
}

/// At the table, a creature calls every this many seconds or so, this loud;
/// looking closely, more often and nearer. (The game's own numbers.)
const TABLE_CALLS: f32 = 9.0;
const TABLE_GAIN: f32 = 0.7;
const LOOK_CALLS: f32 = 3.0;
const LOOK_GAIN: f32 = 0.9;

/// The forest heard: its creatures suit the weather, come closer when you
/// look, and hush behind a menu. Runs every frame, tells the desk only when
/// something changed (the `Local` remembers what it last said).
fn forest(sky: Res<Sky>, mut told: Local<Option<([f32; 3], bool, bool)>>, sound: Res<Sound>) {
    let on = !sky.menu;
    let now = (sky.levels, on, sky.close);
    if *told == Some(now) {
        return;
    }
    *told = Some(now);
    let (every, gain) = if sky.close {
        (LOOK_CALLS, LOOK_GAIN)
    } else {
        (TABLE_CALLS, TABLE_GAIN)
    };
    sound.wildlife(sky.levels, if on { every } else { 0.0 }, gain);
}

// ---- something to look at ------------------------------------------------------------

#[derive(Component)]
struct Status;

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        Text("".into()),
        TextFont {
            font_size: FontSize::Px(26.0),
            ..default()
        },
        TextColor(Color::srgb_u8(0x1a, 0x1a, 0x1a)),
        Node {
            margin: UiRect::all(Val::Px(40.0)),
            ..default()
        },
        Status,
    ));
}

fn show(sky: Res<Sky>, mut text: Single<&mut Text, With<Status>>, mut clear: ResMut<ClearColor>) {
    if !sky.is_changed() {
        return;
    }
    let [rain, fog, mild] = sky.levels;
    let weather = if rain > 0.0 {
        "rain"
    } else if fog > 0.0 {
        "fog"
    } else if mild > 0.0 {
        "mild"
    } else {
        "a clear sky"
    };
    text.0 = format!(
        "The forest, in {weather}{}{}\n\n1 rain   2 fog   3 mild   0 clear\nSpace: lean in   M: a menu",
        if sky.close { ", close up" } else { "" },
        if sky.menu { ", behind a menu (hushed)" } else { "" },
    );
    // Paper, tinted by the weather (the game's Okabe-Ito colours).
    let paper = Vec3::new(0.94, 0.91, 0.85);
    let tint = Vec3::new(0.0, 0.45, 0.70) * rain + Vec3::new(0.80, 0.47, 0.65) * fog + Vec3::new(0.90, 0.62, 0.0) * mild;
    let c = paper.lerp(tint, if tint == Vec3::ZERO { 0.0 } else { 0.25 });
    clear.0 = Color::srgb(c.x, c.y, c.z);
}
