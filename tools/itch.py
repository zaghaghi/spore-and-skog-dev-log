#!/usr/bin/env python3
"""Fetch the game's pictures from its itch.io page into docs/img/itch/.

    python3 tools/itch.py [https://xaghoul.itch.io/spore-and-skog]

Takes the page's banner, its cover (the og:image) and every screenshot,
GIFs included, at full size, and writes docs/img/itch/manifest.json, from
which tools/build.py lays out the page's gallery. Stills are converted to
WebP when ffmpeg is about (smaller); GIFs are kept as they are. Run it again
whenever the itch.io page changes, then python3 tools/build.py.
"""

import html
import json
import pathlib
import re
import shutil
import subprocess
import sys
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "docs/img/itch"
PAGE = "https://xaghoul.itch.io/spore-and-skog"
AGENT = {"User-Agent": "Mozilla/5.0 (devlog image fetch)"}


def get(url: str) -> bytes:
    with urllib.request.urlopen(urllib.request.Request(url, headers=AGENT), timeout=30) as r:
        return r.read()


def pictures(page: str) -> list:
    found = []

    def add(url, kind):
        url = html.unescape(url)
        if url.startswith("//"):
            url = "https:" + url
        if url not in [f["url"] for f in found]:
            found.append({"url": url, "kind": kind})

    # The banner: the header's image.
    m = re.search(r'<div[^>]*id="header"[^>]*>.*?<img[^>]*src="([^"]+)"', page, re.S)
    if m:
        add(m.group(1), "banner")
    # Screenshots, full size: the links around the thumbnails.
    block = re.search(r'class="screenshot_list"(.*?)</div>', page, re.S)
    for url in re.findall(r'<a[^>]*href="([^"]+)"', block.group(1) if block else ""):
        add(url, "screenshot")
    # The cover, as shared.
    m = re.search(r'<meta[^>]*property="og:image"[^>]*content="([^"]+)"', page)
    if m:
        add(m.group(1), "cover")
    return found


def main():
    page_url = sys.argv[1] if len(sys.argv) > 1 else PAGE
    page = get(page_url).decode("utf-8", "replace")
    found = pictures(page)
    if not found:
        raise SystemExit("Found no pictures on the page: has its layout changed?")
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir(parents=True)
    ffmpeg = shutil.which("ffmpeg")
    manifest = []
    counts = {}
    for f in found:
        counts[f["kind"]] = counts.get(f["kind"], 0) + 1
        ext = pathlib.Path(f["url"].split("?")[0]).suffix.lower() or ".png"
        name = f"{f['kind']}-{counts[f['kind']]:02d}"
        raw = OUT / f"{name}{ext}"
        raw.write_bytes(get(f["url"]))
        if ext in (".png", ".jpg", ".jpeg") and ffmpeg:
            webp = OUT / f"{name}.webp"
            r = subprocess.run([ffmpeg, "-y", "-loglevel", "error", "-i", str(raw), "-frames:v", "1",
                                "-vf", "scale='min(1400,iw)':-2", "-c:v", "libwebp", "-quality", "82", str(webp)])
            if r.returncode == 0:
                raw.unlink()
                raw = webp
        ffprobe = shutil.which("ffprobe")
        size = (subprocess.run([ffprobe, "-v", "error", "-select_streams", "v:0", "-show_entries",
                               "stream=width,height", "-of", "csv=p=0", str(raw)], capture_output=True,
                              text=True).stdout.split() or [""])[0] if ffprobe else ""
        w, h = (size.split(",") + ["", ""])[:2] if size else ("", "")
        manifest.append({"file": f"img/itch/{raw.name}", "kind": f["kind"], "width": w, "height": h,
                         "source": f["url"]})
        print(f"  {f['kind']:<10} {raw.name}")
    (OUT / "manifest.json").write_text(json.dumps({"page": page_url, "pictures": manifest}, indent=2) + "\n")
    print(f"{len(manifest)} pictures in docs/img/itch. Now: python3 tools/build.py")


if __name__ == "__main__":
    main()
