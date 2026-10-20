#!/usr/bin/env python3
"""Render the GitHub wiki "Sources" page from assets/sources/catalogue.json."""

from __future__ import annotations

import argparse
import json
import pathlib
import sys


def image_url(repo: str, branch: str, image: str) -> str:
    return f"https://raw.githubusercontent.com/{repo}/{branch}/assets/sources/{image}"


def blob_url(repo: str, branch: str, path: str) -> str:
    return f"https://github.com/{repo}/blob/{branch}/{path}"


def load_catalogue(path: pathlib.Path) -> list[dict]:
    data = json.loads(path.read_text(encoding="utf-8"))
    sources = data.get("sources")
    if not isinstance(sources, list) or not sources:
        raise SystemExit(f"error: {path} has no sources list")
    return sources


def build_page(
    repo: str,
    branch: str,
    catalogue: pathlib.Path,
    images: pathlib.Path,
    allow_missing: bool,
) -> str:
    entries = load_catalogue(catalogue)
    errors: list[str] = []
    seen: set[str] = set()
    for entry in entries:
        for key in ("id", "format", "title", "description", "image"):
            value = entry.get(key)
            if not isinstance(value, str) or not value:
                errors.append(f"catalogue entry missing {key}")
        image = entry.get("image", "")
        if image in seen:
            errors.append(f"duplicate image {image}")
        seen.add(image)
        if image and not (images / image).is_file() and not allow_missing:
            errors.append(f"{image}: image missing from {images}")
        if image and not (images / image).is_file() and allow_missing:
            print(f"warning: {image} not generated yet", file=sys.stderr)

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)

    lines = [
        "# Source formats",
        "",
        "Cards below are generated from mock inputs by `cargo run --example sources`.",
        "Each card is parsed by the corresponding `.oops` source or built-in adapter.",
        "",
        "| Format | OOPS | Parser | Description |",
        "|---|---|---|---|",
    ]
    for entry in entries:
        source = entry.get("source")
        if isinstance(source, str) and source:
            parser = f"[`{source}`]({blob_url(repo, branch, source)})"
        else:
            parser = "built-in"
        version = entry.get("version_label", "built-in")
        lines.append(
            f"| `{entry['format']}` | {version} | {parser} | {entry['description']} |"
        )

    for entry in entries:
        lines.extend(
            [
                "",
                f"## {entry['title']}",
                "",
                f"`{entry['format']}` · **{entry.get('version_label', 'built-in')}**",
                "",
                f"![{entry['title']}]({image_url(repo, branch, entry['image'])})",
                "",
                entry["description"],
            ]
        )
        source = entry.get("source")
        if isinstance(source, str) and source:
            lines.append("")
            lines.append(f"Source preset: [`{source}`]({blob_url(repo, branch, source)})")

    lines.append("")
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True)
    parser.add_argument("--branch", required=True)
    parser.add_argument("--catalogue", default="assets/sources/catalogue.json")
    parser.add_argument("--images", default="assets/sources")
    parser.add_argument("--out", required=True)
    parser.add_argument("--allow-missing", action="store_true")
    args = parser.parse_args()
    page = build_page(
        args.repo,
        args.branch,
        pathlib.Path(args.catalogue),
        pathlib.Path(args.images),
        args.allow_missing,
    )
    pathlib.Path(args.out).write_text(page, encoding="utf-8")
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
