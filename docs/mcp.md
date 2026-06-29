# MCP bridge — drive the engine from Claude Code

"Blender-MCP, but for game development" (#288). The `session-mcp` binary holds a
live engine session in **edit mode** and speaks the
[Model Context Protocol](https://modelcontextprotocol.io) over stdio, so an external
agent (Claude Code) attaches to the engine *natively* — listing tools, calling them,
and reading the API reference — instead of piping raw command lines.

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

**Tools** (`tools/list`, `tools/call`):

- `eval` — evaluate one line of the rusty Lua API against the live edit-mode world
  and return the result. This is the full control surface: create/edit/inspect
  entities, author assets, save scenes, step the sim, etc. Argument: `line` (string).
- `snapshot` — return a JSON snapshot of the live world (entities, components,
  transforms, play state). A convenience wrapper for `Debug.Snapshot()`; no arguments.

A tool whose Lua *fails* reports the error in-band (`isError: true` with the message
as text), per MCP convention — the agent sees it and recovers, the server stays up.

**Resources** (`resources/list`, `resources/read`):

- `rusty://scripting-api.md` — the full Lua API reference (`docs/scripting-api.md`),
  embedded in the binary at compile time so it always matches the build. The agent
  reads it to learn the namespaces and functions available to `eval`.

The transport is MCP stdio: newline-delimited JSON-RPC 2.0. **stdout carries only
JSON-RPC** — engine logs go to stderr or the in-memory console, never stdout.

## Running it

```
cargo run --bin session-mcp --features dev                 # boot the default scene
cargo run --bin session-mcp --features dev -- --empty      # start from an empty scene
cargo run --bin session-mcp --features dev -- <scene.json> # boot a specific scene
cargo run --bin session-mcp --features dev -- --project <dir> [<scene>|--empty]
```

`--project <dir>` changes the working directory to a project before booting, so the
engine's relative asset/scene paths resolve against it. It may precede any scene
form.

## Registering it with Claude Code

Build a dev binary once, then point Claude Code at it:

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
