#!/bin/sh

set -eu

tmp_output=$(mktemp)
trap 'rm -f "$tmp_output"' EXIT INT HUP TERM

set +e
cargo test --workspace --all-targets >"$tmp_output" 2>&1
cargo_status=$?
set -e

awk '
  /^warning:/ ||
  /^error:/ ||
  /^error\[E[0-9]+\]:/ ||
  /^help:/ ||
  /^note:/ ||
  /(^|[^[:alpha:]])FAILED([^[:alpha:]]|$)/ ||
  /failures:/ ||
  /panicked at/ {
    print
  }
' "$tmp_output"

passed_total=0
failed_total=0
ignored_total=0

while IFS= read -r line; do
  case "$line" in
    *"test result:"*)
      counts=$(printf '%s\n' "$line" | awk '
        /test result:/ {
          passed=""; failed=""; ignored="";
          for (i = 1; i <= NF; i++) {
            if ($i == "passed;") passed = $(i - 1);
            if ($i == "failed;") failed = $(i - 1);
            if ($i == "ignored;") ignored = $(i - 1);
          }
          if (passed != "" && failed != "" && ignored != "") {
            printf "%s %s %s\n", passed, failed, ignored;
          }
        }
      ')
      if [ -n "$counts" ]; then
        set -- $counts
        passed_total=$((passed_total + $1))
        failed_total=$((failed_total + $2))
        ignored_total=$((ignored_total + $3))
      fi
      ;;
  esac
done <"$tmp_output"

printf '\nSummed test totals: passed=%s failed=%s ignored=%s\n' \
  "$passed_total" "$failed_total" "$ignored_total"

if [ "$cargo_status" -ne 0 ] || [ "$failed_total" -ne 0 ]; then
  exit 1
fi
