"""Live rulesets as GitHub answers them, and a fake `gh` that serves one."""

from __future__ import annotations

import json
import subprocess

# `main` as it stood before #491, with the keys GitHub adds.
BEFORE = {
    "id": 17663158, "name": "main", "target": "branch", "source_type": "Repository",
    "enforcement": "active", "conditions": {"ref_name": {"exclude": [], "include": ["~DEFAULT_BRANCH"]}},
    "rules": [{"type": "deletion"}, {"type": "non_fast_forward"}],
    "bypass_actors": [], "current_user_can_bypass": "never",
}

# The committed file once applied: the checks listed in another order, and a
# parameter GitHub fills in that the file does not state.
CHECKS = {
    "type": "required_status_checks",
    "parameters": {
        "strict_required_status_checks_policy": False, "do_not_enforce_on_create": False,
        "required_status_checks": [{"context": "lint-gate", "integration_id": 15368},
                                   {"context": "ci-gate", "integration_id": 15368}],
        "some_future_default": True,
    },
}
AFTER = {**BEFORE, "rules": [*BEFORE["rules"], CHECKS]}


class FakeGh:
    """Answers the rulesets list and one ruleset; records every write."""

    def __init__(self, live: dict | None, after_write: dict | None = None):
        self.live, self.after_write, self.writes = live, after_write, []

    def __call__(self, argv, input=None, **_):
        args = argv[2:]
        if args[0] == "--method":
            self.writes.append((args[1], args[2], json.loads(input)))
            self.live = self.after_write
            return self.done({})
        if "rulesets?" in args[0]:
            listed = [{"id": self.live["id"], "name": self.live["name"]}] if self.live else []
            return self.done(listed)
        return self.done(self.live)

    @staticmethod
    def done(answer) -> subprocess.CompletedProcess:
        return subprocess.CompletedProcess([], 0, json.dumps(answer), "")
