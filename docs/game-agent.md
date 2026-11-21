# Building a game on rusty

You are in a **game project**: a folder the rusty engine opens, holding one game.
This guide is the engine's half of your instructions (the game's half is the
project's own `CLAUDE.md`). The `rusty` MCP serves it as `rusty://game-guide.md`,
read from the engine checkout, so it always matches the engine you drive.
Paths below that start with `docs/` are in that checkout.

## The model: Unity's

- **Unity is the reference.** rusty copies Unity's runtime model and grows toward
  everything Unity has, a piece at a time, so a Unity feature may not exist yet:
  `docs/api/` is what exists today. When something is unclear, do what Unity does.
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

So a dev build records every Play session (the editor's, `play`'s, the dev
player's) as a demo in `saved/demos/`: the start (scene, settings, save data,
seed) and what reached the game tick by tick. When the developer reports a bug
from a playtest, replay their demo headless instead of guessing a scenario
(next section).

The editor also keeps a rolling log of how each frame ran, and what the console
said beside it, in `saved/logs/` (50 MB by default, the oldest dropped). When the
developer says "it hitched a minute ago, by the bar", read it instead of asking them
to reproduce it: `Debug.FrameLog{ last_seconds = 90, over_ms = 33 }` returns the
slow frames with their tick, camera position, heaviest GPU pass and system
(`docs/api/Debug.md`, *The frame log*).

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

- **Some files are the engine's.** Every edit-open (the editor, an agent session,
  `make export`) and every `play` run seeds the bundled scripts, the
  starter materials and, in a new project, `assets/scenes/default.scene`: a copy
  you never edited follows the engine's newer default, an edited one is kept, and a
  deleted default scene stays deleted (`.seeded`, at the root, records which is
  which). `assets/scripts/ui/` and `assets/prefabs/ui/` are the UI widget kit's and
  are **rewritten on every seeding**: never edit them (`docs/ui.md`). The shipped
  player seeds nothing: it boots read-only and writes only under `saved/`.
- **One model format inside: glTF.** `.obj` loads for static meshes; `.fbx` and
  `.blend` are converted to a cached glTF at import (`.blend` needs Blender 5 or
  later installed).
- **Test content is primitive**: greybox levels and fixtures from cubes, planes and
  capsules, not authored art.

## After an engine upgrade

Every merge to the engine's `main` is a new engine, and backward compatibility is
**not a concern** for it: an upgrade may break the game, and the game is what
moves. Opening the project with a newer engine doesn't ask; it records the
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

An upgrade may also change a texture graph the engine ships (an `engine/…` path
in a recipe, listed by `Texture.Ops()`), so a material instancing one can bake
differently after it: rebake (`Material.Rebake`) and look before carrying on.

## Enemy AI: the animation graph is the state machine

Build an enemy's decisions the way the animation is built, with no behaviour
tree and no separate state-machine asset: the enemy's **animation graph is its
state machine** (`docs/api/Animator.md`, *State callbacks*).

- **States and transitions are authored once**, in the enemy's `.animgraph`,
  shared by every bot of that kind: `Idle`, `Aim`, `Cover`, `Flank`, `Melee`,
  `Stagger`, each with its clip, edges whose conditions read parameters
  (`distPlayer < 15`, `canShoot`, `numShooters < 2`), and a `tag` on states
  that act alike (`"attack"` on `Aim` and `Fire`). A layer at weight 0 can hold
  decision states with no motion of their own.
- **A director script feeds the parameters** each `Update`: it reads the
  world (distances, line of sight with `Physics.Raycast`), counts how many
  bots are in each state (`Animator.GetCurrentNode` / `GetCurrentTag`), and
  sets every bot's parameters (`Animator.SetFloat(bot, "distPlayer", d)`,
  `SetInt(bot, "numShooters", n)`, `SetBool(bot, "isInPlayerView", seen)`). The
  graph picks the state; no script decides it.
- **The bot acts on `OnStateEnter`**: it switches on `info.state` or
  `info.tag` and starts the action (move to cover, open fire, charge), keeping
  per-state data in its own script. `OnStateUpdate` runs the action each step
  if it needs to; `OnStateExit` stops it.
- **Hits** reach the bot as `OnTakeDamage` (`docs/api/Damage.md`): the bot
  keeps its own health and sets a trigger (`Animator.SetTrigger(id,
  "stagger")`), so getting hit is one more transition of the same graph.

```lua
-- bot.lua, on each enemy (the director sets its parameters)
local M, action, health = {}, "idle", 100
function M.OnStateEnter(id, info)
  if info.tag == "attack" then action = "shoot"
  elseif info.state == "Cover" then action = "take_cover"
  elseif info.state == "Stagger" then action = "none" end
end
function M.OnTakeDamage(id, amount)
  health = health - amount
  Animator.SetTrigger(id, "stagger")
  return amount
end
return M
```

## Start a surface from the engine's library

The engine ships ten tiling materials as texture graphs (concrete, brick, painted
plaster, worn painted metal, tread plate, wood planks, ceramic tile, asphalt, sand
with pebbles, carpet), listed by `Texture.Ops()` under `engine/textures/library/`.
For a wall, floor or ground, **start from the nearest one**: instance it in the
material's `maps`, set its `wear` (0 new, 1 derelict), its `tint` where it has one
and the instance `seed` (two walls that are not copies), and give the material the
tile's real size as `world_size` (`docs/api/Texture.md`, *The library*). The one
exception is `metal_painted_worn` fed a model's `Texture.BakeMeshMaps` maps: those
are that model's UV0 atlas, so its material maps by `uv_mapping = "UV0"`, with no
`world_size`. Write a new graph only when no library material is near, and build it
from the library's parts (`grunge`, `edge_wear`, `crevice_dirt`, `scratches`) where
they fit.

## Look at a material before it goes into a level

You never see a texture or a material unless you render it, so **look before you
use**. Read `Texture.Stats` on a recipe before tuning a `color_ramp` or `map_range`
(a noise does not span 0–1), and `Texture.Preview` for its intermediate maps
(`docs/api/Texture.md`). Then render the finished material with
`Debug.PreviewMaterial` (`docs/api/Debug.md`, *The preview*) on the mesh that
matches its use, and open the PNG: `{ mesh = "plane", repeats = 3 }` for a wall or
floor, which shows whether a stain repeats every tile and whether the normal map's
relief reads under a raking light; the sphere or cube for a prop. Pass `repeats` to
see it tile; leave it out to see its real scale (a world-mapped material at its own
`world_size`). `Debug.Preview`
does the same for a texture, model or shader file.

## Lights are baked unless they must change

A new light is `Baked` (`docs/api/Light.md`, *Mode*): a level is lit by its bake
(`Lighting.Generate()`), and a baked light costs nothing at runtime. Give a level's light
`Light.SetMode(id, "Realtime")` only when it must move, flash or switch while the game
runs (a flashlight, an alarm), and `"Mixed"` when a live light should also bounce from
the bake. A light a script creates while the game plays (a muzzle flash through
`Scene.CreateEntity` or `Scene.AddComponent`) is `Realtime` already, and casts
shadows that fade four ranges from the camera. Keep such a light cheap: a spot is one
shadow view where a point is six, a short range helps, and a flash too brief to read
a shadow from takes `Light.SetCastShadows(id, false)`. A prefab
instantiated at runtime keeps the mode its file says, so a prefab spawned in play
names `Realtime` for its lights. Until a bake takes a `Baked` light in, the editor and the
harness preview it live and the console names it; a shipped game draws it dark. So
bake before you judge a level's look, and read the console for lights waiting on one.

## Play-testing headlessly

A scenario is a Lua file under `scenarios/` that drives the game through the same
API plus `Harness.*` (press keys, step ticks, assert, read the run's warnings with
`Harness.Warnings()`); its `require` and `dofile` read
the project as a gameplay script's do. Run it with the `play` binary from the engine checkout:

```sh
cd <engine> && cargo run --bin play --features dev -- \
    --project <game> <game>/scenarios/smoke.lua <game>/out/
```

It plays the game's startup scene (`build.startup_scene` in `project.rusty`), the
same one the standalone player boots; `--scene <path>`, relative to the game like
the startup scene, plays another. The scenario and output paths are read from where
the command runs. It writes `results.json` and `console.log` into the output folder. `--demo <saved/demos/…>.demo` replays a recorded Play session instead,
on the scene it recorded, with an optional scenario on top for screenshots and
asserts; it fails if the game no longer plays it the same. See `docs/testing.md` for
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
- every edit-mode `eval` that changes something is one Undo step: `Editor.Undo()`
  takes back the last one (the operator's Ctrl+Z does the same), without reloading
  the scene and losing other unsaved work. The history is cleared when Play starts
  and when a scene loads, so an edit made before a play-test can't be undone after
  it (`docs/api/Editor.md`);
- a long bake (a Dust2-sized `Lighting.Generate()` takes over 30 minutes, past the
  client's tool timeout): attached to the editor, start it with
  `Lighting.GenerateAsync()`, which returns at once, poll `Lighting.IsRunning()`
  every few minutes, then read `Lighting.GetBakeStatus()` (`done`, or `failed` /
  `refused` with why) and `Lighting.GetBakeReport(kind)` (`docs/api/Lighting.md`).
  It runs unattended: a locked screen or a minimised window does not stop it
  (on Linux not yet tried: if the attached editor stops answering while hidden,
  bake headless).
  `Editor.EnterPlay()` errors until the bake ended.
  Headless, `Lighting.Generate()` is the only way; it sends progress to a client
  that asks for it (a progress token), which may or may not keep the call alive;
- when the engine warned during a call, the result carries a second text block
  starting `engine warnings:`. Read it: a skipped asset or a dropped collider shows
  up there first;
- `.mcp.json` registers two servers, each a built `session-mcp` of the checkout
  (`target/<profile>/session-mcp`; one you point at another profile's build there
  is kept). **Use one at a time**, and approve only the one this
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
