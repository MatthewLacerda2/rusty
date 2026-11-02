"""A conflict the queue may resolve by itself, and the record of every conflict it meets (#1723).

[`lists`] mirrors the `sorted` lint rule's notion of a list and its keys,
[`resolve`] decides a conflicted file and writes its resolution, and [`record`]
appends one line per conflict met to a file beside the hand-back memory.
"""
