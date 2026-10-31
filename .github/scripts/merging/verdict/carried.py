"""What a pull request's diff carries that its kind may not (#966).

Two of the written rules are about the changed paths, read from the same `files`
the verdict already has:

- **An issue-less pull request that touches code is a bug fix.** CLAUDE.md:
  "Issue-less PRs are allowed only for documentation updates or bug fixes". So a
  pull request that names no issue (no closing keyword, no `Refs #N`) and changes
  anything but Markdown, or touches a Markdown file code reads (`changes.CODE_DOCS`), is refused
  unless it is labelled `bug` or is Dependabot's. #795 and #787 slipped through.
- **No image lands on `main`.** The cloud brief: "Never leave a PNG on the branch
  that merges"; captures go on a throwaway commit (`docs/testing.md`). An image
  the net diff adds outside [`IMAGES_ALLOWED`] is refused. This lives here, not in
  the commit hook, because a throwaway commit is allowed.

Both are fixed by an edit (a description, a label, a deleting commit), and are
pure over the pull request.
"""

from __future__ import annotations

import re

from merging.verdict import changes, closing


IMAGE = re.compile(r"\.(?:png|jpe?g|hdr|exr|ktx2)$", re.IGNORECASE)

# Where an image belongs in the repository, each with its reason.
IMAGES_ALLOWED = {
    # The application icon, every size the platforms ask for.
    "engine/icon/": "the engine's icon",
    # Default textures every build ships and a component names by default (#1442's
    # lens dirt), made by a checked-in generator whose test holds the file to it.
    "engine/textures/": "the engine's default textures",
    # Test inputs a loader or importer reads (a texture, an HDR sky).
    "tests/fixtures/": "test fixtures",
}

# How Dependabot and the bug label are spelled on a pull request.
BOT_LOGINS = ("app/dependabot", "dependabot[bot]", "dependabot")
BUG = "bug"

# A deliberate reference that leaves an issue open still names one.
REFS = re.compile(r"\brefs\b:?\s+#\d+\b", re.IGNORECASE)


def entries(pull: dict) -> list[dict]:
    """The changed files, each `{path, status?}` (REST has `filename`)."""
    return [{"path": f.get("path") or f.get("filename", ""), "status": f.get("status")} for f in pull.get("files") or []]


def issueless(pull: dict) -> list[str]:
    """Why an issue-less pull request may not carry this diff, or nothing."""
    if "body" not in pull or closing.referenced(pull) or REFS.search(pull.get("body") or ""):
        return []
    paths = [e["path"] for e in entries(pull)]
    code = [p for p in paths if not changes.inert([p])]
    if not code or BUG in closing.labels(pull) or (pull.get("author") or {}).get("login") in BOT_LOGINS:
        return []
    shown = ", ".join(f"`{p}`" for p in code[:3]) + (", …" if len(code) > 3 else "")
    return [
        f"it names no issue but changes code ({shown}).",
        "Issue-less pull requests are only for documentation or bug fixes (CLAUDE.md,"
        " *Flow*): close the issue it does, or label it `bug` if it is one.",
    ]


def images(pull: dict) -> list[str]:
    """Why the images this diff adds may not land, or nothing."""
    found = [
        e["path"]
        for e in entries(pull)
        if IMAGE.search(e["path"]) and e["status"] != "removed" and not e["path"].startswith(tuple(IMAGES_ALLOWED))
    ]
    if not found:
        return []
    return [
        f"it adds {', '.join(f'`{p}`' for p in found)}, and no image lands on `main`.",
        "Captures go in the description, from a throwaway commit (`docs/testing.md`);"
        " delete them from the branch, or add a directory with its reason to"
        " `merging/verdict/carried.py`'s IMAGES_ALLOWED.",
    ]
