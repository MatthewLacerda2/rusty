"""`proved_tree.verdict` skips only on a tree a run really built (#1784).

A fake `gh api` answers from canned bodies keyed by path prefix, so nothing
reaches GitHub. Every way the lookup can be wrong must run the gates in full.
"""

from __future__ import annotations

import unittest

import proved_tree

REPO, SHA, BEFORE, TREE = "o/r", "s" * 40, "b" * 40, "t" * 40


def job(name: str, step: str | None, conclusion: str = "success", step_concl: str = "success") -> dict:
    steps = [{"name": step, "conclusion": step_concl}] if step else []
    return {"name": name, "conclusion": conclusion, "steps": steps}


def built_jobs(**over) -> list[dict]:
    jobs = {base: job(f"{base} (macos-26)" if base == "build-test-cross" else base, step)
            for base, step in proved_tree.BUILT["ci.yml"].items()}
    jobs.update(over)
    return list(jobs.values())


class Fake:
    """`gh api`: the answer for the first prefix a path starts with, else `None`."""

    def __init__(self, jobs=None, tree=TREE, files=("src/lib.rs",), status="ahead", fail=(), tip=SHA):
        run = {"id": 7, "html_url": "http://run/7", "head_commit": {"tree_id": tree}}
        self.answers = {
            f"repos/{REPO}/compare/": {"status": status, "files": [{"filename": f} for f in files]},
            f"repos/{REPO}/commits/main": {"sha": tip},
            f"repos/{REPO}/commits/{SHA}": {"commit": {"tree": {"sha": TREE}}},
            f"repos/{REPO}/actions/workflows/": {"workflow_runs": [run]},
            f"repos/{REPO}/actions/runs/7/jobs": {"jobs": built_jobs() if jobs is None else jobs},
        }
        self.fail = fail

    def __call__(self, path: str):
        for prefix, answer in self.answers.items():
            if path.startswith(prefix):
                return None if any(prefix.startswith(f) for f in self.fail) else answer
        return None


def verdict(fake: Fake, before: str = BEFORE):
    return proved_tree.verdict(fake, REPO, "ci.yml", SHA, before)


class Skips(unittest.TestCase):
    def test_an_identical_tree_with_a_run_that_built(self):
        url, why = verdict(Fake())
        self.assertEqual(url, "http://run/7")
        self.assertIn("skips the build", why)

    def test_lint_by_its_own_built_step(self):
        fake = Fake(jobs=[job("lint", "Clippy (hard gate, default features)")])
        self.assertEqual(proved_tree.verdict(fake, REPO, "lint.yml", SHA, BEFORE)[0], "http://run/7")


class Superseded(unittest.TestCase):
    """A train's intermediate squash is no longer `main`'s tip by its push run (#1785)."""

    def test_a_push_main_already_left_skips_without_a_run(self):
        url, why = verdict(Fake(tree="x" * 40, tip="n" * 40))
        self.assertEqual(url, "n" * 40)
        self.assertIn("skips the build", why)

    def test_a_push_still_the_tip_needs_its_own_proof(self):
        self.assertIsNone(verdict(Fake(tree="x" * 40))[0])

    def test_a_tip_lookup_failing_is_not_superseded(self):
        for fake in (Fake(tree="x" * 40, fail=(f"repos/{REPO}/commits/main",)), Fake(tree="x" * 40, tip="short")):
            with self.subTest():
                url, why = verdict(fake)
                self.assertIsNone(url)
                self.assertTrue(why.startswith("runs in full"), why)

    def test_a_superseded_push_that_moves_the_cache_key_still_runs(self):
        self.assertIsNone(verdict(Fake(files=("Cargo.lock",), tip="n" * 40))[0])


class RunsInFull(unittest.TestCase):
    def assert_runs(self, fake: Fake, **kw):
        url, why = verdict(fake, **kw)
        self.assertIsNone(url)
        self.assertTrue(why.startswith("runs in full"), why)

    def test_a_different_tree(self):
        self.assert_runs(Fake(tree="x" * 40))

    def test_a_markdown_only_run_whose_steps_skipped(self):
        skipped = {b: job(b, None, "skipped") for b in ("build-test-cross", "blender", "deny")}
        skipped["build-test"] = job("build-test", "Test (engine, default)", step_concl="skipped")
        self.assert_runs(Fake(jobs=built_jobs(**skipped)))

    def test_a_draft_run_whose_gating_jobs_skipped(self):
        self.assert_runs(Fake(jobs=[job("changes", None), job("ci-gate", None)]))

    def test_a_failed_job(self):
        self.assert_runs(Fake(jobs=built_jobs(blender=job("blender", "Test (the .blend import, through Blender)",
                                                          "failure", "failure"))))

    def test_a_changed_lockfile_or_toolchain_or_workflow(self):
        for path in ("Cargo.lock", "tools/lint/Cargo.toml", "rust-toolchain.toml", ".cargo/config.toml",
                     ".github/workflows/ci.yml", ".github/actions/install-tool/action.yml"):
            with self.subTest(path=path):
                self.assert_runs(Fake(files=("src/lib.rs", path)))

    def test_a_push_that_is_not_a_fast_forward_or_has_no_before(self):
        self.assert_runs(Fake(status="diverged"))
        self.assert_runs(Fake(), before=proved_tree.NO_COMMIT)
        self.assert_runs(Fake(files=[f"f{i}" for i in range(proved_tree.COMPARE_CAP)]))

    def test_any_api_error(self):
        for prefix in (f"repos/{REPO}/compare/", f"repos/{REPO}/commits/",
                       f"repos/{REPO}/actions/workflows/", f"repos/{REPO}/actions/runs/"):
            with self.subTest(prefix=prefix):
                self.assert_runs(Fake(fail=(prefix,)))

    def test_a_malformed_answer(self):
        fake = Fake()
        fake.answers[f"repos/{REPO}/commits/{SHA}"] = {"commit": {}}
        self.assert_runs(fake)
        fake = Fake()
        fake.answers[f"repos/{REPO}/actions/workflows/"] = {"workflow_runs": [{"head_commit": None}]}
        self.assert_runs(fake)


if __name__ == "__main__":
    unittest.main()
