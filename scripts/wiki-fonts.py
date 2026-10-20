#!/usr/bin/env python3
"""Render the GitHub wiki "Fonts" page from the committed specimens.

The catalogue metadata lives in `fonts/catalogue.json`; the images stay in the
main repository and are linked by raw URL, so there is a single copy of every
SVG and CI can regenerate the page at will.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys


def format_size(size: int) -> str:
    if size >= 1_000_000:
        value = size / 1_000_000
        return f"{value:.1f} MB"
    return f"{round(size / 1_000)} KB"


def image_url(repo: str, branch: str, filename: str) -> str:
    return f"https://raw.githubusercontent.com/{repo}/{branch}/assets/fonts/{filename}"


def load_catalogue(path: pathlib.Path) -> list[dict]:
    data = json.loads(path.read_text(encoding="utf-8"))
    fonts = data.get("fonts")
    if not isinstance(fonts, list) or not fonts:
        raise SystemExit(f"error: {path} has no fonts list")
    return fonts


def build_page(
    repo: str,
    branch: str,
    specimens: pathlib.Path,
    fonts_dir: pathlib.Path,
    catalogue_path: pathlib.Path,
    allow_missing_specimens: bool,
) -> str:
    entries = load_catalogue(catalogue_path)
    errors: list[str] = []
    by_file: dict[str, dict] = {}
    for entry in entries:
        filename = entry.get("file", "")
        if not filename:
            errors.append("catalogue entry without a file")
            continue
        by_file[filename] = entry
        if not (fonts_dir / filename).is_file():
            errors.append(f"{filename}: listed in the catalogue but missing from {fonts_dir}")

    font_files = {
        path.name
        for path in fonts_dir.iterdir()
        if path.suffix.lower() in {".ttf", ".otf"} and path.is_file()
    }
    for filename in sorted(font_files - by_file.keys()):
        errors.append(f"{filename}: present in {fonts_dir} but missing from {catalogue_path}")

    committed = {path.stem: path for path in specimens.glob("*.svg")}
    for filename in by_file:
        stem = pathlib.Path(filename).stem
        if stem not in committed and not allow_missing_specimens:
            errors.append(f"{filename}: no specimen at {specimens / (stem + '.svg')}")
    if not allow_missing_specimens:
        for stem, path in sorted(committed.items()):
            if not any(pathlib.Path(entry["file"]).stem == stem for entry in entries):
                errors.append(f"{path}: specimen has no catalogue entry")

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)

    lines = [
        "# Fonts",
        "",
        "The OFL type catalogue shipped with benchcat. Pick a family with `[font] family` or",
        "`--font`; every specimen is generated from the font file by `cargo run --example specimen`.",
        f"The same catalogue is available as [benchcat-fonts.7z](https://github.com/{repo}/releases/tag/fonts-latest).",
        "",
        "| Family | File | Covers | Size |",
        "|---|---|---|---|",
    ]
    for entry in entries:
        filename = entry["file"]
        name = entry.get("name", pathlib.Path(filename).stem)
        if entry.get("default"):
            name += " (default)"
        size = format_size((fonts_dir / filename).stat().st_size)
        lines.append(
            f"| {name} | `{filename}` | {entry.get('covers', '—')} | {size} |"
        )
    lines.append("")

    for entry in entries:
        filename = entry["file"]
        stem = pathlib.Path(filename).stem
        path = committed.get(stem)
        if path is None:
            continue
        name = entry.get("name", stem)
        lines += [
            f"## {name}",
            "",
            f"![{name}]({image_url(repo, branch, path.name)})",
            "",
        ]
    return "\n".join(lines).rstrip("\n") + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True, help="owner/repository")
    parser.add_argument("--branch", required=True, help="branch holding assets/fonts")
    parser.add_argument("--specimens", default="assets/fonts", type=pathlib.Path)
    parser.add_argument("--fonts", default="fonts", type=pathlib.Path)
    parser.add_argument("--catalogue", default="fonts/catalogue.json", type=pathlib.Path)
    parser.add_argument("--out", required=True, type=pathlib.Path)
    parser.add_argument(
        "--allow-missing-specimens",
        action="store_true",
        help="skip the 'every font has a specimen' check (pre-assets dry run)",
    )
    args = parser.parse_args()
    page = build_page(
        args.repo,
        args.branch,
        args.specimens,
        args.fonts,
        args.catalogue,
        args.allow_missing_specimens,
    )
    args.out.write_text(page, encoding="utf-8", newline="\n")
    print(f"wrote {args.out} ({len(page)} bytes, {page.count(chr(10) + '## ')} fonts)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
