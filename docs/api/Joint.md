## `Joint`

Read and tune an entity's `JointComponent` (#449) — Unity's `FixedJoint`,
`HingeJoint` and `CharacterJoint` as one component with a **kind**. It ties the
entity's Rigidbody to the **connected body**'s, or to a fixed point in the world
when there is none:

- `"Fixed"` welds the two bodies (breakable debris, a gun in a ragdoll's hand).
- `"Hinge"` leaves one rotation free, about the **axis** (doors, elbows, knees);
  with limits on, it turns only between `min` and `max` degrees.
- `"Ball"` leaves all three free (shoulders, hips, a hanging lamp); with limits
  on, it twists about the axis between `min` and `max` degrees, swings up to the
  **swing limit** (Unity's *swing 1*) about the **swing axis**, and up to the
  **swing 2 limit** about the axis perpendicular to both — a shoulder that swings
  far forward but little sideways. A scene saved before #675 has no swing 2 limit
  and gets the swing limit: a round cone.

The **anchor** (the pivot), the **axis** and the **swing axis** are in the
entity's local space; only the swing axis's part perpendicular to the axis counts.
The **connected anchor** is in the connected entity's local space — world space with no
connected body — and is used only with auto-configure off; auto-configure (the
default) puts it wherever the anchor sits. **The pose the bodies are in when the
joint is built is its rest pose** — at play-enter, or when its shape (kind, body,
anchors, axes, limits, collision flag) changes during play; the limits are
measured from it. **Both bodies need a collider**: a body without one does not
exist in the physics world, and a joint missing a body does nothing. A deactivated
joint entity's joint is removed until it is active again. `enable_collision` (off
by default, as in Unity) lets the two joined bodies still collide.

**Breaking.** A non-zero break force (newtons) or torque (newton-metres) breaks the
joint on the first tick it carries more: the joint and its `Joint` component are
destroyed and `OnJointBreak(id, force, torque)` fires on the entity's scripts. A
hanging 1 kg body carries ~9.81 N. `0` never breaks. Break thresholds are read
live, so changing them never rebuilds the joint.

Getters return a neutral default (`nil`, `false`, zeros) without a Joint; setters
are then no-ops. Add or remove one with `Scene.AddComponent(id, "Joint")` /
`RemoveComponent` (adding also adds a `RigidBody`). Anchors and axes travel as
`x, y, z`.

| Function | Signature | Returns |
|---|---|---|
| `Joint.GetKind` / `SetKind` | `(id)` / `(id, name)` | `"Fixed"`, `"Hinge"` or `"Ball"` (case-insensitive; unknown names are ignored) |
| `Joint.GetConnectedBody` / `SetConnectedBody` | `(id)` / `(id, otherId)` | the joined entity, or `nil` for the world |
| `Joint.GetAnchor` / `SetAnchor` | `(id)` / `(id, x, y, z)` | the pivot, local to the entity |
| `Joint.GetConnectedAnchor` / `SetConnectedAnchor` | `(id)` / `(id, x, y, z)` | the pivot on the connected body (world space with none) |
| `Joint.GetAutoConfigureConnectedAnchor` / `SetAutoConfigureConnectedAnchor` | `(id)` / `(id, bool)` | whether the connected anchor follows the anchor |
| `Joint.GetAxis` / `SetAxis` | `(id)` / `(id, x, y, z)` | the hinge / twist axis (normalized; a zero axis is ignored) |
| `Joint.GetUseLimits` / `SetUseLimits` | `(id)` / `(id, bool)` | whether the angle limits apply |
| `Joint.GetLimits` / `SetLimits` | `(id)` / `(id, min, max)` | the hinge / twist range in degrees (within ±179, ordered) |
| `Joint.GetSwingAxis` / `SetSwingAxis` | `(id)` / `(id, x, y, z)` | a Ball's swing-1 axis (default `0, 1, 0`; normalized; a zero axis is ignored) |
| `Joint.GetSwingLimit` / `SetSwingLimit` | `(id)` / `(id, degrees)` | a Ball's swing 1, about the swing axis (0..179) |
| `Joint.GetSwing2Limit` / `SetSwing2Limit` | `(id)` / `(id, degrees)` | a Ball's swing 2, about axis × swing axis (0..179) |
| `Joint.GetBreakForce` / `SetBreakForce` | `(id)` / `(id, newtons)` | the breaking force (`0` = never, ≥ 0) |
| `Joint.GetBreakTorque` / `SetBreakTorque` | `(id)` / `(id, newtonMetres)` | the breaking torque (`0` = never, ≥ 0) |
| `Joint.GetEnableCollision` / `SetEnableCollision` | `(id)` / `(id, bool)` | whether the joined bodies collide |

```lua
-- A door: hinged to the world about its left edge, swinging 0..110 degrees.
Scene.AddComponent(door, "Joint")
Joint.SetKind(door, "Hinge")
Joint.SetAnchor(door, -0.5, 0, 0)
Joint.SetAxis(door, 0, 1, 0)
Joint.SetUseLimits(door, true)
Joint.SetLimits(door, 0, 110)

-- A T-posed arm along +X: twists about itself, swings 80 degrees forward/back
-- (swing 1, about Y) but only 20 up/down (swing 2, about X × Y = Z).
Joint.SetKind(arm, "Ball")
Joint.SetAxis(arm, 1, 0, 0)
Joint.SetSwingAxis(arm, 0, 1, 0)
Joint.SetUseLimits(arm, true)
Joint.SetSwingLimit(arm, 80)
Joint.SetSwing2Limit(arm, 20)

-- A lamp that falls when shot hard enough.
Joint.SetBreakForce(lamp, 200)
```
