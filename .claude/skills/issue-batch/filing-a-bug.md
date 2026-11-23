# Filing a bug

The same kind of bug patched one instance at a time, each review finding the
next, is what this prevents. No agent opens an issue on its own (CLAUDE.md, *The
operator promotes; agents do not file*): a finding goes on the list as a comment,
and these steps are for the orchestrator writing the issue once the operator has
promoted it (a coder or a reviewer does steps 1 and 2 in its comment, so the
finding arrives classed):

0. **Checks it belongs on the list**: someone can reach it as a crash, data loss,
   a sandbox escape or a determinism break, or it questions a decision. An edge
   nobody can hit is dropped.
1. **Looks for earlier cases** of its kind, open and closed, by symbol, module
   and wording. `gh search issues` answers 403 in a cloud session; there, page
   `gh api 'repos/MatthewLacerda2/rusty/issues?state=all&per_page=100&page=N'` and
   grep titles and bodies (write the repository out: gh may fail to resolve
   `{owner}/{repo}` through a cloud session's proxy remote).
2. **Classes it: a case or a symptom**, even the first time. A case is a discrete
   slip; a symptom is an underlying problem that will produce more.
3. **For a symptom, files the fix that scales**: a type, a lint rule, a shared or
   property test, one API. The instance becomes its sub-issue
   (`gh api -X POST repos/MatthewLacerda2/rusty/issues/<root>/sub_issues -F
   sub_issue_id=<instance id>`: `-F`, an integer, and the issue's `id`, not its
   number),
   never its own PR. An instance of an **open** root issue is folded into it: a
   comment on the root, or a sub-issue of it, never a sibling issue.
4. **The second case of a kind with no root issue triggers one.**

Every bug issue carries one line, `Kind:`, exactly one of:

- `Kind: #N`: an instance of root issue N, which is also its sub-issue parent;
- `Kind: case — searched <terms>`: a discrete slip, with the search that found no
  kin;
- `Kind: root`: the root issue itself.

`make board` lists an open `bug` issue (not `planning`) without the line, with
more than one, or with a form not listed above; a `case` that names no search;
a `Kind: #N` whose sub-issue parent is not N; and a `case` or `root` that has a
sub-issue parent. It reads the line past list, quote, bold and
backtick markup, and ignores one inside a fenced code block.
