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
`tests/verdict/test_setup_steps.py` holds that every other name still appears in
a workflow, so renaming a step (or bumping `actions/checkout`) fails `make
scripts` rather than silently turning the rule off.
"""

from __future__ import annotations

# Steps that only fetch or install what a job builds with: a failure in one is
# the network's or the runner's, never the code's. Sorted (a test holds it), so
# two branches adding a name meet on different lines.
SETUP: tuple[str, ...] = (
    "Build EmbarkStudios/cargo-deny-action@v2",
    "Cache Blender",
    "Cache cargo registry + target",
    "Ensure rustfmt + clippy",
    "Install Blender",
    "Install cargo-nextest",
    "Run actions/checkout@v7",
    "Set up job",
    "Software Vulkan driver (lavapipe)",
    "System libs for winit/wgpu builds",
    "Toolchain (from rust-toolchain.toml)",
)


def failed_in_setup(job: dict) -> bool:
    """Whether `job` failed only in [`SETUP`] steps, none but setup steps before."""
    steps = sorted(job.get("steps") or [], key=lambda s: s.get("number", 0))
    failed = [s for s in steps if s.get("conclusion") == "failure"]
    if not failed or any(s.get("name") not in SETUP for s in failed):
        return False
    first = failed[0].get("number", 0)
    return all(s.get("name") in SETUP for s in steps if s.get("number", 0) < first)
