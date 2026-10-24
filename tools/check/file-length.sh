#!/usr/bin/env bash
# Fails when a source file is longer than the limit in CONTRIBUTING.md ("Modules"), so big
# modules get split by subject instead of growing. Stylesheets are not checked.
set -euo pipefail

limit=1000

status=0
while IFS= read -r file; do
  lines=$(wc -l <"$file")
  if ((lines > limit)); then
    echo "$file has $lines lines, over the limit of $limit: split it by subject." >&2
    status=1
  fi
done < <(git ls-files '*.rs' '*.js' '*.py' '*.sh')
exit $status
