#!/usr/bin/env bash
# Render benchmark-result.json as a sticky PR comment body.
set -euo pipefail

input="${1:?Usage: render-zsh-bench-comment.sh <input.json> <output.md>}"
output="${2:?Usage: render-zsh-bench-comment.sh <input.json> <output.md>}"

if [[ ! -r "$input" ]]; then
  echo "Benchmark result file not found: $input" >&2
  exit 1
fi

{
  echo "<!-- nova-zsh-bench-pr-comment -->"
  echo "## Nova zsh-bench"
  echo
  echo "| metric | median | status | green | yellow | orange | red |"
  echo "| --- | ---: | --- | ---: | ---: | ---: | ---: |"
  jq -r '
    def thresholds:
      {
        "first prompt lag": { "green": 25, "yellow": 50, "orange": 100 },
        "first command lag": { "green": 75, "yellow": 150, "orange": 300 },
        "command lag": { "green": 5, "yellow": 10, "orange": 20 },
        "input lag": { "green": 10, "yellow": 20, "orange": 40 }
      };

    def status($value; $threshold):
      if $value <= $threshold.green then "green"
      elif $value <= $threshold.yellow then "yellow"
      elif $value <= $threshold.orange then "orange"
      else "red"
      end;

    .[]
    | select((thresholds[.name] // null) != null)
    | . as $metric
    | thresholds[$metric.name] as $threshold
    | [
        $metric.name,
        (($metric.value | tostring) + " " + ($metric.unit // "ms")),
        status(($metric.value | tonumber); $threshold),
        (($threshold.green | tostring) + " ms"),
        (($threshold.yellow | tostring) + " ms"),
        (($threshold.orange | tostring) + " ms"),
        ("> " + ($threshold.orange | tostring) + " ms")
      ]
    | "| " + join(" | ") + " |"
  ' "$input"
  echo
  echo "Red status fails CI. Values are medians from zsh-bench raw iterations."
  if [[ -n "${GITHUB_SERVER_URL:-}" && -n "${GITHUB_REPOSITORY:-}" && -n "${GITHUB_RUN_ID:-}" ]]; then
    echo
    echo "[Workflow run](${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID})"
  fi
} > "$output"

cat "$output"
