#!/usr/bin/env python3
"""Bring the devlog up to date with the game.

    python3 tools/sync.py path/to/the/game

From the game's folder (the one with Cargo.toml and src/audio/) it takes:

  wildlife.rs   src/audio/wildlife.rs -> forest/src/wildlife.rs
                (as is, with `pub` added and its WAV test pointed at forest/src/wav.rs)
  excerpts      the few pieces of the game the article quotes, and nothing else:
                Sound::wildlife() and send(), the stream and its suspend from src/audio/mod.rs;
                the forest systems from src/audio/hooks.rs; the title's call rate
                -> excerpts/*.rs (shown on the page, not compiled)

then runs the tests, rebuilds the wasm and the page, and says which creatures
sound different from before. Pictures come from the itch.io page instead:
tools/itch.py. Nothing is committed: listen with tools/serve.sh, read
`git diff`, then commit and push.
"""

import datetime
import json
import pathlib
import re
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SITE = ROOT / "site"

# What the article quotes from the game: (file under src/, item headers).
EXCERPTS = {
    "excerpts/sound.rs": ("audio/mod.rs", [
        r"^    pub fn wildlife\b",
        r"^    fn send\b",
        r"^struct Stream\b",
        r"^fn suspend\b",
    ]),
    "excerpts/hooks.rs": ("audio/hooks.rs", [
        r"^const TABLE_CALLS\b",
        r"^const TABLE_GAIN\b",
        r"^const LOOK_CALLS\b",
        r"^const LOOK_GAIN\b",
        r"^fn forest\b",
        r"^fn leave_forest\b",
    ]),
    "excerpts/title.rs": ("screens/title_weather.rs", [
        r"^const TITLE_CALLS\b",
        r"^const TITLE_CALL_GAIN\b",
        r"^fn hush\b",
    ]),
}

WAV_TEST = '''    /// With FOREST_WAV=dir set, each creature is written out to listen to, and
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
    }'''

NOTE = """//!
//! This file is the game's `src/audio/wildlife.rs` as it ships, with `pub`
//! added so the devlog can call its parts one at a time. (Copied by
//! tools/sync.py: change it in the game, then sync.)

// The game's own names (`Noise::next`, `Svf::next`) kept, now that they're public.
#![allow(clippy::should_implement_trait)]
"""


def say(*a):
    print(*a, flush=True)


def run(*cmd, capture=False):
    say("  $", " ".join(str(c) for c in cmd))
    r = subprocess.run(cmd, cwd=ROOT, text=True, capture_output=capture)
    if r.returncode != 0:
        if capture:
            sys.stderr.write(r.stdout + r.stderr)
        raise SystemExit(f"\nStopped: {' '.join(str(c) for c in cmd)} failed. The synced files are in place; "
                         "see the message above, fix, and run the sync again.")
    return r.stdout if capture else None


def item_span(lines, header):
    """An item's lines, its doc comments and attributes too, by a regex on its first line."""
    for i, line in enumerate(lines):
        if not re.match(header, line):
            continue
        indent = re.match(r"\s*", line).group()
        start = i
        while start > 0 and lines[start - 1].strip().startswith(("///", "#[")):
            start -= 1
        end = i
        if not line.rstrip().endswith(";"):
            while lines[end] != indent + "}":
                end += 1
        return start, end + 1
    return None


def wildlife_rs(src: str, warnings) -> str:
    out = []
    for line in src.splitlines():
        line = re.sub(r"^(fn|struct|enum|const|static|type) ", r"pub \1 ", line)
        line = re.sub(r"^    fn ", "    pub fn ", line)
        # A tuple struct's field too: `pub struct Noise(pub u32);`
        line = re.sub(r"^pub struct (\w+)\((\w+)\);", r"pub struct \1(pub \2);", line)
        out.append(line)
    span = item_span(out, r"^    pub fn wildlife_can_be_written_out_to_listen_to\b")
    if span:
        out[span[0]:span[1]] = WAV_TEST.splitlines()
    else:
        warnings.append("wildlife.rs: the game's WAV-writing test is gone or renamed; the devlog's isn't in it now.")
    if "crate::" in "\n".join(out).replace("crate::wav", ""):
        warnings.append("wildlife.rs now uses more of the game (crate::...): bring that into forest/ too.")
    k = 0
    while k < len(out) and out[k].startswith("//!"):
        k += 1
    return "\n".join(out[:k]) + "\n" + NOTE + "\n".join(out[k:]) + "\n"


def excerpt(game: pathlib.Path, dst: str, src: str, headers, warnings) -> str:
    lines = (game / "src" / src).read_text().splitlines()
    parts = []
    last_end = None
    for h in headers:
        span = item_span(lines, h)
        if span is None:
            warnings.append(f"{src}: no {h.strip('^').strip(chr(92) + 'b')} any more: take it out of {dst} "
                            "and the article.")
            continue
        block = lines[span[0]:span[1]]
        indent = min((len(l) - len(l.lstrip()) for l in block if l.strip()), default=0)
        text = "\n".join(l[indent:] for l in block)
        # Items next to each other in the game stay together here.
        if parts and span[0] == last_end:
            parts[-1] += "\n" + text
        else:
            parts.append(text)
        last_end = span[1]
    head = (f"// From the game's src/{src}, as it ships: only the parts this devlog\n"
            f"// talks about. Copied by tools/sync.py; shown on the page, not compiled.\n\n")
    return head + "\n\n".join(parts) + "\n"


def write_if_changed(path: pathlib.Path, text: str) -> bool:
    if path.exists() and path.read_text() == text:
        return False
    path.write_text(text)
    return True


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    game = pathlib.Path(sys.argv[1]).expanduser().resolve()
    if not (game / "src/audio/wildlife.rs").exists():
        raise SystemExit(f"{game} doesn't look like the game's folder: no src/audio/wildlife.rs in it.")
    warnings = []
    before = (SITE / "stats.json").read_text() if (SITE / "stats.json").exists() else "{}"

    say(f"Syncing from {game}")
    changed = []
    if write_if_changed(ROOT / "forest/src/wildlife.rs",
                        wildlife_rs((game / "src/audio/wildlife.rs").read_text(), warnings)):
        changed.append("forest/src/wildlife.rs")
    for dst, (src, headers) in EXCERPTS.items():
        if write_if_changed(ROOT / dst, excerpt(game, dst, src, headers, warnings)):
            changed.append(dst)
    say("  updated: " + (", ".join(changed) if changed else "nothing, the code is as it was"))

    today = datetime.date.today()
    record_path = SITE / "game.json"
    record = json.loads(record_path.read_text()) if record_path.exists() else {}
    record.update({"synced": f"{today.day} {today:%B %Y}"})
    record.pop("commit", None)
    record_path.write_text(json.dumps(record, indent=2) + "\n")

    if warnings:
        say("\nTo look at by hand")
        for w in warnings:
            say("  - " + w)

    say("\nChecks")
    run("cargo", "test", "-p", "forest", "--release")
    run("cargo", "build", "-p", "forest", "--release", "--target", "wasm32-unknown-unknown")
    shutil.copy(ROOT / "target/wasm32-unknown-unknown/release/forest.wasm", ROOT / "docs/forest.wasm")
    (SITE / "stats.json").write_text(run("cargo", "run", "-q", "-p", "forest", "--release", "--bin", "stats",
                                         capture=True))
    run(sys.executable, "tools/build.py")

    say("\nWhat you'll hear differently")
    old = {c["kind"]: c for c in json.loads(before).get("creatures", [])}
    new = {c["kind"]: c for c in json.loads((SITE / "stats.json").read_text()).get("creatures", [])}
    report = []
    for k in new:
        if k not in old:
            report.append(f"  new creature   {k}  (the page lists it by itself; give it a few words in site/index.src.html)")
        elif old[k]["hash"] != new[k]["hash"]:
            report.append(f"  sounds different  {k}  {old[k]['ms']} -> {new[k]['ms']} ms")
    for k in old:
        if k not in new:
            report.append(f"  gone           {k}  (take its section out of site/index.src.html)")
    say("\n".join(report) if report else "  nothing: every creature calls sample for sample as before")
    say("\nNext: tools/serve.sh to listen, `git diff` to read, then commit and push.")


if __name__ == "__main__":
    main()
