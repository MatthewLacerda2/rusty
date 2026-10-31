# rusty engine

A 3D game engine built **without me ever looking at the code** — every line is
written by an AI coding agent.

rusty copies Unity's runtime model — GameObjects, components, and scripts with an
`Update()` loop — so if you've used Unity, you already know how to think in it. The
difference is *why* it exists: rusty is made to be **driven by a coding agent**, so
you build the game from your IDE (Claude Code, ideally) instead of clicking around an
editor — and a lot faster.

rusty aims to match **Unity** in quality and capability, built a piece at a time as
the work needs it — so today it has the traditional engine features a game needs, and
grows toward the rest — but with **full Claude support** from the start: the agent can
create and edit the whole game (scenes, GameObjects, assets, playtests, builds) through
one API, and the developer only says what they want done.

The north star is a game on par with **F.E.A.R.** (2005) or **Trepang2** — visceral
first-person combat carried by reactive enemy AI. The engine is "done enough" when an
agent could build a shooter of that caliber on it.

The second yardstick is an **offline Counter-Strike: Global Offensive** — bots,
bomb/defuse rounds, a buy menu and HUD, raycast gunplay with spread, recoil and
wallbangs, grenades, and multi-floor maps with jump spots, drops and ladders. rusty is
**offline only: no multiplayer, no networking, no online services — ever.**

## What it is

- **Unity-shaped.** Entities each have a `Transform` plus optional components (`Mesh`,
  `Camera`, `Light`, `Collider`, `Rigidbody`, `NavMeshAgent`, `Animator`,
  …). Behaviour lives in Lua scripts that act like MonoBehaviours, with lifecycle
  hooks (`Start`, `Update(dt)`, `OnTrigger`) — see
  [`docs/api/index.md`](docs/api/index.md#script-lifecycle-callbacks).
- **One scene at a time.** A scene is saved to disk as a plain document of references
  and values. Entering Play runs on a *clone*, and Stop restores your edits, so
  edit-mode is always what gets saved.
- **Scriptable against one stable API.** Gameplay calls a single set of namespaces —
  `Transform`, `Input`, `Time`, `Physics`, `Scene`, `Camera`, `Navigation`,
  `Animator`, `Material`. See [`docs/api/`](docs/api/index.md).
- **A real 3D engine underneath:** rendering, physics (rapier3d), navigation/navmesh,
  shadows, a skybox, and a post-processing chain.

## Assets

Authored 3D assets are `.blend` files saved straight from **Blender**, or
`glTF` / `glb` / `obj` / `fbx` files brought in, downloaded, or exported from
anywhere else. Inside, the engine reads one format, glTF 2.0 (`.obj` covers static
meshes); everything else is converted to glTF when it is imported, the way Unity read
only FBX and had Blender export one:

- an `.fbx` (binary or ASCII), skins and animation stacks included, by the `ufbx`
  library inside the engine. A texture file beside it is used only when the engine
  can load it (a PNG, JPEG or TGA whose bytes agree with its name); otherwise a
  copy embedded in the FBX stands in, or the texture is left out with a warning
  and the material keeps its colour. Colours follow Unity's (URP's) FBX import: a
  diffuse or emissive colour under a texture is white, so the texture shows as
  itself, and a Lambert or Phong colour without one is taken as is. As in Unity, a
  skipped emission texture leaves its colour glowing;
- a `.blend` by **Blender 5 or later**, which rusty requires: it runs Blender headless
  with the export settings pinned in one script (+Y up, modifiers applied, animations
  and skins on). Blender is found on `PATH`, then where it usually installs; set
  `RUSTY_BLENDER` to its executable for any other install. Without it, a `.blend`
  import fails saying so, and an unchanged `.blend` still loads its last import. An
  export that runs past ten minutes is stopped (`RUSTY_BLENDER_TIMEOUT` sets the limit
  in seconds). Images packed into the `.blend` are extracted and named after their
  image (a namesake from a linked library takes `.1`): a packed PNG or JPEG as its
  bytes, any other (BMP, TGA, EXR, a generated image) as an 8-bit PNG of its pixels,
  so a float image's colour is clamped to 0..1 (HDR maps are not imported). Images
  on disk are used where they lie, a linked library's found beside that library.

The converted copy is cached under `assets/generated/imported/` and re-made when the
source changes, or the Blender that made it does (outside Play, and for every source
as Play starts; Play itself never converts), while scenes keep naming the source. A
running game never starts Blender, not even to ask its version: it reads the cache as
it stands. Only the refresh before Play does, in the editor and, while a game still
ships with its sources (until #1267's export), in the standalone player; a `.blend`
missing from the project is served from its cache without asking Blender anything. The engine
never parses a `.blend` itself. Levels of detail
follow the Unity / Blender naming convention: objects named `Crate_LOD0`,
`Crate_LOD1`, … import as one `Crate` carrying an `LODGroup` that shows one level at
a time by on-screen size (see `docs/api/LODGroup.md`). A mesh's second UV map (glTF
`TEXCOORD_1`) is its lightmap UV for baked lighting ([`Lighting`](docs/api/Lighting.md)).

The editor picks up a file changed outside the engine without a restart, as Unity's
asset refresh does: a model re-exported from Blender, a texture saved from an image
editor, a shader or a script edited elsewhere. It watches the project's `assets/`
(and the engine's shaders) and reloads what changed on the next frame; a source
re-converts there and then. The open scene follows its file too, as long as it lies
under `assets/` (the only scenes watched): the file on disk wins, so the scene reloads,
and unsaved edits in the editor are discarded with a warning in the console, never a
dialog. The editor's own Save and Generate Lighting are not outside changes, and Play
refuses both, since the World then holds the play state. A change made during Play
waits for Stop. On a filesystem
that gives no change events (some network or container mounts), set
`RUSTY_WATCH_POLL=1` and it compares the files every second instead. Only the editor
watches: a headless or MCP-embedded session, and a shipped game, see an outside edit
to a shader only once it is baked again (`Shader.Bake`).

## Built to be played by an agent

The headline feature: the entire simulation runs **headlessly** — no window, no GPU —
because the sim knows nothing about rendering. So a coding agent can play your game
and report back instead of you opening the app every time. Five pieces, all speaking
the same API as gameplay, all compiled out of a shipped build (the `dev` feature):

- **Harness** — steps the game at full speed for as many ticks as you want, then dumps
  world state as JSON. Ten seconds of game time computes in milliseconds, and a fixed
  timestep makes every run reproducible.
- **Bot-players** — ordinary scripts that press the same keys a human would, so the
  agent can "play as the user".
- **Console + REPL** — evaluate API calls against the live game; the same evaluator
  backs the in-editor terminal and the headless runs, so they never drift.
- **MCP bridge** — the same evaluator spoken as the Model Context Protocol, so Claude
  Code attaches to a live edit-mode session natively and drives it like Blender-MCP
  drives Blender (see [`docs/mcp.md`](docs/mcp.md)).
- **Screenshots** — render a single frame offscreen to a PNG so the agent can actually
  *see* and critique a frame. **Editor captures** do the same for the whole editor
  (panels and viewport), so an editor change can be reviewed from a picture.

The payoff: you can trust your IDE — preferably Claude — to write, run, and play-test
the game for you, and only open a window when you want to.

## Running it

- `cargo run --bin rusty` — the editor, starting on the project picker
  (`-- --project <dir>` skips it).
- `make launcher` — put **Rusty** in the desktop's app search (the Linux app grid,
  Spotlight on macOS): it builds the dev editor and installs an entry that opens it
  from this checkout. `make launcher ARGS=--remove` takes it away. An app started
  this way gets the desktop session's environment, not your shell's: a provider key
  exported only in `.bashrc`/`.zshrc` belongs in `.env` (`docs/providers.md`), and
  `RUSTY_BLENDER` in the session's environment (`~/.config/environment.d/` on
  Linux, `launchctl setenv` on macOS).
- `cargo run --bin player` — the standalone player: the project's startup scene,
  straight into Play, full-window, no editor (see *Shipping a build* below).
- `cargo run --bin play --features dev -- [--project <dir>] [--scene <path>] <scenario.lua> <out_dir>` — the headless
  harness on the project's startup scene (or `--scene`); writes `results.json` + `console.log` (and any screenshots) to `<out_dir>`.
- `make editor-capture OUT=editor.png` — the editor over the default scene, headless,
  to a PNG (see [`docs/testing.md`](docs/testing.md) § Editor captures).
- `cargo run --bin session-mcp --features dev` — drive the live engine from Claude
  Code over MCP (see [`docs/mcp.md`](docs/mcp.md)).
- `cargo doc --no-deps` — the Rust API reference.

## Game projects

A game is a **project folder** that lives anywhere — its own directory, its own git
repo — and the engine opens it, the way Unity opens a project folder. Every binary
takes `--project <dir>`:

```sh
cargo run --bin rusty -- --project ~/games/horde  # the editor
cargo run --bin player -- --project ~/games/horde
cargo run --bin play --features dev -- --project ~/games/horde \
    ~/games/horde/scenarios/smoke.lua out/
```

Without `--project` the **editor** starts on the **project picker**, Unity Hub's
Projects page: the projects opened before (most recent first, with when; a folder
that has gone is greyed out and can be removed from the list), *New Project* (a name
and a parent folder) and *Open* (an existing project folder: one holding
`project.rusty` or `assets/`). A project last opened by an older engine opens
without asking; one last opened by a newer engine, or by a branch build, asks
before it opens. The list is yours, not the project's: it lives in
`$XDG_CONFIG_HOME/rusty/recent_projects.json` (default `~/.config/rusty/`) on Linux and
`~/Library/Application Support/rusty/` on macOS, and every project the editor opens,
with the picker or `--project`, goes to its top.

**rusty bench** is the same picker for agents: the session MCP's `bench_list`,
`bench_create`, `bench_open` and `bench_forget` tools list, make, open and forget
projects over that same list, with the same checks (see `docs/mcp.md`).

The other binaries (`player`, `play`, `session`, `session-mcp`) never show it: without
`--project` they open a packaged `project/` beside the executable, else `./project`
(created if missing). Opening a folder that doesn't exist yet creates it, as *New
Project* does. The engine writes the folder, `project.rusty` and two files for the
agent that builds the game, and nothing else — no `git init` and no root `.gitignore`;
version control is yours:

- `CLAUDE.md`, the game's instructions for Claude: that this is a rusty project, to
  read the engine's guide (`rusty://game-guide.md`, served by the MCP from
  `docs/game-agent.md`) first, where the engine checkout is, and that engine findings
  are filed as issues, not fixed from the game. It is the game's file, written once
  when absent and never touched again;
- `.mcp.json`, which registers the `rusty` MCP (attach mode, the editor window) and
  `rusty-headless` (embed mode) with Claude Code, both running the checkout's built
  `target/debug/session-mcp`. The engine owns only those two entries: a `dev` build
  rewrites them on every edit-open, with this machine's absolute paths, and keeps any
  other server the game registers. A build without `dev` never writes it.

Both are written when a project is opened to edit (*New Project*, `bench_create`, the
editor, the agent sessions and `make export`), so an existing project gets them on its
next edit-open. A project inside the engine checkout (its own `project/`) gets
neither, and a file that can't be written is a warning, never a failed open. A project looks like Unity's and Unreal's:

```text
<project>/
  assets/              the game: scenes/, scripts/, prefabs/, models/, textures/,
                       audio/, materials/, shaders/ (Unity's Assets/)
  project.rusty        marks the folder as a project: the engine commit it was
                       last opened with, and the build settings
  scenarios/           headless play-test scenarios (optional)
  cache/  saved/       regenerable output and save data; each ignores itself in git
  CLAUDE.md            the game's instructions for an agent
  .mcp.json            registers the engine's MCP with Claude Code
```

Every path the project stores — a scene's scripts and meshes, the startup scene —
is relative to the project root (`assets/scripts/bot.lua`), so the folder can move
or be cloned anywhere.

`project.rusty` is the project's one engine-owned file, like Unreal's `.uproject`
or Unity's `ProjectVersion.txt`. It records the **engine commit** the project was
last opened with (embedded in each engine build), that commit's place in history
(`git rev-list --count`), and the build settings. Every merge to `main` is a new
engine and backward compatibility is not a concern, so moving **forward** is the
normal case: a newer engine opens the project without asking, and an edit-open
records it and logs one `[Project]` warning naming the `git log` of what changed (on
stderr, in the editor console and in the MCP's replies), so the game can be brought
up to date (#1319). Only an
**older** engine, or a branch build (the same count, another commit), warns: one
`[Project]` line on stderr, a warning in the editor console, and the picker and
`bench_open` ask first. Then it proceeds, and the editor and the agent sessions
(`session`, `session-mcp`) record the running engine. The player, `play` and
`editor-capture` only check, never write. A dirty build (`<commit>-dirty`) counts as
its commit, and a build that can't name its commit (`unknown`, no git) or count it
(a shallow clone) can't be compared, so neither warns. A file that records no count
reads as an upgrade. A project without the file gets it on its first edit-open.

The engine's own content (shaders, the bundled scripts) is not part of a project:
it ships with the engine (`engine/`), and the scripts and starter materials are
seeded into each project from the binary.

A new project's first launch seeds a **default scene** into
`assets/scenes/default.scene`, once: a
greybox of fy_pool_day, built from boxes and planes — a walled yard with a sunken pool
between two spawns (a ramp at each end, translucent water you walk through), a raised
jacuzzi you jump into, and crates for cover. The Player starts at one end, and Enemy_1
starts behind a crate at the other and chases it down one ramp and up the other.
Seeding also bakes the scene's checker texture (`assets/textures/`) and an
example surface shader, `default_rim`, with its recipe (`assets/shaders/`). A default
scene you never edited follows the engine's newer one on a later launch; one you
edited is kept. The shader is re-baked from its recipe whenever the engine's shaders
change (#1638), so edit its recipe, not its `.wgsl`. A default scene you delete
stays deleted, as Unity never recreates a template's `SampleScene` (#1618). To get
the current default scene back, use File ▸ Reset Scene and save it.

The editor opens on the scene this machine last had open for the project (remembered
in `cache/last_scene`, never in `project.rusty`), then on the build's startup scene,
then on an empty Untitled scene; `session` and `session-mcp` boot the same way. The
player and `play` boot the startup scene.

## Shipping a build

A shipped game is the **player** binary built without the editor:

```sh
cargo build --release --bin player --no-default-features
```

`--no-default-features` drops the `editor` feature, so egui and the editor UI are not
compiled in (and `dev` is off, so neither is the harness or console). The player
reads the build settings in its project's `project.rusty` — the startup scene, the product name (window
title; a new project starts with its folder's name) and the first-launch window mode — which you edit in the editor under
**File → Build Settings** or from a script through the `Application` namespace. It
boots that scene straight into Play and exits when the game calls
`Application.Quit()` (a main-menu Quit button) or the window closes, saving
`Storage` on the way out.

The project it ships is an **export** of yours, Unity's Build for rusty's one
runtime:

```sh
make export PROJECT=~/games/horde OUT=~/builds/horde
```

It brings every converted model (`.fbx`, `.blend`) up to date in the import cache
(`assets/generated/imported/`), then copies `project.rusty` and `assets/` into
`OUT`, each model's import cache included, and leaves those sources behind, with
Blender's backups of them (`.blend1`, …). A source that fails to convert fails the
export, naming it; so does a texture a model reads from outside `assets/`, and a
symlinked folder under `assets/`. `cache/` and `saved/` are made empty rather than
copied, so the player's first launch writes nothing into the install. `PROJECT`
must be a project, and `OUT` a new or empty folder outside it. Files are copied in
sorted order, so two exports of one project are identical. The exported project
has nothing to convert, so the player never converts and never needs Blender;
scenes still name `soldier.fbx::Body` and `soldier.fbx#tex/Skin`, which resolve
through the cache. Run straight from a project folder instead, the player brings
its sources up to date before Play, as the editor does.

A shipped game is laid out with its project and the engine's content beside the
binary; the player finds both from wherever it is launched:

```text
MyGame/                       MyGame.app/Contents/
  player                        MacOS/player
  engine/   (from rusty)        Resources/engine/
  project/  (an export)        Resources/project/
```

`--project <dir>` still overrides. Packaging and installers are not covered yet.

For the engine's architecture and the conventions agents follow, see
[`CLAUDE.md`](CLAUDE.md); for the commit gate, testing, and the Lua scripting API, see
[`docs/`](docs/).
