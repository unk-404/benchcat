#!/usr/bin/env python3
"""Time the current and previous benchcat releases on the same corpus.

The release workflow calls this with the freshly published binary and the last
released binary. It prints a markdown table, appends it to the job summary, and
writes canonical JSONL files that benchcat itself renders as a comparison card.
"""

from __future__ import annotations

import argparse
import json
import os
import statistics
import subprocess
import sys
import time
from pathlib import Path

# Same corpus sizes and run count for every release; RAYON_NUM_THREADS keeps the
# parallel parse/emit stages comparable on runners with a different core count.
CASES = (5_000, 20_000)
CASES_LABEL = {5_000: "render 5k rows", 20_000: "render 20k rows"}
RUNS = 7
THREADS = "4"


def write_corpus(path: Path, rows: int) -> None:
    with path.open("w", encoding="utf-8", newline="\n") as fh:
        for i in range(rows):
            record = {
                "id": f"op.{i // 250:03d}.{i:05d}",
                "group": f"group {i // 250:03d}",
                "label": f"operation {i}",
                "unit": "ops/s",
                "value": 1_000.0 + (i % 977) * 3.5,
                "direction": "higher",
                "params": {"workers": 1 + i % 8},
            }
            if i % 25 == 0:
                record["note"] = f"batch {i}"
            fh.write(json.dumps(record, separators=(",", ":")) + "\n")


def run_once(binary: str, corpus: Path, out: Path, env: dict[str, str]) -> None:
    subprocess.run(
        [binary, "render", str(corpus), "-o", str(out)],
        check=True,
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def measure(
    previous: str,
    current: str,
    corpora: dict[int, Path],
    out_dir: Path,
    env: dict[str, str],
    runs: int,
) -> dict[int, dict[str, list[float]]]:
    timings: dict[int, dict[str, list[float]]] = {
        rows: {"previous": [], "current": []} for rows in corpora
    }
    out = out_dir / "render.svg"
    for rows, corpus in corpora.items():
        run_once(previous, corpus, out, env)
        run_once(current, corpus, out, env)
    for _ in range(runs):
        for rows, corpus in corpora.items():
            for name, binary in (("previous", previous), ("current", current)):
                start = time.perf_counter()
                run_once(binary, corpus, out, env)
                timings[rows][name].append((time.perf_counter() - start) * 1_000.0)
    return timings


def series_record(rows: int, samples: list[float]) -> dict[str, object]:
    return {
        "id": CASES_LABEL[rows],
        "group": "render",
        "label": CASES_LABEL[rows],
        "unit": "ms",
        "value": round(statistics.median(samples), 3),
        "direction": "lower",
        "stats": {"samples": len(samples), "min": round(min(samples), 3)},
    }


def write_series(path: Path, timings: dict[int, dict[str, list[float]]], side: str) -> None:
    with path.open("w", encoding="utf-8", newline="\n") as fh:
        for rows in CASES:
            fh.write(json.dumps(series_record(rows, timings[rows][side]), separators=(",", ":")))
            fh.write("\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--previous", required=True, help="previous release binary")
    parser.add_argument("--current", required=True, help="current release binary")
    parser.add_argument("--previous-tag", required=True)
    parser.add_argument("--current-tag", required=True)
    parser.add_argument("--out-dir", required=True, type=Path)
    parser.add_argument("--runs", type=int, default=RUNS)
    args = parser.parse_args()

    out_dir = args.out_dir
    out_dir.mkdir(parents=True, exist_ok=True)
    corpora = {}
    for rows in CASES:
        corpus = out_dir / f"corpus-{rows}.jsonl"
        write_corpus(corpus, rows)
        corpora[rows] = corpus

    env = dict(os.environ)
    env["RAYON_NUM_THREADS"] = THREADS
    try:
        timings = measure(
            args.previous, args.current, corpora, out_dir, env, args.runs
        )
    except FileNotFoundError as err:
        print(f"error: {err}", file=sys.stderr)
        return 1
    except subprocess.CalledProcessError as err:
        print(f"error: benchmark command failed with {err.returncode}", file=sys.stderr)
        return 1

    write_series(out_dir / "previous.jsonl", timings, "previous")
    write_series(out_dir / "current.jsonl", timings, "current")

    lines = [
        "### benchcat self-benchmark",
        "",
        f"Median of {args.runs} interleaved runs, `RAYON_NUM_THREADS={THREADS}`, "
        "same runner; lower is better.",
        "",
        f"| case | {args.previous_tag} | {args.current_tag} | delta |",
        "|---|---:|---:|---:|",
    ]
    regressions = []
    for rows in CASES:
        previous = statistics.median(timings[rows]["previous"])
        current = statistics.median(timings[rows]["current"])
        delta = (current - previous) / previous * 100.0
        label = CASES_LABEL[rows]
        lines.append(f"| {label} | {previous:.1f} ms | {current:.1f} ms | {delta:+.1f}% |")
        if delta > 10.0:
            regressions.append(f"{label}: {delta:+.1f}%")
    table = "\n".join(lines) + "\n"
    print(table)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as fh:
            fh.write(table)

    for regression in regressions:
        print(f"::warning::slower than {args.previous_tag} — {regression}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
