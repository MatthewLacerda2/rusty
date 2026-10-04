#!/usr/bin/env bash
# Files a red run on `main` as an issue on the board (#481). Called by
# .github/workflows/main-health.yml once a watched workflow run completes.
#
# Env: GH_TOKEN, REPO (owner/name), RUN_ID (the completed run),
#      DRY_RUN ('true' prints every write instead of making it).
#
# One open issue per workflow, found by a hidden marker in its body. A red run
# opens it, or comments on it when the set of failing jobs changed since the
# last note. A green run comments once, and never closes it: green here can be
# vacuous (a docs-only merge skips the build steps; the nightly does not
# rebuild), so the fix PR closes the issue, not this script.
set -euo pipefail

WATCHED='ci lint mutants-sweep coverage windows'
LABEL=bug

run=$(gh api "repos/$REPO/actions/runs/$RUN_ID")
field() { jq -r ".$1" <<<"$run"; }
wf=$(field name); event=$(field event); branch=$(field head_branch)
sha=$(field head_sha); url=$(field html_url); run_concl=$(field conclusion)

if [[ " $WATCHED " != *" $wf "* ]] || [[ $branch != main ]] \
  || [[ $event != push && $event != schedule ]]; then
  echo "run $RUN_ID is '$wf' on '$branch' via '$event' — not a watched main run; nothing to do"
  exit 0
fi

# A weekly sweep shard that concluded non-success is a *finding* when
# cargo-mutants finished (exit 2 = survivors, 3 = timeouts) — that is the
# signal's own report, not an outage. Anything else (no artifact, a baseline
# that did not build, the shard's time budget, a lost runner) left its mutants
# unmeasured: file it.
shard_had_findings() {
  local dir rc
  dir=$(mktemp -d)
  gh run download "$RUN_ID" -R "$REPO" -n "mutants-shard-$1" -D "$dir" >/dev/null 2>&1 || return 1
  rc=$(cat "$dir/exit-code" 2>/dev/null || echo none)
  [[ $rc == 2 || $rc == 3 ]]
}

# Every other non-success job files. The weekly signal jobs never fail on a
# finding — `coverage` flags a floor drop without exiting non-zero, and
# `mutants-report` only summarises — so their failure is always
# infrastructural. The nightly `windows` signal is the exception on purpose: a
# red Windows build is the finding, and this issue is where it lands (#741).
failed=()
while IFS=$'\t' read -r name concl; do
  case "$concl" in success | skipped | neutral) continue ;; esac
  if [[ $name =~ ^mutants\ \(([0-9]+)\)$ ]] && shard_had_findings "${BASH_REMATCH[1]}"; then
    echo "skip: $name ($concl) — mutation findings, surfaced by mutants-report"
    continue
  fi
  failed+=("$name ($concl)")
done < <(gh api --paginate "repos/$REPO/actions/runs/$RUN_ID/jobs?filter=latest&per_page=100" \
  --jq '.jobs[] | [.name, (.conclusion // "unfinished")] | @tsv' | sort)

# A run with no jobs at all never started (e.g. an invalid workflow file).
if [[ ${#failed[@]} -eq 0 && $run_concl != success && $run_concl != skipped ]] \
  && [[ -z $(gh api "repos/$REPO/actions/runs/$RUN_ID/jobs" --jq '.jobs[].name') ]]; then
  failed+=("the run itself ($run_concl, no jobs started)")
fi

state=green
[[ ${#failed[@]} -gt 0 ]] && state=red
jobs=''
[[ $state == red ]] && jobs=$(printf '%s;' "${failed[@]}")
sig="state=$state|jobs=$jobs"
echo "$wf on $sha via $event: $sig"

marker="<!-- main-health:workflow=$wf -->"
issue=$(gh api --paginate "repos/$REPO/issues?state=open&labels=$LABEL&per_page=100" \
  --jq ".[] | select(.pull_request == null) | select((.body // \"\") | contains(\"$marker\")) | .number" |
  head -n1)

# The last state note on the issue (body first, then comments, in order).
last_sig() {
  {
    gh api "repos/$REPO/issues/$issue" --jq '.body'
    gh api --paginate "repos/$REPO/issues/$issue/comments?per_page=100" --jq '.[].body'
  } | grep -o 'main-health:state=[a-z]*|jobs=[^|]*' | tail -n1 | sed 's/^main-health://' || true
}

write() {
  if [[ ${DRY_RUN:-false} == true ]]; then
    echo "DRY-RUN: gh $*"
    [[ -n ${body:-} ]] && sed 's/^/DRY-RUN>   /' "$body"
    return 0
  fi
  gh "$@"
}

body=$(mktemp)
echo "<!-- main-health:$sig|end -->" >"$body"

details() {
  echo "- Run: $url"
  echo "- Commit: \`$sha\` (\`$event\`)"
  if [[ $state == red ]]; then
    echo '- Failed jobs:'
    # shellcheck disable=SC2016 # literal backticks: Markdown code spans
    printf '  - `%s`\n' "${failed[@]}"
  fi
}

if [[ $state == red && -z $issue ]]; then
  {
    echo "$marker"
    echo "The \`$wf\` workflow went red on \`main\`. Filed automatically by \`main-health.yml\` (#481)."
    echo ''
    details
    echo ''
    echo 'Informational signals (coverage, the weekly mutation sweep) file here only when they'
    echo 'fail to *run* — a crash, a timeout, a cancelled shard. Their findings (surviving mutants,'
    echo 'a coverage drop) stay in their own reports. The nightly Windows signal files when it'
    echo 'goes red too: Windows is not a shipped platform, so this never blocked a merge, but it is'
    echo 'how Windows rot is caught before it piles up (#741). Later red runs comment here when the set of'
    echo 'failing jobs changes. A green run leaves a note but does not close this: green can be'
    echo 'vacuous (a docs-only merge skips the build steps; the nightly does not rebuild), so close'
    echo 'it from the PR that fixes it.'
  } >>"$body"
  write issue create -R "$REPO" --label "$LABEL" \
    --title "Red main: the \`$wf\` workflow is failing" --body-file "$body"
  exit 0
fi

[[ -z $issue ]] && { echo 'green and no open issue; nothing to do'; exit 0; }

prev=$(last_sig)
if [[ $state == green && $prev == state=green* ]] || [[ $sig == "$prev" ]]; then
  echo "issue #$issue already says '$prev'; nothing new to add"
  exit 0
fi

{
  if [[ $state == red && $prev == state=green* ]]; then
    echo "Red again:"
  elif [[ $state == red ]]; then
    echo "Still red, and the failing jobs changed:"
  else
    echo "This run was green. Check that it re-ran what failed before closing — a docs-only merge"
    echo "skips the build steps and the nightly does not rebuild."
  fi
  echo ''
  details
} >>"$body"
write issue comment "$issue" -R "$REPO" --body-file "$body"
