#!/usr/bin/env bash
# Install the benchcat binary for this runner.
#
# Resolution order:
#   1. INPUT_TOOL_PATH                     — use a binary that is already present
#   2. INPUT_VERSION (explicit tag)        — e.g. v2026.10.19-cd5cd87
#   3. GITHUB_ACTION_REF, when it is a tag — keeps action and binary in lockstep
#   4. latest release                      — no API call, no rate limiting
set -euo pipefail

log() { printf '[benchcat] %s\n' "$*"; }
fail() {
  printf '[benchcat] error: %s\n' "$*" >&2
  exit 1
}

if [[ -n "${INPUT_TOOL_PATH:-}" ]]; then
  [[ -x "$INPUT_TOOL_PATH" ]] || fail "tool-path is not executable: $INPUT_TOOL_PATH"
  dirname "$INPUT_TOOL_PATH" >>"${GITHUB_PATH:-/dev/null}"
  "$INPUT_TOOL_PATH" --version >/dev/null || fail "tool-path is not benchcat: $INPUT_TOOL_PATH"
  log "using existing binary at $INPUT_TOOL_PATH"
  exit 0
fi

version="${INPUT_VERSION:-auto}"
ref="${GITHUB_ACTION_REF:-}"

if [[ "$version" == "auto" ]]; then
  if [[ "$ref" =~ ^v[0-9]{4}\.[0-9]{2}\.[0-9]{2}-[0-9a-f]{7}$ ]]; then
    version="$ref"
  else
    version="latest"
  fi
fi

case "$(uname -s)" in
  Linux*) os=linux ;;
  Darwin*) os=darwin ;;
  MINGW* | MSYS* | CYGWIN*) os=windows ;;
  *) fail "unsupported operating system: $(uname -s)" ;;
esac
case "$(uname -m)" in
  x86_64 | amd64) arch=x86_64 ;;
  arm64 | aarch64) arch=aarch64 ;;
  *) fail "unsupported architecture: $(uname -m)" ;;
esac

if [[ "$os" == linux && "$arch" == x86_64 ]]; then
  target=x86_64-unknown-linux-musl
elif [[ "$os" == linux && "$arch" == aarch64 ]]; then
  target=aarch64-unknown-linux-musl
elif [[ "$os" == darwin && "$arch" == x86_64 ]]; then
  target=x86_64-apple-darwin
elif [[ "$os" == darwin && "$arch" == aarch64 ]]; then
  target=aarch64-apple-darwin
elif [[ "$os" == windows && "$arch" == x86_64 ]]; then
  target=x86_64-pc-windows-msvc
else
  fail "no prebuilt binary for $os/$arch; pass tool-path instead"
fi

cache_root="${RUNNER_TOOL_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}}/benchcat"
cache_dir="$cache_root/${version//\//_}-$target"
bin="$cache_dir/benchcat"
[[ "$os" == windows ]] && bin="$bin.exe"

if [[ -x "$bin" ]]; then
  log "cache hit: $bin"
  printf '%s\n' "$cache_dir" >>"${GITHUB_PATH:-/dev/null}"
  exit 0
fi

# In a consumer workflow GITHUB_REPOSITORY is the caller's repository, not this
# action's. GitHub leaves GITHUB_ACTION_REPOSITORY empty inside composite-action
# steps (community discussion #49245), so derive `owner/repo` from the action
# checkout path instead: <runner>/_actions/<owner>/<repo>/<ref>. A local
# `uses: ./` has no _actions segment and falls back to GITHUB_REPOSITORY.
action_repo="${GITHUB_ACTION_REPOSITORY:-}"
if [[ -z "$action_repo" && -n "${GITHUB_ACTION_PATH:-}" ]]; then
  action_path="${GITHUB_ACTION_PATH//\\//}"
  if [[ "$action_path" == */_actions/* ]]; then
    rest="${action_path#*/_actions/}"
    owner="${rest%%/*}"
    rest="${rest#*/}"
    action_name="${rest%%/*}"
    if [[ -n "$owner" && -n "$action_name" ]]; then
      action_repo="$owner/$action_name"
    fi
  fi
fi
repo="${action_repo:-${GITHUB_REPOSITORY:-}}"
base="${BENCHCAT_RELEASE_BASE:-}"
if [[ -z "$base" ]]; then
  [[ -n "$repo" ]] || fail "cannot tell which repository hosts this action; pass tool-path or set BENCHCAT_RELEASE_BASE"
  base="https://github.com/$repo/releases"
fi
if [[ "$version" == "latest" ]]; then
  url="$base/latest/download/benchcat-$target.tar.gz"
else
  url="$base/download/$version/benchcat-$target.tar.gz"
fi

mkdir -p "$cache_dir"
archive="$cache_dir/benchcat.tar.gz"
log "downloading $url"
if ! curl -fsSL --retry 3 --retry-delay 2 -o "$archive" "$url"; then
  # Releases published before the repository was renamed still use the
  # bench-svg-* asset names; fall back so `version: latest` keeps working.
  legacy_url="${url/benchcat-/bench-svg-}"
  if [[ "$legacy_url" == "$url" ]]; then
    fail "download failed; pass tool-path, or set BENCHCAT_RELEASE_BASE to a mirror"
  fi
  log "retrying with the pre-rename asset name"
  curl -fsSL --retry 3 --retry-delay 2 -o "$archive" "$legacy_url" ||
    fail "download failed; pass tool-path, or set BENCHCAT_RELEASE_BASE to a mirror"
  url="$legacy_url"
fi

# Releases carry a per-archive .sha256 and a combined SHA256SUMS; older ones
# only have SHA256SUMS, so try the single file first and fall back to the list.
if [[ "$version" == "latest" ]]; then
  sums_url="$base/latest/download/SHA256SUMS"
else
  sums_url="$base/download/$version/SHA256SUMS"
fi
expected=""
if curl -fsSL --retry 2 -o "$archive.sha256" "$url.sha256" 2>/dev/null; then
  expected=$(awk '{print $1; exit}' "$archive.sha256")
elif curl -fsSL --retry 2 -o "$archive.sha256" "$sums_url" 2>/dev/null; then
  asset="$(basename "$url")"
  expected=$(awk -v name="$asset" '$2 == name {print $1; exit}' "$archive.sha256")
fi
if [[ -n "$expected" ]]; then
  # Hash the bare file name from inside its directory. MSYS sha256sum escapes a
  # Windows path (leading backslash, doubled separators), so hashing
  # "$archive" directly yields "\\<hash>" and never matches on Windows runners.
  if command -v sha256sum >/dev/null 2>&1; then
    actual=$(cd "$cache_dir" && sha256sum benchcat.tar.gz | awk '{print $1}')
  else
    actual=$(cd "$cache_dir" && shasum -a 256 benchcat.tar.gz | awk '{print $1}')
  fi
  actual="${actual#\\}"
  [[ "$expected" == "$actual" ]] || fail "checksum mismatch for $url"
  log "checksum verified"
else
  log "warning: no checksum published for this release"
fi

# Extract from inside the directory: an absolute `D:/...` archive name is parsed
# as a remote host by GNU tar on Windows runners.
(
  cd "$cache_dir"
  tar -xzf benchcat.tar.gz
)
# Pre-rename archives contain a binary called bench-svg; expose the new name.
legacy_bin="$cache_dir/bench-svg"
[[ "$os" == windows ]] && legacy_bin="$legacy_bin.exe"
if [[ ! -f "$bin" && -f "$legacy_bin" ]]; then
  mv "$legacy_bin" "$bin"
fi
rm -f "$archive" "$archive.sha256"
chmod +x "$bin"
[[ -x "$bin" ]] || fail "archive did not contain benchcat"
"$bin" --version >/dev/null || fail "downloaded binary does not run on this runner"

log "installed $("$bin" --version) from $version"
printf '%s\n' "$cache_dir" >>"${GITHUB_PATH:-/dev/null}"
