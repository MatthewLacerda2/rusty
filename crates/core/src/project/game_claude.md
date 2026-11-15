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

**Start a wall, floor or ground from the engine's material library**
(`engine/textures/library/`, listed by `Texture.Ops()`): set its `wear`, `tint` and
`seed`, and its tile size as `world_size` (but `uv_mapping = "UV0"` and no
`world_size` when it reads a model's baked mesh maps). Write a new graph only when no
library material is near.

**Look at a material before it goes into a level.** Read `Texture.Stats` before
tuning a ramp, then render the material with `Debug.PreviewMaterial` on the mesh that
matches its use (`{ mesh = "plane", repeats = 3 }` for a wall or floor, the sphere or
cube for a prop) and open the PNG. Pass `repeats` to see it tile; leave it out to see
its real scale.

This file is the game's: the engine wrote it once and never touches it again.

## This game
