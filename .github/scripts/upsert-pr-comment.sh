#!/usr/bin/env bash
# Create or update the Nova zsh-bench sticky PR comment.
set -euo pipefail

pr_number="${1:?Usage: upsert-pr-comment.sh <pr-number> <body.md>}"
body_file="${2:?Usage: upsert-pr-comment.sh <pr-number> <body.md>}"
marker="<!-- nova-zsh-bench-pr-comment -->"
repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"

if [[ ! -r "$body_file" ]]; then
  echo "Comment body file not found: $body_file" >&2
  exit 1
fi

comments_json="$(mktemp)"
payload_json="$(mktemp)"

cleanup() {
  rm -f "$comments_json" "$payload_json"
}
trap cleanup EXIT HUP INT TERM

gh api --paginate "repos/${repo}/issues/${pr_number}/comments" > "$comments_json"

comment_id="$(
  jq -r --arg marker "$marker" '
    .[]
    | select(.user.type == "Bot" and (.body | contains($marker)))
    | .id
  ' "$comments_json" | tail -n 1
)"

jq -n --rawfile body "$body_file" '{ body: $body }' > "$payload_json"

if [[ -n "$comment_id" ]]; then
  gh api \
    --method PATCH \
    "repos/${repo}/issues/comments/${comment_id}" \
    --input "$payload_json"
else
  gh api \
    --method POST \
    "repos/${repo}/issues/${pr_number}/comments" \
    --input "$payload_json"
fi
