#!/usr/bin/env python3
"""Build docs/index.html from site/index.src.html.

Every Rust snippet on the page is cut from the sources, never pasted, so the
article can't drift from the code it describes:

    {{item forest/src/wildlife.rs fn window}}          an item, with its doc comment
    {{item forest/src/wildlife.rs struct Svf + impl Svf}} several items, one figure
    {{arm forest/src/wildlife.rs 1 Kind::Owl}}          the 1st match arm starting
                                                         `Kind::Owl` (2nd: `2`)
    {{lines forest/src/wildlife.rs // Far ~ Some([}}    from one line to another
    {{file examples/forest-cpal/src/main.rs}}           a whole file
    {{gallery}}                                         the pictures tools/itch.py fetched

Code is folded behind a toggle; " || " adds its caption:

    {{item forest/src/wildlife.rs fn window || The bump, in Rust}}

and two helpers for the prose around it:

    {{src forest/src/wildlife.rs}}                      that file's address on GitHub
    {{analytics}}                                       GoatCounter, if site/analytics.json has a code
    {{recipe Owl | a sine | its notes | breath | far}}  one row of a recipe diagram

and the article's numbers come from data, not prose:

    {{stat kinds}}     site/stats.json (cargo run -p forest --release --bin stats)
    {{game synced}}    site/game.json  (written by tools/sync.py)

    python3 tools/build.py          write docs/index.html
    python3 tools/build.py --check  fail if docs/index.html is out of date
"""

import html
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "site" / "index.src.html"
OUT = ROOT / "docs" / "index.html"

KEYWORDS = set(
    "as break const continue else enum false fn for if impl in let loop match mod move mut pub "
    "ref return self Self static struct super trait true type unsafe use where while dyn crate".split()
)
TYPES = set("f32 f64 u8 u16 u32 u64 usize i16 i32 i64 bool str String Vec Option Some None Ok Err Result Box Arc".split())

TOKEN = re.compile(
    r"(?P<comment>//[^\n]*)"
    r"|(?P<string>b?\"(?:\\.|[^\"\\])*\")"
    r"|(?P<char>'(?:\\.|[^'\\])')"
    r"|(?P<attr>#!?\[[^\]]*\])"
    r"|(?P<number>\b\d[\d_]*(?:\.\d[\d_]*)?(?:e-?\d+)?(?:f32|f64|u32|usize|u64|i32)?\b|\b0x[0-9a-f_]+\b)"
    r"|(?P<macro>\b[a-z_][a-z0-9_]*!)"
    r"|(?P<word>\b[A-Za-z_][A-Za-z0-9_]*\b)"
    r"|(?P<other>.)",
    re.S,
)


def highlight(code: str) -> str:
    out = []
    for m in TOKEN.finditer(code):
        kind, text = m.lastgroup, m.group()
        esc = html.escape(text, quote=False)
        if kind == "word":
            if text in KEYWORDS:
                out.append(f'<span class="k">{esc}</span>')
            elif text in TYPES or text[0].isupper():
                out.append(f'<span class="t">{esc}</span>')
            else:
                out.append(esc)
        elif kind == "other":
            out.append(esc)
        else:
            cls = {"comment": "c", "string": "s", "char": "s", "attr": "a", "number": "n", "macro": "m"}[kind]
            out.append(f'<span class="{cls}">{esc}</span>')
    return "".join(out)


def item(path: str, kind: str, name: str) -> str:
    lines = (ROOT / path).read_text().splitlines()
    head = re.compile(rf"^(\s*)(pub )?{kind} {re.escape(name)}\b")
    for i, line in enumerate(lines):
        m = head.match(line)
        if not m:
            continue
        indent = m.group(1)
        start = i
        while start > 0 and lines[start - 1].strip().startswith(("///", "#[")):
            start -= 1
        end = i
        if not line.rstrip().endswith((";", "}")) or line.rstrip().endswith("{"):
            while lines[end] != indent + "}":
                end += 1
        block = lines[start : end + 1]
        return "\n".join(l[len(indent) :] if l.startswith(indent) else l for l in block)
    raise SystemExit(f"{path}: no {kind} {name}")


def data(name: str) -> dict:
    path = ROOT / "site" / f"{name}.json"
    return json.loads(path.read_text()) if path.exists() else {}


def arm(path: str, nth: str, start: str) -> str:
    """The `nth` match arm whose first line starts with `start` (`Kind::Song`
    also finds `Kind::Song | Kind::Cuckoo | Kind::Owl => {`)."""
    lines = (ROOT / path).read_text().splitlines()
    seen = 0
    for i, line in enumerate(lines):
        if not (line.strip().startswith(start) and line.rstrip().endswith("{")):
            continue
        seen += 1
        if seen != int(nth):
            continue
        indent = line[: len(line) - len(line.lstrip())]
        start_i = i
        while lines[start_i - 1].strip().startswith("//"):
            start_i -= 1
        end = i
        while lines[end].rstrip() not in (indent + "}", indent + "},"):
            end += 1
        block = lines[start_i : end + 1]
        return "\n".join(l[len(indent) :] if l.startswith(indent) else l for l in block)
    raise SystemExit(f"{path}: no match arm {nth} starting {start}")


def gallery() -> str:
    """The game's pictures, as fetched from its itch.io page by tools/itch.py."""
    path = ROOT / "docs/img/itch/manifest.json"
    if not path.exists():
        return "<!-- No pictures yet: run python3 tools/itch.py, then this again. -->"
    pics = json.loads(path.read_text())["pictures"]
    alt = {"banner": "The banner of Spore & Skog: The Divine Forage",
           "cover": "Spore & Skog: The Divine Forage",
           "screenshot": "Spore & Skog: The Divine Forage, in play"}

    def img(p, cls=""):
        size = f' width="{p["width"]}" height="{p["height"]}"' if p.get("width") else ""
        c = f' class="{cls}"' if cls else ""
        return f'<img{c} src="{html.escape(p["file"])}"{size} loading="lazy" alt="{alt[p["kind"]]}">'

    banner = [p for p in pics if p["kind"] == "banner"]
    shots = [p for p in pics if p["kind"] == "screenshot"] or [p for p in pics if p["kind"] == "cover"]
    out = [img(banner[0], "banner")] if banner else []
    if shots:
        out.append('<div class="media">' + "".join(img(p) for p in shots) + "</div>")
    return "\n".join(out)


REPO = "https://github.com/zaghaghi/spore-and-skog-dev-log/blob/main/"


STAGES = ["Source", "Shape in time", "Filter", "Place"]


def recipe(cells) -> str:
    """One row of a recipe diagram: a name, then its stages in order."""
    name, *stages = cells
    # Each stage carries its column's name, for narrow screens where the header row is hidden.
    steps = "".join(f'<li data-step="{col}">{html.escape(st)}</li>' for col, st in zip(STAGES, stages))
    return f'<div class="recipe"><b class="recipe-name">{html.escape(name)}</b><ol>{steps}</ol></div>'


def snippet(match: re.Match) -> str:
    whole = match.group(1)
    # A caption for the folded code, after "||": {{item path fn x || what it is}}
    whole, _, caption = whole.partition(" || ")
    parts = whole.split()
    if parts[0] == "gallery":
        return gallery()
    if parts[0] == "analytics":
        # GoatCounter (no cookies), only once a site code is set in site/analytics.json.
        code = data("analytics").get("goatcounter", "")
        if not code:
            return ""
        return (f'<script data-goatcounter="https://{html.escape(code)}.goatcounter.com/count" '
                'async src="https://gc.zgo.at/count.js"></script>')
    if parts[0] == "analytics-note":
        if not data("analytics").get("goatcounter", ""):
            return ""
        return ('<p>Visits are counted with <a href="https://www.goatcounter.com">GoatCounter</a>: '
                'no cookies, nothing personal.</p>')
    if parts[0] == "src":
        return REPO + parts[1]
    if parts[0] == "recipehead":
        return recipe(["", *STAGES]).replace('class="recipe"', 'class="recipe recipe-head" aria-hidden="true"', 1)
    if parts[0] == "recipe":
        return recipe([c.strip() for c in " ".join(parts[1:]).split(" | ")])
    if parts[0] in ("stat", "game"):
        values = data("stats" if parts[0] == "stat" else "game")
        if parts[1] not in values:
            raise SystemExit(f"site/{'stats' if parts[0] == 'stat' else 'game'}.json has no {parts[1]}")
        return html.escape(str(values[parts[1]]))
    if parts[0] == "item":
        # Several items from one file: {{item path struct Low + impl Low}}
        path = parts[1]
        items = " ".join(parts[2:]).split(" + ")
        code = ""
        for i in items:
            kind = i.split()[0]
            # Constants side by side stay together; anything else gets a blank line.
            sep = "" if not code else "\n" if kind == "const" and last == "const" else "\n\n"
            code += sep + item(path, *i.split(None, 1))
            last = kind
    elif parts[0] == "lines":
        # {{lines path FIRST ~ LAST}}: from the line holding FIRST to the next holding LAST.
        path = parts[1]
        first, last = (x.strip() for x in " ".join(parts[2:]).split("~"))
        lines = (ROOT / path).read_text().splitlines()
        a = next(i for i, l in enumerate(lines) if first in l)
        b = next(i for i in range(a, len(lines)) if last in lines[i])
        block = lines[a : b + 1]
        indent = min(len(l) - len(l.lstrip()) for l in block if l.strip())
        code = "\n".join(l[indent:] for l in block)
    elif parts[0] == "arm":
        path = parts[1]
        code = arm(path, parts[2], " ".join(parts[3:]))
    elif parts[0] == "file":
        path = parts[1]
        code = (ROOT / path).read_text().rstrip("\n")
    else:
        raise SystemExit(f"unknown snippet {match.group(0)}")
    # Folded by default: the text reads on without it, the code is a click away.
    n = code.count("\n") + 1
    label = html.escape(caption) if caption else "The code"
    return (
        f'<details class="code"><summary><span class="code-what">{label}</span>'
        f'<span class="code-meta">Rust · {n} lines</span></summary>'
        f'<div class="code-body"><p class="code-file"><a href="{REPO}{path}">{html.escape(path)}</a></p>'
        f'<pre><code>{highlight(code)}</code></pre></div></details>'
    )


def build() -> str:
    text = SRC.read_text()
    return re.sub(r"\{\{(.+?)\}\}", snippet, text)


if __name__ == "__main__":
    page = build()
    if "--check" in sys.argv:
        if not OUT.exists() or OUT.read_text() != page:
            raise SystemExit("docs/index.html is out of date: run python3 tools/build.py")
        print("docs/index.html is up to date")
    else:
        OUT.write_text(page)
        print(f"wrote {OUT.relative_to(ROOT)}")
