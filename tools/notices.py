#!/usr/bin/env python3
"""Rebuilds THIRD-PARTY-NOTICES.md from the crates that ship in the Mac, Linux and Windows builds.

Run from the repository root:  python3 tools/notices.py
"""
import json, subprocess

TARGETS = ["aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "x86_64-pc-windows-msvc"]

meta = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1"]))
by_id = {f"{p['name']} v{p['version']}": p for p in meta["packages"]}
shipped = set()
for t in TARGETS:
    out = subprocess.check_output(
        ["cargo", "tree", "--target", t, "--prefix", "none", "-e", "normal,build", "--format", "{p}"], text=True)
    for line in out.splitlines():
        parts = line.split()
        if len(parts) >= 2:
            shipped.add(f"{parts[0]} {parts[1]}")

rows = sorted((by_id[k] for k in shipped if k in by_id and by_id[k]["name"] != "cleanerio"),
              key=lambda p: (p["name"].lower(), p["version"]))
lines = [
    "# Third-party notices",
    "",
    "Cleanerio itself is released under the MIT License (see `LICENSE`).",
    "The Mac, Linux and Windows builds also contain the open-source Rust crates listed below, each",
    "under its own license. Where a crate offers a choice of licenses, Cleanerio",
    "uses it under the MIT License, or under Apache-2.0 when MIT is not offered.",
    "The full text of each license is in the crate's repository, linked in the table.",
    "",
    "Two crates are not under a permissive license of that kind: `option-ext`",
    "(MPL-2.0, used unmodified; its source is at the link in the table) and",
    "`self_cell` (used under Apache-2.0).",
    "",
    "## Fonts",
    "",
    "The interface text is drawn with the fonts bundled in `epaint_default_fonts`",
    "(part of egui): Ubuntu Light (Ubuntu Font Licence 1.0), Hack (MIT),",
    "Noto Emoji (SIL Open Font License 1.1) and emoji-icon-font (MIT).",
    "",
    "## Crates",
    "",
    f"{len(rows)} crates. Regenerate this file with `python3 tools/notices.py`.",
    "",
    "| Crate | Version | License | Source |",
    "|---|---|---|---|",
]
for p in rows:
    lic = (p.get("license") or "see repository").replace("|", "/")
    repo = p.get("repository") or f"https://crates.io/crates/{p['name']}"
    lines.append(f"| {p['name']} | {p['version']} | {lic} | {repo} |")
open("THIRD-PARTY-NOTICES.md", "w").write("\n".join(lines) + "\n")
print(len(rows), "crates")
