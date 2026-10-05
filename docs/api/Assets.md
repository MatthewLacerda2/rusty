## `Assets`

The project's importable assets — the "see what I can place" half of authoring.
`Assets.Manifest()` (alias `Assets.List()`) walks the open project's `assets/` folder, imports
every model file (`.gltf`/`.glb`/`.obj`), and returns a structured catalogue of the
addressable sub-objects inside each file plus their footprint (so the agent can lay
things out without overlap) and material count. Unlike the rest of the surface,
this returns a Lua **table** (not a scalar triple): the result is nested data.

Each `subObject`'s `reference` is the canonical `path::sub_object` string and
round-trips with `AssetRef` / `import_sub_mesh`, so it can be handed straight to the
mesh-instantiation path to place exactly what the manifest names. Files that fail to
import are skipped; a missing root yields an empty list. Output is deterministic
(files sorted by path, sub-objects in source order).

| Function | Signature | Returns |
|---|---|---|
| `Assets.Manifest` | `()` | array of asset tables (see shape below) |
| `Assets.List` | `()` | alias for `Assets.Manifest` |
| `Assets.Refresh` | `()` | `{ converted = { wavPath, ... }, skipped = { { path, reason }, ... } }` — imports what arrived (below) |

Returned shape (Lua, 1-indexed arrays):

```lua
{
  {
    path = "assets/models/crates.glb",
    materialCount = 2,            -- materials in the file's shared table
    subObjects = {
      {
        id = "Sedan",
        reference = "assets/models/crates.glb::Sedan",  -- round-trips with AssetRef
        materialCount = 1,         -- 0 or 1 (a sub-mesh uses at most one material)
        size = { x = 4.2, y = 1.5, z = 1.8 },            -- AABB extent (max - min)
        min  = { x = -2.1, y = 0.0, z = -0.9 },          -- absent if the mesh is empty
        max  = { x =  2.1, y = 1.5, z =  0.9 },
      },
    },
  },
}
```

### `Assets.Refresh`

Unity's `AssetDatabase.Refresh()`: import whatever arrived in the project's `assets/` tree since
the last look. Today that is audio: every `.mp3` anywhere under `assets/` is decoded
once, its encoder delay and padding trimmed, written as a 16-bit PCM `.wav` beside it
(same name, same channels and sample rate), and the `.mp3` is removed, so a built game
carries WAV, never MP3. OGG is never converted. The editor runs the same refresh at
boot and whenever its window regains focus, logging each conversion to the console;
call it yourself after writing or copying an `.mp3` into the project.

An MP3 is left in place, and listed in `skipped` with the reason, when its `.wav`
sibling already exists (a refresh never overwrites a file) or when it does not decode
(a file still being copied in; the next refresh retries it). An `AudioSource` or
`PlayAt` that still names `foo.mp3` after conversion plays `foo.wav`.

```lua
local r = Assets.Refresh()
for _, wav in ipairs(r.converted) do print("imported " .. wav) end
for _, s in ipairs(r.skipped) do print(s.path .. ": " .. s.reason) end
```
