#!/bin/sh

set -eu

tmp_output=$(mktemp)
trap 'rm -f "$tmp_output"' EXIT INT HUP TERM

set +e
cargo build --workspace >"$tmp_output" 2>&1
cargo_status=$?
set -e

cat "$tmp_output" | head -80
echo "---EXIT---"
echo "exit code: $cargo_status"
set +e
tail -30 "$tmp_output" | grep -E 'error'
set -e