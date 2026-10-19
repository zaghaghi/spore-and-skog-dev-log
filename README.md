# A forest never recorded

Eight forest creatures (songbirds, a cuckoo, a woodpecker, a bumblebee, crickets, frogs, a
tawny owl, ravens) synthesized live in plain Rust: no samples, no recordings, every call made
a sample at a time inside an audio callback. Played through [CPAL](https://github.com/RustAudio/cpal),
and wired into a [Bevy](https://bevyengine.org) app.

This code is open-sourced from the card game
**[Spore & Skog: The Divine Forage](https://xaghoul.itch.io/spore-and-skog)**, where this forest
plays behind the cards and follows the weather on the table.

**The devlog, which you can listen to in the browser: https://zaghaghi.github.io/spore-and-skog-dev-log/**

Play the game: [itch.io](https://xaghoul.itch.io/spore-and-skog), or the
[TestFlight beta](https://testflight.apple.com/join/PXy2phyZ) on iPad and Mac.

## Try it

```sh
cargo run -p forest-cpal --release            # a forest in each weather, through your speakers
cargo run -p forest-cpal --release -- fog     # just the fog
cargo run -p forest-cpal --release -- Owl     # one creature
cargo run -p bevy-forest --release            # the Bevy integration: 1/2/3/0 weather, Space, M
FOREST_WAV=/tmp/forest cargo test -p forest   # every creature, and each weather, as WAV files
```

On Linux, CPAL needs the ALSA headers and Bevy a few more:

```sh
sudo apt-get install libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev
```

## Using the forest

`forest` has no dependencies (CPAL only with its `cpal` feature). The pieces:

| | |
|---|---|
| `forest::wildlife::Wildlife` | The forest itself. `set(levels, every, gain)` tells it the weather (rain, fog, mild, 0 to 1 each), how often a creature calls (mean seconds, 0 for none) and how loud; `next_frame()` returns the next stereo frame. `call(kind)` calls one up now. |
| `forest::desk::Desk` | What goes in the audio callback: owns a `Wildlife`, takes `Command`s from an `mpsc` channel at the start of each buffer, and fills interleaved buffers of any channel count. |
| `forest::device::open` | (feature `cpal`) Opens the default output with a `Desk` in its callback, for any sample format the device wants. Keep the returned `cpal::Stream` alive. |
| `forest::one_call` | Renders one call offline, for pictures or tests. |

```rust
let (tx, rx) = std::sync::mpsc::channel();
let _stream = forest::device::open(rx)?;           // sound until this is dropped
tx.send(forest::desk::Command::Wildlife {
    levels: [0.0, 1.0, 0.0],                       // fog: owls and ravens
    every: 3.0,                                    // a call every 3 s or so
    gain: 1.0,
})?;
```

In Bevy, keep the stream as a non-send resource and the sender in a resource of your own;
`examples/bevy-forest` shows the whole pattern in one file.

The creatures are quiet on purpose (they sit under a game's music). Alone, raise the desk's
level with `Command::Level`; the desk's soft limiter keeps the peaks in range.

## What's here

| Path | What |
|---|---|
| `forest/src/wildlife.rs` | The creatures: who calls when, and every sample of every call |
| `forest/src/desk.rs` | The mixer in the audio callback, and its `Command`s |
| `forest/src/device.rs` | The CPAL stream |
| `forest/src/wasm.rs` | Plain `extern "C"` exports for the devlog page (no wasm-bindgen) |
| `examples/forest-cpal` | The forest with no game around it |
| `examples/bevy-forest` | The forest in a Bevy app, wired as the game wires it |
| `excerpts/` | The few pieces of the game the article quotes (not compiled) |
| `site/index.src.html` | The article; `tools/build.py` cuts its code from the sources into `docs/index.html` |
| `docs/` | The published page |

`forest/src/wildlife.rs` and `excerpts/` are copied from the game with `tools/sync.py`; changes
to them belong upstream. The pictures come from the game's itch.io page (`tools/itch.py`).

## Building the page

```sh
rustup target add wasm32-unknown-unknown
tools/serve.sh       # builds forest.wasm and the page, serves it at http://localhost:8000
```

Every code block on the page is cut from the sources by `tools/build.py`, and CI fails if the
page is out of date with them. `.github/workflows/pages.yml` tests, builds and publishes to
GitHub Pages on every push to `main`.

## Analytics

The page counts visits with [GoatCounter](https://www.goatcounter.com) (no cookies), plus a few
events: the play and beta buttons, stepping into the forest, playing a creature. It's off until a
site code is set in `site/analytics.json` (`{"goatcounter": "your-code"}`); then rebuild with
`python3 tools/build.py`.

## Written with Claude

This code was written with the help of [Claude](https://claude.ai), Anthropic's AI assistant. We
decided not to put `noreply@anthropic.com` as a co-author on every commit, so this note stands in
for those trailers: assume Claude had a hand in any of it. `.claude/settings.json` turns the
trailers off for Claude Code sessions in this repository.

## Licence

MIT. The game's name and pictures belong to the game, and appear here to introduce it.
