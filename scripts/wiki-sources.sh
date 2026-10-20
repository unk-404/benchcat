#!/usr/bin/env bash
# Refresh the GitHub wiki "Sources" page from assets/sources/catalogue.json.
set -euo pipefail

repo="${GITHUB_REPOSITORY:?}"
branch="${WIKI_SOURCE_BRANCH:-${GITHUB_REF_NAME:-master}}"
token="${WIKI_TOKEN:-${GH_TOKEN:-}}"
work="${RUNNER_TEMP:-/tmp}/benchcat-wiki"
page="$work/Sources.md"

if [[ "${WIKI_DRY_RUN:-}" == "1" ]]; then
  out="${WIKI_OUT:-${RUNNER_TEMP:-/tmp}/Sources.md}"
  args=()
  if [[ "${WIKI_ALLOW_MISSING:-}" == "1" ]]; then
    args+=(--allow-missing)
  fi
  python3 scripts/wiki-sources.py --repo "$repo" --branch "$branch" --out "$out" "${args[@]}"
  cat "$out"
  exit 0
fi

[[ -n "$token" ]] || { echo "::notice::no wiki token; skipping"; exit 0; }

rm -rf "$work"
export GIT_TERMINAL_PROMPT=0
if ! git clone "https://x-access-token:${token}@github.com/${repo}.wiki.git" "$work" >/dev/null 2>&1; then
  echo "::notice::${repo}.wiki.git is not available; check that the wiki exists and the token can read it"
  exit 0
fi

python3 scripts/wiki-sources.py --repo "$repo" --branch "$branch" --out "$page"

cd "$work"
if [[ -z "$(git status --porcelain -- Sources.md)" ]]; then
  echo "wiki Sources page already up to date"
  exit 0
fi

git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git add Sources.md
git commit -m "Update source format examples"
if ! git push origin HEAD; then
  git pull --rebase >/dev/null 2>&1 || git rebase --abort 2>/dev/null || true
  if ! git push origin HEAD; then
    echo "::warning::could not push the wiki Sources page; check the wiki token"
    exit 0
  fi
fi
echo "::notice::updated the wiki Sources page"
