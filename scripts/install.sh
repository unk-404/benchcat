#!/usr/bin/env bash
# Install the bench-svg binary for this runner.
#
# Resolution order:
#   1. INPUT_TOOL_PATH                     — use a binary that is already present
#   2. INPUT_VERSION (explicit tag)        — e.g. v2026.10.19-cd5cd87
#   3. GITHUB_ACTION_REF, when it is a tag — keeps action and binary in lockstep
#   4. latest release                      — no API call, no rate limiting
set -euo pipefail

log() { printf '[bench-svg] %s\n' "$*"; }
fail() {
  printf '[bench-svg] error: %s\n' "$*" >&2
  exit 1
}

if [[ -n "${INPUT_TOOL_PATH:-}" ]]; then
  [[ -x "$INPUT_TOOL_PATH" ]] || fail "tool-path is not executable: $INPUT_TOOL_PATH"
  dirname "$INPUT_TOOL_PATH" >>"${GITHUB_PATH:-/dev/null}"
  "$INPUT_TOOL_PATH" --version >/dev/null || fail "tool-path is not bench-svg: $INPUT_TOOL_PATH"
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

cache_root="${RUNNER_TOOL_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}}/bench-svg"
cache_dir="$cache_root/${version//\//_}-$target"
bin="$cache_dir/bench-svg"
[[ "$os" == windows ]] && bin="$bin.exe"

if [[ -x "$bin" ]]; then
  log "cache hit: $bin"
  printf '%s\n' "$cache_dir" >>"${GITHUB_PATH:-/dev/null}"
  exit 0
fi

# In a consumer workflow GITHUB_REPOSITORY is the caller's repository, not this
# action's; GITHUB_ACTION_REPOSITORY is the action's own `owner/name`.
repo="${GITHUB_ACTION_REPOSITORY:-${GITHUB_REPOSITORY:-}}"
base="${BENCH_SVG_RELEASE_BASE:-}"
if [[ -z "$base" ]]; then
  [[ -n "$repo" ]] || fail "GITHUB_ACTION_REPOSITORY is not set; pass tool-path or set BENCH_SVG_RELEASE_BASE"
  base="https://github.com/$repo/releases"
fi
if [[ "$version" == "latest" ]]; then
  url="$base/latest/download/bench-svg-$target.tar.gz"
else
  url="$base/download/$version/bench-svg-$target.tar.gz"
fi

mkdir -p "$cache_dir"
archive="$cache_dir/bench-svg.tar.gz"
log "downloading $url"
curl -fsSL --retry 3 --retry-delay 2 -o "$archive" "$url" ||
  fail "download failed; pass tool-path, or set BENCH_SVG_RELEASE_BASE to a mirror"

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
  expected=$(awk -v name="bench-svg-$target.tar.gz" '$2 == name {print $1; exit}' "$archive.sha256")
fi
if [[ -n "$expected" ]]; then
  if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$archive" | awk '{print $1}')
  else
    actual=$(shasum -a 256 "$archive" | awk '{print $1}')
  fi
  [[ "$expected" == "$actual" ]] || fail "checksum mismatch for $url"
  log "checksum verified"
else
  log "warning: no checksum published for this release"
fi

# Extract from inside the directory: an absolute `D:/...` archive name is parsed
# as a remote host by GNU tar on Windows runners.
(
  cd "$cache_dir"
  tar -xzf bench-svg.tar.gz
)
rm -f "$archive" "$archive.sha256"
chmod +x "$bin"
[[ -x "$bin" ]] || fail "archive did not contain bench-svg"
"$bin" --version >/dev/null || fail "downloaded binary does not run on this runner"

log "installed $("$bin" --version) from $version"
printf '%s\n' "$cache_dir" >>"${GITHUB_PATH:-/dev/null}"
