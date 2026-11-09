"""A job that failed while installing a tool, before anything was built (#1932).

On 2026-11-08 the queue lost two rounds this way within an hour, neither
PR's code involved: train `b9cdaba` (#1920 + #1916) was split because
`build-test-cross (macos-26)` failed in `Install cargo-nextest` (a 504 from the
release host, run 42466064410), and #1920 alone was handed back because `deny`
failed building the cargo-deny action's image (`tar: Error is not recoverable`,
run 42468896745 attempt 1). Such a job concludes `failure`, not `cancelled`, so
#869's rule ([`runners.runner_red`]) read it as the code's.

[`failed_in_setup`] reads the job's `steps[]` (the jobs API lists each with its
`number` and `conclusion`): every failed step is a [`SETUP`] step, and every step
before the first failure is one too. Steps after a failure are skipped or run
under `always()` (`Test summary` reports nothing over no results), so they say
nothing about the code. A job whose steps are not listed is not judged a setup
failure: there is no evidence either way.

[`SETUP`] is matched by the name the jobs API reports. A step written with
`name:` reports that name; one written only with `uses:` reports `Run <uses>`,
and a Docker action's image build `Build <uses>`. `Set up job` is GitHub's own.
`tests/verdict/test_setup_order.py` reads every gated job's steps in order and
holds two things: every [`SETUP`] name is still a step's (renaming one, or
bumping `actions/checkout`, fails `make scripts` rather than silently turning
the rule off), and every step before a job's last setup step is a setup step
too. The review of PR #1933 found `Driver version (Mesa / lavapipe)` between
the lavapipe install and `Install cargo-nextest` in `build-test`, so a 504 in
that install was still the code's there; and `lint` ran `Doc references` before
its setup steps. Both are fixed, and the next version print inserted before an
install fails the test.

## A pull request that edits its own setup is judged on it

The toolchain, the install steps and `Set up job` itself run what the pull
request's own `rust-toolchain.toml`, `.github/workflows/` and the composite
actions under `.github/actions/` they call (#1973) say. A bad channel
or action version there fails a setup step every time, so read as the runners'
it would be re-run once and handed back as `runners unavailable`, never as the
code's. [`edited`] is that case: such a failure is the code's, and for a train
it is so when any member touched those files (PR #1933's review).
"""

from __future__ import annotations

# Steps that only fetch or install what a job builds with: a failure in one is
# the network's or the runner's, never the code's. Sorted (a test holds it), so
# two branches adding a name meet on different lines.
SETUP: tuple[str, ...] = (
    "Build EmbarkStudios/cargo-deny-action@v2",
    "Cache Blender",
    "Cache Open Image Denoise",
    "Cache cargo registry + target",
    "Driver version (Mesa / lavapipe)",
    "Ensure rustfmt + clippy",
    "Install Blender",
    "Install Open Image Denoise",
    "Install cargo-nextest",
    "Run actions/checkout@v7",
    "Set up job",
    "Software Vulkan driver (lavapipe)",
    "System libs for winit/wgpu builds",
    "Toolchain (from rust-toolchain.toml)",
)


# What a job's setup steps run is written in these: a pull request changing one
# is judged on its own setup ([`edited`]).
# `.github/actions/` holds the composite install step the workflows call (#1973).
OWN_SETUP: tuple[str, ...] = (".github/actions/", ".github/workflows/", "rust-toolchain.toml")


def edited(paths: list[str]) -> bool:
    """Whether `paths` (a pull request's, or every member's of a train) change
    what the setup steps run, so that a setup failure is the code's."""
    return any(p == own or (own.endswith("/") and p.startswith(own)) for p in paths for own in OWN_SETUP)


def step(job: dict) -> str | None:
    """The name of the step `job` failed in, or `None` when none is listed failed."""
    steps = sorted(job.get("steps") or [], key=lambda s: s.get("number", 0))
    return next((s.get("name") for s in steps if s.get("conclusion") == "failure"), None)


def failed_in_setup(job: dict) -> bool:
    """Whether `job` failed only in [`SETUP`] steps, none but setup steps before."""
    steps = sorted(job.get("steps") or [], key=lambda s: s.get("number", 0))
    failed = [s for s in steps if s.get("conclusion") == "failure"]
    if not failed or any(s.get("name") not in SETUP for s in failed):
        return False
    first = failed[0].get("number", 0)
    return all(s.get("name") in SETUP for s in steps if s.get("number", 0) < first)
