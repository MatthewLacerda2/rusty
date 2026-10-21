## `Transform`

Per-entity position / rotation (Euler degrees) / scale, in Unity's two spaces:

- **World space** — `GetPosition` / `SetPosition`, `GetRotation` / `SetRotation`,
  `GetLossyScale` and `MoveTowards`: where the entity is in the world, through every
  parent's transform. For an entity without a parent, world and local are the same.
- **Local space** — `GetLocalPosition` / `SetLocalPosition`, `GetLocalRotation` /
  `SetLocalRotation`, `GetLocalScale` / `SetLocalScale`: the entity's own Transform,
  relative to its parent (what the inspector and the scene file store).

Scale follows Unity's split: `SetLocalScale` is the only scale a script writes, and
`GetLossyScale` reads the resulting world scale (signed when the hierarchy mirrors it;
exact unless a turned child under a non-uniformly scaled parent shears). `SetPosition`
under a parent with a zero scale keeps the local value it cannot solve for, as Unity
does. Every write moves the entity's children and colliders with it.

| Function | Signature | Returns |
|---|---|---|
| `Transform.GetPosition` | `(id)` | `x, y, z` (world) |
| `Transform.SetPosition` | `(id, x, y, z)` | — (world) |
| `Transform.GetLocalPosition` | `(id)` | `x, y, z` (relative to the parent) |
| `Transform.SetLocalPosition` | `(id, x, y, z)` | — (relative to the parent) |
| `Transform.GetRotation` | `(id)` | `x, y, z` (world, Euler degrees) |
| `Transform.SetRotation` | `(id, x, y, z)` | — (world, Euler degrees) |
| `Transform.GetLocalRotation` | `(id)` | `x, y, z` (relative to the parent, Euler degrees) |
| `Transform.SetLocalRotation` | `(id, x, y, z)` | — (relative to the parent, Euler degrees) |
| `Transform.GetLocalScale` | `(id)` | `x, y, z` |
| `Transform.SetLocalScale` | `(id, x, y, z)` | — |
| `Transform.GetLossyScale` | `(id)` | `x, y, z` (world scale; read-only, like Unity's `lossyScale`) |
| `Transform.TransformPoint` | `(id, x, y, z)` | `x, y, z` (a point in the entity's local space, in world space) |
| `Transform.TransformDirection` | `(id, x, y, z)` | `x, y, z` (a local direction in world space: turned, not scaled or moved, and mirrored with the entity) |
| `Transform.InverseTransformPoint` | `(id, x, y, z)` | `x, y, z` (a world point in the entity's local space), or `nil` when its world scale has a zero axis |
| `Transform.MoveTowards` | `(id, tx, ty, tz, step)` | — (steps the world position toward a world target, snaps within `step`; a negative `step` moves away, like Unity's `Vector3.MoveTowards`) |

A missing entity reads as the origin, no rotation and unit scale; its setters do
nothing, and the space maps return the vector unchanged.

```lua with=hand,gun
Scene.SetParent(gun, hand)
Transform.SetPosition(hand, 2, 1, 0)
Transform.SetLocalPosition(gun, 0, 0, 0.3)
-- A muzzle 0.5 m in front of the gun, in world space, wherever the gun is parented.
local mx, my, mz = Transform.TransformPoint(gun, 0, 0, 0.5)
-- The gun's world position (2, 1, 0.3), and its offset from the hand (0, 0, 0.3).
local wx, wy, wz = Transform.GetPosition(gun)
local lx, ly, lz = Transform.GetLocalPosition(gun)
```
