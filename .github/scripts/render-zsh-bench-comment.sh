#!/usr/bin/env bash
# Render benchmark-result.json as a sticky PR comment body or workflow summary.
set -euo pipefail

input="${1:?Usage: render-zsh-bench-comment.sh <input.json> <output.md> [previous.json]}"
output="${2:?Usage: render-zsh-bench-comment.sh <input.json> <output.md> [previous.json]}"
previous_input="${3:-}"
commit_sha="${NOVA_BENCH_COMMIT_SHA:-${GITHUB_SHA:-}}"
commit_url="${NOVA_BENCH_COMMIT_URL:-}"
workflow_url=""
show_previous=false
previous_metric_count=0

if [[ ! -r "$input" ]]; then
  echo "Benchmark result file not found: $input" >&2
  exit 1
fi

if [[ -n "$previous_input" ]]; then
  if [[ ! -r "$previous_input" ]]; then
    echo "Previous benchmark result file not found: $previous_input" >&2
    exit 1
  fi
  show_previous=true
  previous_metric_count="$(jq 'length' "$previous_input")"
else
  previous_input="$(mktemp)"
  trap 'rm -f "$previous_input"' EXIT
  printf '[]\n' > "$previous_input"
fi

if [[ -z "$commit_url" && -n "$commit_sha" && -n "${GITHUB_SERVER_URL:-}" && -n "${GITHUB_REPOSITORY:-}" ]]; then
  commit_url="${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/commit/${commit_sha}"
fi

if [[ -n "${GITHUB_SERVER_URL:-}" && -n "${GITHUB_REPOSITORY:-}" && -n "${GITHUB_RUN_ID:-}" ]]; then
  workflow_url="${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}"
fi

short_sha() {
  printf '%s' "$1" | cut -c1-7
}

{
  echo "<!-- nova-zsh-bench-pr-comment -->"
  echo "## Nova zsh-bench"
  echo
  if [[ "$show_previous" == true ]]; then
    echo "| metric | median | previous | delta | status | green | yellow | orange | red |"
    echo "| --- | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: |"
  else
    echo "| metric | median | status | green | yellow | orange | red |"
    echo "| --- | ---: | --- | ---: | ---: | ---: | ---: |"
  fi
  jq -r '
    def round2:
      (. * 100 | round) / 100;

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

    def metric_value:
      if . == null or (.value // null) == null then
        "N/A"
      else
        ((.value | tostring) + " " + (.unit // "ms"))
      end;

    def metric_delta($metric; $previous_metric):
      if $previous_metric == null or ($previous_metric.value // null) == null then
        "N/A"
      else
        (($metric.value | tonumber) - ($previous_metric.value | tonumber) | round2) as $delta
        | ($metric.unit // $previous_metric.unit // "ms") as $unit
        | if $delta == 0 then
            "0 " + $unit
          elif $delta > 0 then
            "+" + ($delta | tostring) + " " + $unit
          else
            ($delta | tostring) + " " + $unit
          end
      end;

    ($previous[0] // []) as $previous_metrics
    | .[]
    | select((thresholds[.name] // null) != null)
    | . as $metric
    | thresholds[$metric.name] as $threshold
    | ($previous_metrics | map(select(.name == $metric.name)) | .[0]) as $previous_metric
    | if $show_previous == "true" then
        [
          $metric.name,
          ($metric | metric_value),
          ($previous_metric | metric_value),
          metric_delta($metric; $previous_metric),
          status(($metric.value | tonumber); $threshold),
          (($threshold.green | tostring) + " ms"),
          (($threshold.yellow | tostring) + " ms"),
          (($threshold.orange | tostring) + " ms"),
          ("> " + ($threshold.orange | tostring) + " ms")
        ]
      else
        [
          $metric.name,
          ($metric | metric_value),
          status(($metric.value | tonumber); $threshold),
          (($threshold.green | tostring) + " ms"),
          (($threshold.yellow | tostring) + " ms"),
          (($threshold.orange | tostring) + " ms"),
          ("> " + ($threshold.orange | tostring) + " ms")
        ]
      end
    | "| " + join(" | ") + " |"
  ' --arg show_previous "$show_previous" --slurpfile previous "$previous_input" "$input"
  echo
  if [[ "$show_previous" == true ]]; then
    previous_found="${NOVA_BENCH_PREVIOUS_FOUND:-}"
    previous_matched="${NOVA_BENCH_PREVIOUS_MATCHED:-}"
    previous_sha="${NOVA_BENCH_PREVIOUS_SHA:-}"
    previous_url="${NOVA_BENCH_PREVIOUS_URL:-}"
    previous_target_sha="${NOVA_BENCH_PREVIOUS_TARGET_SHA:-}"

    if [[ "$previous_found" == "true" && -n "$previous_sha" ]]; then
      previous_short="$(short_sha "$previous_sha")"
      previous_target_short="$(short_sha "$previous_target_sha")"
      if [[ "$previous_matched" == "true" ]]; then
        if [[ -n "$previous_url" ]]; then
          echo "Previous is the stored benchmark for [${previous_short}](${previous_url})."
        else
          echo "Previous is the stored benchmark for ${previous_short}."
        fi
      elif [[ -n "$previous_target_short" ]]; then
        if [[ -n "$previous_url" ]]; then
          echo "Previous is the latest stored main benchmark, [${previous_short}](${previous_url}); requested baseline ${previous_target_short} was not in stored history."
        else
          echo "Previous is the latest stored main benchmark, ${previous_short}; requested baseline ${previous_target_short} was not in stored history."
        fi
      elif [[ -n "$previous_url" ]]; then
        echo "Previous is the latest stored main benchmark, [${previous_short}](${previous_url})."
      else
        echo "Previous is the latest stored main benchmark, ${previous_short}."
      fi
    elif [[ "$previous_metric_count" != "0" ]]; then
      echo "Previous is loaded from the provided benchmark file."
    else
      echo "Previous is N/A because no stored zsh-bench history was available."
    fi
    echo "Delta is current median minus previous median. Lower is faster."
    echo
  fi
  echo "Red status fails CI. Values are medians from zsh-bench raw iterations."
  if [[ -n "$commit_sha" && -n "$commit_url" ]]; then
    echo
    echo "$commit_url"
  fi
  if [[ -n "$workflow_url" ]]; then
    echo
    echo "[Workflow run](${workflow_url})"
  fi
} > "$output"

cat "$output"
