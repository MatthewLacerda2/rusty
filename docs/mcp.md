# MCP bridge — drive the engine from Claude Code

"Blender-MCP, but for game development" (#288). The `session-mcp` binary speaks the
[Model Context Protocol](https://modelcontextprotocol.io) over stdio, so an external
agent (Claude Code) attaches to the engine *natively* — listing tools, calling them,
and reading the API reference — instead of piping raw command lines.

## Two modes: embed vs attach

The same bin runs in either of two modes, both driving the *same* evaluator:

- **embed** (default) — boots its **own** headless edit-mode world in-process and
  evaluates against it. There is **no window**: ideal for an agent authoring or
  play-testing logic solo.
- **attach** (`--attach [<addr>]`, #307) — forwards every tool call to a **running
  window's** command socket (#282) instead, so the agent drives the **live, rendered**
  world you are watching on screen. This is the "open the engine yourself, then tell
  Claude to drive it" workflow.

Attach mode **holds no connection when idle** and reconnects on each call, so a window
you close and reopen is picked up transparently by the next tool call — no manual
reconnect. Claude Code does not auto-restart an stdio MCP server that exits, so the
bridge deliberately stays up and reports an in-band error (*"no rusty engine reachable
…"*) when the window is down, rather than exiting.

## It is the same one evaluator

There is no second engine API. The bridge is a **third framing** over the engine's
single evaluator, exactly parallel to the two that already exist:

- `session` — line-Lua-in / line-JSON-out over stdin/stdout.
- the windowed command channel — the same protocol over a socket.
- `session-mcp` — the same `Session::eval`, exposed as MCP stdio JSON-RPC.

Every tool call ultimately runs one line of the rusty Lua API against the live world,
so the MCP surface, the console, and the headless session can never drift apart. The
server is pure Rust (only `serde_json` + std), synchronous, single-threaded, and runs
the session in-process — no subprocess, no async runtime, no extra dependency.

## What it exposes

<!-- docrefs: allow MCP's JSON-RPC method names, not repo paths -->
**Tools** (`tools/list`, `tools/call`):

- `eval` — evaluate one line of the rusty Lua API against the live edit-mode world
  and return the result. This is the full control surface: create/edit/inspect
  entities, author assets, save scenes, step the sim, etc. Argument: `line` (string).
- `snapshot` — return a JSON snapshot of the live world (entities, components,
  transforms, play state). A convenience wrapper for `Debug.Snapshot()`; no arguments.

**rusty bench** (#942) — the editor's project picker (Unity Hub's Projects page) as
tools, so an agent manages projects without the window. They share the picker's code
(`core::project::bench`) and the user's recent list (README § Game projects):

- `bench_list` — the projects opened before, most recent first: `name`, `path`,
  `opened_at` (Unix seconds), `ago` ("3 days ago") and `missing` (the folder is gone).
- `bench_create` — make the project `name` under the folder `parent`, the editor's
  *New Project*: the same checks (a plain name, an empty or new folder) and the same
  skeleton and `project.rusty`. It goes to the top of the list but is not opened.
- `bench_open` — open the project at `path` (one holding `project.rusty` or
  `assets/`). One last opened by a different engine is reported in-band instead of
  opened; call again with `confirm: true` to open it anyway (the picker's *Open
  anyway*). In **embed** mode the bridge then drives that project: like Unity's *Open
  Project*, the current one closes (unsaved scene edits are lost) and a fresh session
  boots on the new root's default scene, exactly as `--project <path>` would. In
  **attach** mode the window can't change project from here, so the answer carries
  `relaunch`, the command that starts the editor on it.
- `bench_forget` — take `path` off the recent list. The folder is left alone.

Each answers with JSON text. Paths are best absolute: a relative one resolves against
the open project's root.

A tool whose Lua *fails* reports the error in-band (`isError: true` with the message
as text), per MCP convention — the agent sees it and recovers, the server stays up.

**Engine warnings come back in-band.** When the engine warned or errored while a
call ran (an asset it skipped, a mesh collider it dropped), the `eval` or `snapshot`
result carries a **second text block** after the answer, starting `engine warnings:`
with one warning per line; the first block stays the answer, so a snapshot's JSON
still parses. Headless, it holds everything raised since the previous call (the first
call also gets what the boot raised); attached to a window, what was raised while
that call's line was evaluated, the rest showing in the window's console. A warning
that repeated in a row is shown once with its count, `… (×3)`.

**Resources** (`resources/list`, `resources/read`):

- `rusty://game-guide.md` — the game guide, `docs/game-agent.md` (#1313): the engine's rules
  for an agent building a game (the five kinds, determinism in scripts, the project layout,
  headless scenarios, how to file an engine finding) and a short manual for this MCP. A game
  project's `CLAUDE.md` points at it, so the engine's half of the instructions ships with the
  engine and never goes stale in a game. Read from the checkout on each request.
- `rusty://scripting-api.md` — the full Lua API reference (`docs/api/`, assembled into one
  document by `scripting::api_doc`), read from the engine checkout on each request so it is always
  the checkout's current doc (#931: an embedded doc rebuilt the crate on every doc edit). The agent
  reads it to learn the namespaces and functions available to `eval`.

**Server instructions.** The `initialize` answer carries a short `instructions` string
(MCP's server instructions) naming both resources and saying to read the game guide first.
It is a pointer only: Claude Code truncates long instructions, so the guide is never
inlined there.

The transport is MCP stdio: newline-delimited JSON-RPC 2.0. **stdout carries only
JSON-RPC** — engine logs go to stderr or the in-memory console, never stdout.

## Running it

```
# embed (own headless world):
cargo run --bin session-mcp --features dev                 # boot the default scene
cargo run --bin session-mcp --features dev -- --empty      # start from an empty scene
cargo run --bin session-mcp --features dev -- <scene.json> # boot a specific scene
cargo run --bin session-mcp --features dev -- --project <dir> [<scene>|--empty]

# attach (drive a running window):
cargo run --bin session-mcp --features dev -- --attach          # default socket
cargo run --bin session-mcp --features dev -- --attach <addr>   # explicit socket
```

`--project <dir>` (embed mode) opens that game project (README § Game projects): its
folder becomes the working directory, so the engine's project-relative asset and scene
paths resolve against it. Without it the bridge opens `./project`. It may sit before
or after the scene argument, and a scene path is relative to the project root.

`--attach` resolves the **same** socket address the window binds (so they always agree):
on unix `$RUSTY_CMD_SOCK`, else `$XDG_RUNTIME_DIR/rusty.sock`, else `/tmp/rusty.sock`;
on windows `$RUSTY_CMD_ADDR`, else `127.0.0.1:8787`. Pass `--attach <addr>` to override.
The window must be built with `--features dev` for its socket to exist.

## Registering it with Claude Code

A game project needs no registering: a `dev` build's edit-open writes the project's
`.mcp.json` (README § Game projects), which Claude Code offers the first time it starts
in the folder. It registers two servers, both the checkout's built
`<checkout>/target/debug/session-mcp`: `rusty` in attach mode (`--attach`, the editor
window) and `rusty-headless` in embed mode (`--project <root>`). They run the binary,
not `cargo run`: from a game folder cargo would ignore the checkout's
`rust-toolchain.toml` and `.cargo/config.toml` and cold-build the engine past Claude
Code's server startup timeout, so build it once (`cargo build --bin session-mcp
--features dev`). Use one at a time: two worlds saving the same scene overwrite each
other. The engine owns only those two entries under `mcpServers`, keeping any other
server and key in the file; the paths are absolute, so each edit-open rewrites them to
this machine's checkout.

To register it by hand instead, build a dev binary once, then point Claude Code at it:

```
cargo build --bin session-mcp --features dev
claude mcp add rusty -- /path/to/rusty/target/debug/session-mcp --project /path/to/project
```

Or run it straight from source (no separate build step):

```
claude mcp add rusty -- cargo run --quiet --bin session-mcp --features dev -- --project /path/to/project
```

Either way, Claude Code launches the bridge as an MCP server and gains the `eval` and
`snapshot` tools plus the scripting-API resource — the same engine an editor user
would drive, minus the window.

### Attaching to a window you can watch

To have the agent drive the engine **in front of you**, open the window yourself and
register the bridge in **attach** mode:

```
# 1. open the editor window on a project (its command socket binds once a
#    project is open; without --project, pick one in the project picker first):
cargo run --bin rusty --features dev -- --project /path/to/project

# 2. register the bridge in attach mode (build once, or run from source):
claude mcp add rusty -- /path/to/rusty/target/debug/session-mcp --attach
```

Now `eval`/`snapshot` mutate the **live, rendered** world: create an entity and it
appears in the viewport; `Lighting.Bake()`, the inspector-equivalent setters, and
Play/Step all drive the world on screen. Close and reopen the window freely — the next
tool call reconnects, and you never have to `/mcp reconnect`.
