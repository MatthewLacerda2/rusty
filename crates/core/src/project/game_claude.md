# {name}

{name} is a game built on the **rusty** engine; this folder is its rusty project.

The engine's rules and API come from the `rusty` MCP: read `rusty://game-guide.md`
first, then `rusty://scripting-api.md` before writing Lua. The engine checkout this
project was made with is `{engine}`; its `docs/` folder holds the same guides. If the
MCP server fails to start, or the editor is older than the engine, build both in the
checkout: `cd {engine} && cargo build --features dev --bin session-mcp --bin rusty`.

**Engine findings are filed, not fixed.** An engine bug or a missing capability
becomes an issue on `MatthewLacerda2/rusty` (the `agent` label, and a `Kind:` line as
the game guide says), and the game carries on. Never edit the engine from a game
session.

This file is the game's: the engine wrote it once and never touches it again.

## This game
