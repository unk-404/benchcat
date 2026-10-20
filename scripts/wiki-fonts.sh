#!/usr/bin/env bash
# Refresh the GitHub wiki "Fonts" page from the committed specimens.
#
# Images stay in the main repository; the wiki page links their raw URLs, so
# there is one copy of every SVG. The wiki must exist (open the Wiki tab and
# create the first page once); until then this script exits successfully with a
# notice so the release workflow is not blocked.
set -euo pipefail

repo="${GITHUB_REPOSITORY:?}"
branch="${WIKI_SOURCE_BRANCH:-${GITHUB_REF_NAME:-master}}"
token="${WIKI_TOKEN:-${GH_TOKEN:-}}"
work="${RUNNER_TEMP:-/tmp}/benchcat-wiki"
page="$work/Fonts.md"

if [[ "${WIKI_DRY_RUN:-}" == "1" ]]; then
  out="${WIKI_OUT:-${RUNNER_TEMP:-/tmp}/Fonts.md}"
  args=()
  if [[ "${WIKI_ALLOW_MISSING:-}" == "1" ]]; then
    args+=(--allow-missing-specimens)
  fi
  python3 scripts/wiki-fonts.py --repo "$repo" --branch "$branch" --out "$out" "${args[@]}"
  cat "$out"
  exit 0
fi

[[ -n "$token" ]] || { echo "::notice::no wiki token; skipping"; exit 0; }

rm -rf "$work"
export GIT_TERMINAL_PROMPT=0
if ! git clone "https://x-access-token:${token}@github.com/${repo}.wiki.git" "$work" >/dev/null 2>&1; then
  echo "::notice::${repo}.wiki.git is not available; create the wiki once and rerun"
  exit 0
fi

python3 scripts/wiki-fonts.py --repo "$repo" --branch "$branch" --out "$page"

cd "$work"
if [[ -z "$(git status --porcelain -- Fonts.md)" ]]; then
  echo "wiki Fonts page already up to date"
  exit 0
fi

git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git add Fonts.md
git commit -m "Update font specimens"
if ! git push origin HEAD; then
  # Another wiki edit landed first; rebase and retry once. The page is
  # deterministic, so regenerating it here is equivalent to resolving by hand.
  git pull --rebase >/dev/null 2>&1 || git rebase --abort 2>/dev/null || true
  if ! git push origin HEAD; then
    echo "::warning::could not push the wiki Fonts page; check the wiki token"
    exit 0
  fi
fi
echo "::notice::updated the wiki Fonts page"
