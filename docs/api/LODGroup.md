## `LODGroup`

Read and tune an entity's `LodGroupComponent` (#472, `src/api/lod_group.rs`) —
Unity's `LODGroup`. It sits on a parent entity and lists its **levels of detail**,
finest first. Each level names the entities that render it (its **renderers**,
usually the group's children) and the **screen height** down to which it is shown:
the fraction of the viewport's height (`0..1`) the group's **size** covers at its
distance from the camera. Level `i` shows while that fraction is at least its
threshold; below the last level's threshold the whole group is culled. With levels
at `0.5`, `0.2` and `0.02`, LOD0 draws while the prop fills half the screen or
more, LOD1 down to a fifth, LOD2 down to 2%, and nothing smaller. Set the last
threshold to `0` to never cull.

- Levels are indexed from **0** (`LOD0` is the finest), matching the `_LOD0`
  naming. Thresholds never increase from one level to the next: a setter that would
  break the order is clamped to the neighbouring level's value.
- **Size** is in metres, before the entity's scale (the largest axis scale
  multiplies it). The distance is measured to the group entity's origin.
- An entity listed in no level is never hidden by the group; one listed in two
  levels is shown whenever either is.
- **Which level is shown is decided by the renderer, per camera, each frame, and is
  not readable from scripts** — the simulation never depends on it (determinism).
  Stacked cameras each pick their own level. Shadows follow the main camera's
  choice, so a shadow never shows a different level than its caster; static casters
  are baked at LOD0 (the bake is cached across frames). `Debug.Stats()` reports how
  many renderers a frame hid as `lod_hidden_entities`.

**Importing LODs.** A glTF whose objects are named `Crate_LOD0`, `Crate_LOD1`, … (the
Unity / Blender convention, case-insensitive; the glTF **node** name — Blender's
object name — or else the mesh name) is one prop at several levels. Instantiating
the whole file (the editor's *Instantiate into Scene*) spawns each such set as one
entity named `Crate` carrying an `LODGroup`, with one child per level;
`Scene.Instantiate("props.glb::Crate")` spawns just that set. The levels start at
`0.5`, `0.25`, … with the coarsest shown down to 1%, and the size is LOD0's largest
extent. The renderer references follow the group through prefab save / stamp.

Getters return a neutral default (`0`, `nil`, an empty table) without an LODGroup or
for an out-of-range level; setters are then no-ops. Add or remove one with
`Scene.AddComponent(id, "LODGroup")` / `RemoveComponent` (a fresh group has two
empty levels at `0.5` and `0.01`).

| Function | Signature | Returns |
|---|---|---|
| `LODGroup.GetSize` / `SetSize` | `(id)` / `(id, metres)` | the group's size (≥ 0.001) |
| `LODGroup.GetLevelCount` | `(id)` | how many levels |
| `LODGroup.GetLevelHeight` / `SetLevelHeight` | `(id, level)` / `(id, level, fraction)` | the level's screen-height threshold (`0..1`, kept between its neighbours'); `nil` out of range |
| `LODGroup.GetRenderers` / `SetRenderers` | `(id, level)` / `(id, level, {ids})` | the entity ids that render the level |
| `LODGroup.AddLevel` | `(id)` | appends a coarser, empty level at half the last threshold; returns its index |
| `LODGroup.RemoveLevel` | `(id, level)` | — (its renderers are no longer managed) |

```lua
-- A crate that swaps to a box past a few metres and disappears when tiny.
local crate = Scene.Instantiate("assets/props.glb::Crate")
LODGroup.SetLevelHeight(crate, 0, 0.4)
LODGroup.SetLevelHeight(crate, 1, 0.02)

-- Build one by hand over two children.
Scene.AddComponent(tree, "LODGroup")
LODGroup.SetSize(tree, 6)
LODGroup.SetRenderers(tree, 0, { treeHigh })
LODGroup.SetRenderers(tree, 1, { treeCard })
```
