"""No pull request starves in line: past an age or a count of passes, it goes first (#928)."""

from __future__ import annotations


# How long a pull request may wait in line before it goes first, whatever its
# conflicts. The conflict-first order (#753) passes a one-conflict pull request
# over for every clean one; on scorsese, 2026-10-15, five waited 4–5 hours
# (MatthewLacerda2/scorsese#1345).
AGE_MINUTES = 60

# How many other pull requests may be taken ahead of one before it goes first.
AGE_PASSES = 3


class Line:
    """Who is in line, since when, and how many takes went ahead of each.

    A pull request that leaves the line (merged, handed back, back to draft,
    unlabelled) is forgotten, so coming back starts its wait afresh.
    """

    def __init__(self) -> None:
        self.since: dict[int, float] = {}
        self.passes: dict[int, int] = {}

    def see(self, numbers: list[int], now: float) -> None:
        """The pull requests in line on this listing."""
        for number in numbers:
            self.since.setdefault(number, now)
        for gone in set(self.since) - set(numbers):
            self.since.pop(gone)
            self.passes.pop(gone, None)

    def took(self, number: int) -> None:
        """`number` was taken: everyone else in line was passed over once."""
        for other in self.since:
            if other != number:
                self.passes[other] = self.passes.get(other, 0) + 1

    def aged(self, now: float) -> set[int]:
        """Those that have waited [`AGE_MINUTES`] or been passed [`AGE_PASSES`] times."""
        return {
            n for n, since in self.since.items()
            if now - since >= AGE_MINUTES * 60 or self.passes.get(n, 0) >= AGE_PASSES
        }

    def said(self, number: int, now: float) -> str:
        """The line saying aging put `number` first."""
        waited, passes = (now - self.since.get(number, now)) / 60, self.passes.get(number, 0)
        return (
            f"watch: #{number} has waited {waited:.0f} minutes in line and been passed over {passes} time(s);"
            " it goes first, whatever it conflicts with (#928)."
        )
