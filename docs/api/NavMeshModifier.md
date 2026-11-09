## `NavMeshModifier`

How the navmesh bake reads one **object's colliders**, and by default its children's:
Unity's (AI Navigation's) `NavMeshModifier` (#1959). It follows the colliders' own
geometry, so there is no box to size and it moves with the object; a
`NavMeshModifierVolume` is the box-shaped tool for a region instead.

- **`OverrideArea`**: the object's floors bake as that area id. **`1` (`NotWalkable`)**
  is what furniture wants: a table, a sofa or a bar counter then has no walkable top, but
  stays solid geometry, so the floor under it still lacks the agent's headroom and agents
  walk around it. `nil` keeps the bake's own area. Where a `NavMeshModifierVolume` covers
  the same floor, the volume's area wins.
- **`IgnoreFromBuild`** leaves the colliders out of the bake entirely, as in Unity: the
  object neither has a floor nor blocks one, so the floor under a table is walkable. Use
  it for clutter agents walk through, not for an obstacle.
- **`ApplyToChildren`** (default `true`): the modifier covers the object's descendants.
  The one that covers a collider is the **nearest** active modifier, on its own object
  or, applied to children, on an ancestor: a child's own modifier wins over its parent's.
- An inactive modifier changes nothing, and does not shield what is below it from one
  further up.

A modifier is a bake input: adding, removing or changing one rebakes only the cells of
the colliders it covers on the next sync, giving exactly what a full bake gives (see
`Navigation`, *Keeping the navmesh current*), and it is part of the saved navmesh's
fingerprint.

Getters return a neutral default (`false`, `nil`) without a modifier; setters are then
no-ops. Add one with `Scene.AddComponent(id, "NavMeshModifier")` (alias `NavModifier`);
its fields are also `Component.Get` / `Component.Set`'s (`active`, `override_area`,
`ignore_from_build`, `apply_to_children`). `Debug.Snapshot` reports it as
`nav_mesh_modifier`.

| Function | Signature | Returns |
|---|---|---|
| `NavMeshModifier.GetOverrideArea` / `SetOverrideArea` | `(id)` / `(id, area)` | the area id the object bakes as, `0`–`31`, or `nil` for none (out of range is ignored) |
| `NavMeshModifier.GetIgnoreFromBuild` / `SetIgnoreFromBuild` | `(id)` / `(id, bool)` | whether the object is left out of the bake |
| `NavMeshModifier.GetApplyToChildren` / `SetApplyToChildren` | `(id)` / `(id, bool)` | whether it covers the object's descendants |
| `NavMeshModifier.GetActive` / `SetActive` | `(id)` / `(id, bool)` | an inactive modifier changes nothing |

```lua with=table:NavMeshModifier
NavMeshModifier.SetOverrideArea(table, Navigation.GetAreaFromName("NotWalkable"))
```
