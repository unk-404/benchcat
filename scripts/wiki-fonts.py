#!/usr/bin/env python3
"""Render the GitHub wiki "Fonts" page from the committed specimens.

The images stay in the main repository; the wiki page only links their raw URLs,
so there is a single copy of every SVG and CI can regenerate the page at will.
"""

from __future__ import annotations

import argparse
import pathlib
import re

# Human names for the catalogue; unknown stems fall back to the file stem.
NAMES = {
    "comfortaa": "Comfortaa",
    "gaegu": "Gaegu",
    "jetbrains-mono": "JetBrains Mono",
    "lato": "Lato",
    "mplus-rounded": "M PLUS Rounded 1c",
    "noto-sans": "Noto Sans",
    "nunito": "Nunito",
    "quicksand": "Quicksand",
    "rubik": "Rubik",
    "sniglet": "Sniglet",
    "space-grotesk": "Space Grotesk",
    "zcool-kuaile": "ZCOOL KuaiLe",
    "zen-maru-gothic": "Zen Maru Gothic",
}


def display_name(stem: str) -> str:
    return NAMES.get(stem, re.sub(r"[-_]+", " ", stem).title())


def image_url(repo: str, branch: str, filename: str) -> str:
    return f"https://raw.githubusercontent.com/{repo}/{branch}/assets/fonts/{filename}"


def build_page(repo: str, branch: str, specimens: pathlib.Path) -> str:
    files = sorted(specimens.glob("*.svg"))
    lines = [
        "# Fonts",
        "",
        "The OFL type catalogue shipped with benchcat. Every specimen is generated from the font file",
        "by `cargo run --example specimen`; this page is refreshed by CI.",
        "",
    ]
    for path in files:
        name = display_name(path.stem)
        lines += [
            f"## {name}",
            "",
            f"![{name}]({image_url(repo, branch, path.name)})",
            "",
        ]
    if not files:
        lines.append("_No specimens are committed yet._")
    return "\n".join(lines).rstrip("\n") + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True, help="owner/repository")
    parser.add_argument("--branch", required=True, help="branch holding assets/fonts")
    parser.add_argument("--specimens", default="assets/fonts", type=pathlib.Path)
    parser.add_argument("--out", required=True, type=pathlib.Path)
    args = parser.parse_args()
    page = build_page(args.repo, args.branch, args.specimens)
    args.out.write_text(page, encoding="utf-8", newline="\n")
    print(f"wrote {args.out} ({len(page)} bytes, {page.count(chr(10) + '## ')} fonts)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
