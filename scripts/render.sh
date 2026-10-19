#!/usr/bin/env bash
# Render step of the composite action. Reads INPUT_* from the environment so
# values never reach a shell through string interpolation.
set -euo pipefail

log() { printf '[benchcat] %s\n' "$*"; }

input="${INPUT_INPUT:-}"

paths=()
if [[ -n "$input" ]]; then
  while IFS= read -r path; do
    [[ -n "$path" ]] && paths+=("$path")
  done < <(printf '%s\n' "$input" | tr ' ' '\n')
fi

# No positional paths is fine when benchcat.toml defines [[series]]; when
# neither is present the CLI reports the precise error.
out="${INPUT_OUT:-assets/benchmarks.svg}"
format="${INPUT_FORMAT:-auto}"
args=(render)
# Guard the expansion: bash 3.2 (macOS /bin/bash) treats "${arr[@]}" on an
# empty array as unbound under `set -u`.
if [[ ${#paths[@]} -gt 0 ]]; then
  args+=("${paths[@]}")
fi
args+=(--out "$out" --format "$format")

if [[ -n "${INPUT_CONFIG:-}" ]]; then args+=(--config "$INPUT_CONFIG"); fi
if [[ -n "${INPUT_FONT:-}" ]]; then args+=(--font "$INPUT_FONT"); fi
if [[ -n "${INPUT_FONT_FILE:-}" ]]; then args+=(--font-file "$INPUT_FONT_FILE"); fi

benchcat "${args[@]}"

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
    echo "### benchcat"
    echo
    echo "Wrote \`$out\` ($bytes bytes)."
    echo
    echo "The artifact is deterministic: commit it, upload it, or reference it from a README."
  } >>"$GITHUB_STEP_SUMMARY"
fi
