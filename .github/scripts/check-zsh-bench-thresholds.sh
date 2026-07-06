#!/usr/bin/env bash
# Fail CI only when a zsh-bench metric reaches the dashboard red state.
set -euo pipefail

input="${1:-benchmark-result.json}"

if [[ ! -r "$input" ]]; then
  echo "::error::Benchmark result file not found: $input"
  exit 1
fi

failures="$(
  jq -r '
    def targets:
      {
        "first prompt lag": 50,
        "first command lag": 150,
        "command lag": 10,
        "input lag": 20
      };

    .[]
    | select((targets[.name] // null) != null)
    | . as $metric
    | (targets[$metric.name]) as $target
    | ($target * 2) as $red_threshold
    | select(($metric.value | tonumber) > $red_threshold)
    | [
        $metric.name,
        (($metric.value | tostring) + " " + ($metric.unit // "ms")),
        (($target | tostring) + " ms"),
        ("> " + ($red_threshold | tostring) + " ms")
      ]
    | @tsv
  ' "$input"
)"

if [[ -z "$failures" ]]; then
  echo "All thresholded zsh-bench metrics are below dashboard red status."
  exit 0
fi

echo "Red zsh-bench metric(s) detected. Dashboard red status fails CI."
echo "metric | value | target | red threshold"
echo "--- | --- | --- | ---"

while IFS=$'\t' read -r metric value target red_threshold; do
  echo "${metric} | ${value} | ${target} | ${red_threshold}"
  echo "::error title=Zsh bench red metric::${metric} is ${value}; red threshold is ${red_threshold}."
done <<< "$failures"

exit 1
