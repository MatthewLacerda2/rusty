# crates/zimmer

Rules for working inside `zimmer`. The root `CLAUDE.md` still applies; this
file holds what only matters here.

## The name

**`zimmer` is ours, and its name is the joke told twice.** The project is named
for Martin Scorsese; the crate that writes the music is named for Hans Zimmer,
surname only both times. It is not a vendor, not a third-party dependency and
not an acronym. The name deliberately says nothing about what the crate does —
the first line of its own `lib.rs` doc does that instead (*sound from a
document*), and `missing_docs` is a merge gate, so that line cannot quietly go
missing. **The rename stops at the crate boundary**: `scorsese synth`, the
`synth_*` MCP tools and `"kind": "synth_audio"` all stay, because an
agent-facing surface has to describe itself and the asset kind is the format
contract.

## Its second consumer: rusty

**`zimmer` has a second consumer: [rusty](https://github.com/MatthewLacerda2/rusty)**,
the owner's game engine, which depends on it as a git dependency pinned to a
commit instead of keeping its own copy. Three things follow. Its
**public API is rusty's contract** — renaming, removing or narrowing anything
`pub` still lands, but breaks rusty the next time it moves its pin, so the pull
request that does it says so. **`SYNTH_VERSION` governs rusty's bakes too** —
the same rule, one more reader. And **game-facing needs are in scope** for
`zimmer`, inside its no-I/O boundary: a game loops music forever where a video
never does, and the north star's "must add to this vision" does not exclude
them. None of this makes it a vendor here; everything above still holds.

## `SYNTH_VERSION`

**A change to what a recipe renders to requires a `SYNTH_VERSION` bump**, in
the same commit, for the same reason a format change requires a
`schema_version` bump: breaking loudly is the point. A bake in `generated/`
is addressed by a hash of the recipe's bytes **and** that number (plus each
patch file a song names and what it read from the project; the address is `providers`' `synth/address.rs`), so bumping it is
what makes every affected file miss the cache and be re-rendered — and not
bumping it leaves every project on disk holding audio its own recipe no
longer describes, silently. The number is declared rather than derived
because deriving it means hashing rendered output, and a digest has no
tolerance to spend on a platform's `sin` and `exp` differing; the constant's
own doc in `src/lib.rs` carries the argument and a table of the versions. So: touch a source, a
filter, an envelope or an effect and the samples move — bump it. Touch only
prose, a name or a document type — leave it alone.

**Three checks hold this**. `tests/suite/fingerprint` bakes a small probe
corpus and compares its length, level and band shares, with tolerance, to a
table recorded at the current version: measures that move under an unchanged
version fail with "bump `SYNTH_VERSION`", and a bumped version asks for the
table to be re-recorded, which the failure message prints. It runs on every
platform (sums over a whole file average a platform's last-bit `sin` and
`exp` away) with CI's x86_64 Linux as the authority. The merge queue hands
back a branch whose bump a sibling already took (`numbering.py`), since two
identical bumps merge without a conflict. And `tests/suite/defaults.rs`, with the
worked recipes in `crates/providers/tests/suite/documented`, holds the rule below.

**Verify by rendering only when the change touches rendering maths.** Baking a
probe corpus against two checkouts settles a genuine doubt — an edited note
loop, a shared helper moved, a stage reordered — and is waste when the diff
already answers it: a new optional field defaulting to old behaviour cannot
move an existing recipe's bytes. Say so in one line and move on.
**`SYNTH_VERSION.md`** beside this file records what previous branches checked,
and why — a bump adds its row to the constant's table and its entry there; the
`ci-merge` skill has how to run probes and what a conflict on the number means.

**A new optional field is skipped at its default.** Recipes are re-serialised
whenever a tool writes one, and the address hashes the file's bytes, so a
field that writes its default into every document invalidates every cached
bake without changing a sample. Give it `skip_serializing_if`, the way
`fx.rs`'s `not_ping_pong` and `song`'s `no_swing` do. A round trip that adds a
key fails `tests/suite/defaults.rs` (and the docs' worked recipes, in `providers`);
the fields that predate the rule are named in `tests/suite/common/written.rs`. A new
field never joins that list.
