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
  `eval` can build and configure the whole game. Every field an inspector card shows
  is reachable by name with `Component.Get` / `Component.Set`, through the card's own
  validation (`docs/api/Component.md`).

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
project.rusty    engine-owned: the engine commit, its place in history, and the
                 build settings
scenarios/       headless play-tests (optional)
cache/  saved/   regenerable output and save data, ignored by git
CLAUDE.md        the game's rules: yours
.mcp.json        registers this MCP: the engine owns its `rusty` and `rusty-headless`
                 entries and rewrites them, with this machine's absolute paths,
                 whenever a dev build opens the project to edit (the editor, New
                 Project, an agent session, an export); other servers are yours
```

Every path the game stores is **relative to the project root**, `/`-separated
(`assets/scripts/enemy.lua`).

- **One model format inside: glTF.** `.obj` loads for static meshes; `.fbx` and
  `.blend` are converted to a cached glTF at import (`.blend` needs Blender 5 or
  later installed).
- **Test content is primitive**: greybox levels and fixtures from cubes, planes and
  capsules, not authored art.

## After an engine upgrade

Every merge to the engine's `main` is a new engine, and the engine keeps **no
backward compatibility**: an upgrade may break the game, by design, and the game
is what moves. Opening the project with a newer engine doesn't ask; it records the
new engine in `project.rusty` and logs one engine warning:

```text
[Project] upgraded from engine <old> to <new>; what changed: git -C <engine> log --oneline <old>..<new>
```

Where you find it: `session-mcp --project` returns it in the first call's
`engine warnings:` block; `bench_open` answers with it as `upgrade` (embed and attach
mode alike); the editor shows it in its console, and every binary prints it on
stderr.

Run that `git log`, read what changed, and bring the game up to date (scenes,
scripts, prefabs) before carrying on. Only an **older** engine, or a branch build,
warns and asks before it opens.

## Play-testing headlessly

A scenario is a Lua file under `scenarios/` that drives the game through the same
API plus `Harness.*` (press keys, step ticks, assert), run with the `play` binary from the engine checkout:

```sh
cd <engine> && cargo run --bin play --features dev -- \
    --project <game> <game>/scenarios/smoke.lua <game>/out/
```

It plays the game's startup scene (`build.startup_scene` in `project.rusty`), the
same one the standalone player boots; `--scene <path>`, relative to the game like
the startup scene, plays another. The scenario and output paths are read from where
the command runs. It writes `results.json` and `console.log` into the output folder. See `docs/testing.md` for
scenarios and captures, and `docs/ui.md` for the in-game UI.

## The MCP

The tools describe themselves; what they don't say:
- read **`rusty://scripting-api.md`** before writing Lua: it is the whole API
  (`docs/api/`, one page per namespace);
- `eval` runs one line of that API; `snapshot` returns the world as JSON;
- to play-test the game's own scene, in the window or headless, one `eval` each:
  `Time.Pause()` first, then `Editor.EnterPlay()`, then `Input.Press`, `Time.Step(n)`
  and `snapshot`, then `Editor.ExitPlay()` to get the authored scene back and
  `Time.Resume()` last, so the next Play runs live (`docs/api/Editor.md`). Globals
  set before a switch are gone after it;
- when the engine warned during a call, the result carries a second text block
  starting `engine warnings:`. Read it: a skipped asset or a dropped collider shows
  up there first;
- `.mcp.json` registers two servers, each the checkout's built
  `target/debug/session-mcp`. **Use one at a time**, and approve only the one this
  session will use. `rusty` attaches to the editor window the operator watches; open
  it first, from the checkout, with a `dev` build (a build without `dev` has no
  command socket): `cd <engine> && cargo run --bin rusty --features dev -- --project <game>`.
  The app search's **Rusty** (`make launcher`) opens that same dev build.
  `rusty-headless` runs its own world with no window, for when none is open. Two
  worlds saving the same scene overwrite each other;
- if a server fails to start, build it in the checkout, with the editor beside it:
  `cd <engine> && cargo build --features dev --bin session-mcp --bin rusty`.

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
