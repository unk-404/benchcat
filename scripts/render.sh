#!/usr/bin/env bash
# Render step of the composite action. Reads INPUT_* from the environment so
# values never reach a shell through string interpolation.
set -euo pipefail

log() { printf '[bench-svg] %s\n' "$*"; }
fail() {
  printf '[bench-svg] error: %s\n' "$*" >&2
  exit 2
}

input="${INPUT_INPUT:-}"
[[ -n "$input" ]] || fail "input is empty"

paths=()
while IFS= read -r path; do
  [[ -n "$path" ]] && paths+=("$path")
done < <(printf '%s\n' "$input" | tr ' ' '\n')

[[ ${#paths[@]} -gt 0 ]] || fail "input is empty"

out="${INPUT_OUT:-assets/benchmarks.svg}"
format="${INPUT_FORMAT:-auto}"
args=(render "${paths[@]}" --out "$out" --format "$format")

if [[ -n "${INPUT_CONFIG:-}" ]]; then args+=(--config "$INPUT_CONFIG"); fi
if [[ -n "${INPUT_FONT:-}" ]]; then args+=(--font "$INPUT_FONT"); fi
if [[ -n "${INPUT_FONT_FILE:-}" ]]; then args+=(--font-file "$INPUT_FONT_FILE"); fi

bench-svg "${args[@]}"

bytes=$(wc -c <"$out" | tr -d ' ')
log "wrote $out ($bytes bytes)"

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  {
    printf 'svg=%s\n' "$out"
    printf 'bytes=%s\n' "$bytes"
  } >>"$GITHUB_OUTPUT"
fi

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  {
    echo "### bench-svg"
    echo
    echo "Wrote \`$out\` ($bytes bytes)."
    echo
    echo "The artifact is deterministic: commit it, upload it, or reference it from a README."
  } >>"$GITHUB_STEP_SUMMARY"
fi
