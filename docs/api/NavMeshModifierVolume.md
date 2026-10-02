## `NavMeshModifierVolume`

A box that assigns a **navigation area** to the walkable surface inside it: Unity's
`NavMeshModifierVolume` (#460). A strip of mud that costs more to cross, an open street
in an AWP's sightline, a spawn zone some bots may not enter. What an area *means* (its
cost, who may enter it) lives in the scene's area table and the agents' masks (see
`Navigation`, *Areas and costs*); the volume only says *where* an area is.

The box is `Center` ± `Size / 2` **local to the entity**: it moves, turns and scales with
the `Transform`. The defaults are Unity's: a 4 × 3 × 4 box raised 1 m, assigning area `0`
(`Walkable`). At bake time every walkable floor whose point (its cell centre at the floor's
height) lies inside the box takes the volume's area, so a volume around the ground floor
leaves the deck above it alone unless the box reaches up to it. Area **`1`
(`NotWalkable`) removes the floor** instead, and the agent-radius erosion pulls the
surface back from it as from a wall. Where volumes overlap, `NotWalkable` wins, then the
highest area id. Look an area's id up with `Navigation.GetAreaFromName`.

A volume is a bake input: in Play, an added, moved, resized, re-targeted or toggled volume
rebakes only the cells its old and new boxes cover on the next tick, giving exactly what a
full bake gives (see `Navigation`, *Keeping the navmesh current*). Changing an area's
**cost** needs no rebake at all.

Getters return a neutral default (`false`, zeros) without a volume; setters are then
no-ops. Add one with `Scene.AddComponent(id, "NavMeshModifierVolume")` (alias
`NavModifier`). `Debug.Snapshot` reports it as `nav_modifier`.

| Function | Signature | Returns |
|---|---|---|
| `NavMeshModifierVolume.GetCenter` / `SetCenter` | `(id)` / `(id, x, y, z)` | the box's centre, local to the entity (non-finite is ignored) |
| `NavMeshModifierVolume.GetSize` / `SetSize` | `(id)` / `(id, x, y, z)` | the box's full size; each side at least `0.001` (non-finite is ignored) |
| `NavMeshModifierVolume.GetArea` / `SetArea` | `(id)` / `(id, area)` | the area id it assigns, `0`–`31` (out of range is ignored) |
| `NavMeshModifierVolume.GetActive` / `SetActive` | `(id)` / `(id, bool)` | an inactive volume assigns nothing |
