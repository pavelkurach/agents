#!/usr/bin/env bash
# Rules index and validation for revy-rules-ingest.
#   rules.sh index            every rule (all languages) and specialist, from the skill's and ~/.revy-review/ folders
#   rules.sh lint <file>...   format checks the CLI leaves to the author, on draft or written rule files
#   rules.sh check [file...]  `revy-review rules` over both folders (parse, duplicate ids, specialists), then lint
# REVY_SKILL overrides the revy-review skill directory (default: ../skill beside this skill).
set -euo pipefail

here=$(cd -P "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
skill=${REVY_SKILL:-$(cd -P "$here/../skill" 2>/dev/null && pwd || true)}
own="$HOME/.revy-review"
max_when=200

[[ -n "$skill" && -d "$skill/rules" ]] || { echo "revy-review skill not found (set REVY_SKILL to the directory holding rules/)" >&2; exit 2; }

index() {
  local dir
  for dir in "$skill" "$own"; do
    [[ -d "$dir/rules" ]] || continue
    echo "Rules: $dir/rules/<topic>/<id>.md"
    for f in "$dir"/rules/*/*.md; do
      [[ -e "$f" ]] || continue
      awk -v id="$(basename "$f" .md)" -v topic="$(basename "$(dirname "$f")")" '
        /^langs:/ { sub(/^langs:[ \t]*/, ""); langs = $0 }
        /^When:/  { sub(/^When:[ \t]*/, "");  when = $0 }
        END { printf "%s (%s) [%s] — %s\n", id, topic, langs, when }' "$f"
    done
  done
  for dir in "$skill" "$own"; do
    [[ -d "$dir/reviewers" ]] || continue
    echo "Specialists: $dir/reviewers/<name>.md"
    for f in "$dir"/reviewers/*.md; do
      [[ -e "$f" && $(basename "$f") != README.md ]] || continue
      printf '%s — %s\n' "$(basename "$f" .md)" "$(grep -m1 '^rules:' "$f" || echo 'rules: none')"
    done
  done
}

lint() {
  local f id errors=0 when report
  fail() { echo "$f: $1" >&2; errors=1; }
  for f in "$@"; do
    [[ -f "$f" ]] || { echo "$f: no such file" >&2; errors=1; continue; }
    id=$(basename "$f" .md)
    [[ $(head -n1 "$f") == "# $id" ]] || fail "first line must be '# $id' (the file name)"
    [[ $id =~ ^[a-z0-9]+(-[a-z0-9]+)*$ ]] || fail "id '$id' must be kebab-case"
    grep -q '^langs: ' "$f" || fail "no 'langs:' line"
    when=$(grep -m1 '^When: ' "$f" | sed 's/^When: //' || true)
    [[ -n "$when" ]] || fail "no 'When:' line"
    (( ${#when} <= max_when )) || fail "When is ${#when} chars; keep it to $max_when"
    [[ $when != "The diff"* ]] || fail "When opens on 'The diff'; open on the construct the reviewer sees"
    grep -q '^Ask: ' "$f" || fail "no 'Ask:' line"
    report=$(grep -m1 '^Report as: ' "$f" || true)
    [[ -n "$report" ]] || fail "no 'Report as:' line"
    [[ $report == *"; fix: "* ]] || fail "'Report as:' needs '; fix: <the deepest fix>'"
  done
  return "$errors"
}

check() {
  local repo head tmp status=0
  repo=$(git -C "$skill" rev-parse --show-toplevel)
  head=$(git -C "$repo" rev-parse HEAD)
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' RETURN
  # An empty change (base = head): the CLI still parses every rule and specialist, and indexes only `any` rules.
  printf '{"repo":"%s","target":"rules-check","base":"%s","head":"%s","skill":"%s"}\n' \
    "$repo" "$head" "$head" "$skill" > "$tmp/state.json"
  revy-review rules "$tmp" > /dev/null || status=1
  (( $# == 0 )) || lint "$@" || status=1
  (( status == 0 )) && echo "clean"
  return "$status"
}

case "${1:-}" in
  index) index ;;
  lint)  shift; (( $# > 0 )) || { echo "usage: rules.sh lint <file>..." >&2; exit 2; }; lint "$@" ;;
  check) shift; check "$@" ;;
  *) sed -n '2,6p' "$0" >&2; exit 2 ;;
esac
