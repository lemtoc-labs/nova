#!/usr/bin/env bash
# Convert zsh-bench --raw output to a JSON summary.
# Usage: convert-zsh-bench.sh <input.txt> <output.json>
set -euo pipefail

input="${1:?Usage: convert-zsh-bench.sh <input.txt> <output.json>}"
output="${2:?Usage: convert-zsh-bench.sh <input.txt> <output.json>}"

jq --raw-input --null-input '
  def round2:
    (. * 100 | round) / 100;

  def median:
    sort as $sorted
    | length as $length
    | if $length == 0 then
        null
      elif ($length % 2) == 1 then
        $sorted[($length / 2 | floor)]
      else
        (($sorted[($length / 2) - 1] + $sorted[$length / 2]) / 2)
      end;

  def mean:
    add / length;

  def stddev:
    mean as $mean
    | (map((. - $mean) * (. - $mean)) | add / length | sqrt);

  def metric_name:
    gsub("_ms$"; "")
    | gsub("_"; " ");

  [
    inputs
    | capture("^(?<name>[a-z_]+_ms)=\\( (?<values>.*) \\)$")?
    | select(. != null)
    | .name as $name
    | (.values | split(" ") | map(select(length > 0)) | map(tonumber)) as $values
    | ($values | median | round2) as $median
    | ($values | min | round2) as $min
    | ($values | max | round2) as $max
    | ($values | stddev | round2) as $stddev
    | {
        name: ($name | metric_name),
        unit: "ms",
        value: $median,
        range: (($stddev | tostring) + " ms"),
        extra: (
          "median: " + ($median | tostring) + " ms\n" +
          "min: " + ($min | tostring) + " ms\n" +
          "max: " + ($max | tostring) + " ms\n" +
          "stddev: " + ($stddev | tostring) + " ms\n" +
          "runs: " + ($values | length | tostring)
        )
      }
  ]
' "$input" > "$output"

echo "Converted benchmark results:"
cat "$output"
