# Building a game on rusty

You are in a **game project**: a folder the rusty engine opens, holding one game.
This guide is the engine's half of your instructions (the game's half is the
project's own `CLAUDE.md`). The `rusty` MCP serves it as `rusty://game-guide.md`,
read from the engine checkout, so it always matches the engine you drive.
Paths below that start with `docs/` are in that checkout.

## The model: Unity's, as a subset

- **Unity is the reference.** rusty copies Unity's runtime model and implements the
  subset a game needs. When something is unclear, do what Unity does.
- A **GameObject** is a `Transform` plus any mix of the engine's components (`Mesh`,
  `Collider`, `Rigidbody`, `Camera`, `Light`, `Animator`, `NavMeshAgent`, …) and a
  place in the parent/child hierarchy. An empty one is a pivot or a marker.
- **Custom behaviour is a Lua script component** (a MonoBehaviour with `Start`,
  `Update`, …), never a new engine component. When the game needs something the
  engine can't do, that is an engine finding (below), not something to patch.
- **Anything the editor does, the API does.** Every editor action has a Lua call, so
  `eval` can build and configure the whole game.

## Scripts are deterministic

The game is a pure function of (seed, inputs, fixed timestep), so a play-test can be
replayed exactly:
- the gameplay Lua has no `os` or `io`;
- `math.random` is seeded (the `Random` namespace);
- `require`, `dofile` and `loadfile` read inside the project; a file outside it warns.

## The project

```text
assets/          the game: scenes/, scripts/, prefabs/, models/, textures/,
                 audio/, materials/, shaders/
project.rusty    engine-owned: the engine commit and the build settings
scenarios/       headless play-tests (optional)
cache/  saved/   regenerable output and save data, ignored by git
CLAUDE.md        the game's rules: yours
.mcp.json        engine-owned: registers this MCP; rewritten when a dev editor opens
                 the project, with this machine's absolute paths
```

Every path the game stores is **relative to the project root**, `/`-separated
(`assets/scripts/enemy.lua`).

- **One model format inside: glTF.** `.obj` loads for static meshes; `.fbx` and
  `.blend` are converted to a cached glTF at import (`.blend` needs Blender 5 or
  later installed).
- **Test content is primitive**: greybox levels and fixtures from cubes, planes and
  capsules, not authored art.

## Play-testing headlessly

A scenario is a Lua file under `scenarios/` that drives the game through the same
API plus `Harness.*` (press keys, step ticks, assert), run with the `play` binary:

```sh
cargo run --manifest-path <engine>/Cargo.toml --bin play --features dev -- \
    --project . scenarios/smoke.lua out/
```

It writes `out/results.json` and `out/console.log`. See `docs/testing.md` for
scenarios and captures, and `docs/ui.md` for the in-game UI.

## The MCP

The tools describe themselves; what they don't say:
- read **`rusty://scripting-api.md`** before writing Lua: it is the whole API
  (`docs/api/`, one page per namespace);
- `eval` runs one line of that API; `snapshot` returns the world as JSON;
- when the engine warned during a call, the result carries a second text block
  starting `engine warnings:`. Read it: a skipped asset or a dropped collider shows
  up there first;
- `.mcp.json` registers two servers. **Use one at a time:** `rusty` attaches to the
  editor window the operator watches (open the editor on this project first);
  `rusty-headless` runs its own world with no window, for when none is open. Two
  worlds saving the same scene overwrite each other.

More: `docs/mcp.md`.

## Engine findings: file them, don't fix them

A game session never edits the engine. An engine bug or a missing capability becomes
an issue on `MatthewLacerda2/rusty`, and the game carries on (work around it, or
leave that part for later):
- label it `agent`;
- say what the game tried, what happened and what should have happened, with the
  smallest `eval` lines or scenario that shows it;
- give a bug one `Kind:` line: `Kind: case — searched <terms>` after looking for
  earlier issues of its kind, or `Kind: #N` when it is an instance of open issue N.
