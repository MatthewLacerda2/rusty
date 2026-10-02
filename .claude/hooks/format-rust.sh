#!/bin/bash
# PostToolUse hook (#488): rustfmt the Rust file an Edit/Write just touched, so the
# format gate is never what fails. rustfmt reads rustfmt.toml (edition, width) from
# the repo. Silent either way: a file that does not parse is rust-analyzer's to
# report, not this hook's.
file="$(python3 -c 'import json,sys; print(json.load(sys.stdin).get("tool_input", {}).get("file_path", ""))' 2>/dev/null)"
case "$file" in
  *.rs) [ -f "$file" ] && rustfmt --quiet "$file" >/dev/null 2>&1 ;;
esac
exit 0
