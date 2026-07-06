#!/usr/bin/env bash
# Install a pinned zsh-bench checkout for CI benchmarks.
set -euo pipefail

target="${1:?Usage: install-zsh-bench.sh <target-dir>}"
ref="${ZSH_BENCH_REF:-28b1b1bc888159f0a2cf50f9d29381758341aba1}"

if [[ -e "$target" ]]; then
  echo "target already exists: $target" >&2
  exit 1
fi

mkdir -p "$(dirname "$target")"
git clone https://github.com/romkatv/zsh-bench.git "$target"
git -C "$target" checkout --detach "$ref"

echo "Installed zsh-bench at $ref"
