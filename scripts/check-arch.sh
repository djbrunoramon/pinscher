#!/usr/bin/env bash
# Architecture checks that the compiler can't enforce. Run by CI and before every commit.
set -euo pipefail
cd "$(dirname "$0")/.."

status=0
fail() { echo "check-arch: $*" >&2; status=1; }

# core/ is pure: no macOS frameworks, no processes.
if grep -rnE '\b(objc2|block2|dispatch2|global_hotkey|tracing_oslog)\b|std::process' src/core; then
  fail "src/core must not use macOS APIs or spawn processes (see AGENTS.md)"
fi

# Captured images never touch the disk (AM-04).
for file in src/platform/capture.rs src/platform/secondary_bar.rs; do
  if [ -f "$file" ] && grep -nE 'std::fs|File::create|\bwrite_to|writeToFile|writeToURL' "$file"; then
    fail "$file must keep captured images in memory only"
  fi
done

# Every unsafe block is explained.
while IFS=: read -r file line _; do
  prev=$((line - 1))
  if ! sed -n "$((line > 3 ? line - 3 : 1)),${prev}p" "$file" | grep -q 'SAFETY:'; then
    fail "$file:$line: unsafe block without a // SAFETY: comment"
  fi
done < <(grep -rnE '^\s*(let .*= )?unsafe \{|[^_]unsafe \{' src --include='*.rs' | grep -v '// SAFETY' || true)

exit $status
