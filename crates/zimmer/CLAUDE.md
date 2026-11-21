# CLAUDE.md — zimmer, the synthesiser

This file loads when you work under `crates/zimmer/`: `rusty-zimmer`, the synthesiser
behind `Sound.*`. The root `CLAUDE.md` still applies; this is what only matters here.

## The name
**`zimmer` is named for Hans Zimmer, surname only**, and says nothing about what the
crate does; the first line of `crates/zimmer/src/lib.rs` does that (*sound from a document*). It is
the engine's own crate, not a vendor.

## What it is and is not
- **No I/O.** A recipe (a `Patch` for one note, a `Song` for music) renders to samples
  and WAV bytes; the caller writes the files and resolves a track's patch reference
  (`PatchResolver`). `crates/script/src/api/sound/` is that caller.
- **A sim crate**: a bake is a pure function of the document and `SYNTH_VERSION`. No
  clock, no unseeded randomness (noise draws from `hash`, seeded by the recipe), and
  `rusty_core::collections` for maps, never std's.
- **Its public API is `Sound.*`'s contract.** The patch and song types are the stored
  document formats a game's recipes are written in, and their serde field names are
  the vocabulary `docs/api/Sound.md` points at: a rename or removal there breaks every
  recipe on disk, so the PR that does it says so and updates `docs/api/Sound.md`.

## `SYNTH_VERSION`
- **A change to what a recipe renders to bumps `SYNTH_VERSION`** (`crates/zimmer/src/lib.rs`) in the
  same change: the number is what makes a change to the sound deliberate, and a game's
  baked `.wav` files no longer match their recipes once the sound moves. Touch a source,
  filter, envelope or effect and the samples move: bump it. Touch only prose, a name or
  a document type that defaults to the old behaviour: leave it.
- **`crates/zimmer/tests/suite/fingerprint` holds it**: it bakes the probe corpus (`probes.rs`) and
  compares length, level and band shares, with tolerance, to a table recorded at the
  current version. Measures that move under an unchanged version fail with "bump
  `SYNTH_VERSION`"; a bumped version asks for the table to be re-recorded, which the
  failure prints. CI's x86_64 Linux is the authority.
- **Prove a refactor sounds the same by bytes**: bake the probe corpus through
  `Sound.Bake` and `Sound.BakeSong` on `main` and on the branch and compare the `.wav`
  files' sha256 in the PR. A change the diff already answers (a new optional field
  defaulting to the old behaviour) says so in one line instead.

## Documents
- **A new optional field is skipped at its default** (`skip_serializing_if`): recipes are
  re-serialised whenever a tool writes one (`Sound.ToJson`), so a field that writes its
  default into every document rewrites every recipe file without changing a sample.
  `crates/zimmer/tests/suite/defaults.rs` fails on a round trip that adds a key; the fields older than
  the rule are named in `crates/zimmer/tests/suite/common/written.rs`, and a new field never joins it.

## Tests
- **`crates/zimmer/tests/suite/` is the crate's own integration target**: it reaches the crate only
  through its public API, and `cargo nextest run -p rusty-zimmer` runs it without
  building the rest of the engine.
