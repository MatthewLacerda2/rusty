## `Assets`

The project's importable assets — the "see what I can place" half of authoring.
`Assets.Manifest()` (alias `Assets.List()`) walks the `project` asset root, imports
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

Returned shape (Lua, 1-indexed arrays):

```lua
{
  {
    path = "project/models/crates.glb",
    materialCount = 2,            -- materials in the file's shared table
    subObjects = {
      {
        id = "Sedan",
        reference = "project/models/crates.glb::Sedan",  -- round-trips with AssetRef
        materialCount = 1,         -- 0 or 1 (a sub-mesh uses at most one material)
        size = { x = 4.2, y = 1.5, z = 1.8 },            -- AABB extent (max - min)
        min  = { x = -2.1, y = 0.0, z = -0.9 },          -- absent if the mesh is empty
        max  = { x =  2.1, y = 1.5, z =  0.9 },
      },
    },
  },
}
```
