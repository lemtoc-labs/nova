#!/usr/bin/env bash
# Publish the custom zsh-bench dashboard template into the gh-pages benchmark directory.
set -euo pipefail

source_file="${1:?Usage: publish-zsh-bench-dashboard.sh <index.html> [data-dir] [branch]}"
data_dir="${2:-bench/zsh}"
branch="${3:-gh-pages}"

if [[ ! -r "$source_file" ]]; then
  echo "Dashboard template not found: $source_file" >&2
  exit 1
fi

worktree="$(mktemp -d "${TMPDIR:-/tmp}/nova-zsh-dashboard.XXXXXX")"

cleanup() {
  git worktree remove --force "$worktree" >/dev/null 2>&1 || rm -rf "$worktree"
}
trap cleanup EXIT HUP INT TERM

git fetch origin "$branch"
git worktree add --detach "$worktree" "origin/$branch"

mkdir -p "$worktree/$data_dir"
cp "$source_file" "$worktree/$data_dir/index.html"

git -C "$worktree" add "$data_dir/index.html"
if git -C "$worktree" diff --cached --quiet; then
  echo "Benchmark dashboard template is already up to date."
  exit 0
fi

git \
  -C "$worktree" \
  -c user.name=github-action-benchmark \
  -c user.email=github@users.noreply.github.com \
  -c commit.gpgsign=false \
  commit -m "update zsh benchmark dashboard"

git -C "$worktree" push origin "HEAD:$branch"
