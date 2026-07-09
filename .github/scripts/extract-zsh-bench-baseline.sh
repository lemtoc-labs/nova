#!/usr/bin/env bash
# Extract one stored zsh-bench entry from github-action-benchmark data.js.
# Usage: extract-zsh-bench-baseline.sh <data.js> <target-sha> <output.json> [metadata.env]
set -euo pipefail

input="${1:?Usage: extract-zsh-bench-baseline.sh <data.js> <target-sha> <output.json> [metadata.env]}"
target_sha="${2:-}"
output="${3:?Usage: extract-zsh-bench-baseline.sh <data.js> <target-sha> <output.json> [metadata.env]}"
metadata_output="${4:-}"
benchmark_name="${NOVA_BENCHMARK_NAME:-Zsh Interactive Latency}"

if [[ ! -r "$input" ]]; then
  echo "Benchmark data file not found: $input" >&2
  exit 1
fi

data_json="$(mktemp)"
metadata_json="$(mktemp)"
trap 'rm -f "$data_json" "$metadata_json"' EXIT

sed '1s/^window\.BENCHMARK_DATA = //' "$input" | sed '$s/;[[:space:]]*$//' > "$data_json"

jq \
  --arg benchmark_name "$benchmark_name" \
  --arg target_sha "$target_sha" \
  '
    (.entries[$benchmark_name] // []) as $entries
    | ($entries | map(select(.commit.id == $target_sha)) | last) as $exact
    | ($exact // ($entries | last)) as $entry
    | if $entry == null then
        []
      else
        $entry.benches
      end
  ' "$data_json" > "$output"

jq \
  --arg benchmark_name "$benchmark_name" \
  --arg target_sha "$target_sha" \
  '
    (.entries[$benchmark_name] // []) as $entries
    | ($entries | map(select(.commit.id == $target_sha)) | last) as $exact
    | ($exact // ($entries | last)) as $entry
    | if $entry == null then
        {
          found: false,
          matched: false,
          target_sha: $target_sha,
          commit_sha: "",
          commit_url: ""
        }
      else
        {
          found: true,
          matched: ($exact != null),
          target_sha: $target_sha,
          commit_sha: $entry.commit.id,
          commit_url: ($entry.commit.url // "")
        }
      end
  ' "$data_json" > "$metadata_json"

if [[ -n "$metadata_output" ]]; then
  jq -r '
    def bool: if . then "true" else "false" end;
    [
      "NOVA_BENCH_PREVIOUS_FOUND=" + (.found | bool),
      "NOVA_BENCH_PREVIOUS_MATCHED=" + (.matched | bool),
      "NOVA_BENCH_PREVIOUS_TARGET_SHA=" + .target_sha,
      "NOVA_BENCH_PREVIOUS_SHA=" + .commit_sha,
      "NOVA_BENCH_PREVIOUS_URL=" + .commit_url
    ][]
  ' "$metadata_json" > "$metadata_output"
fi

echo "Extracted zsh-bench baseline metadata:"
cat "$metadata_json"
