## `CharacterController`

Move a walking character with collide-and-slide — Unity's `CharacterController`
(#451). The component is an upright **capsule** that is the entity's collider in
the physics world. It moves **only** when a script calls `Move` with a
displacement: the capsule stops at walls and slides along them, climbs steps up to
the **step offset**, walks up slopes up to the **slope limit** and slides back off
steeper ones. It keeps a **skin width** gap to what it touches. A move shorter than
the **min move distance** does nothing.

**`Move` applies no gravity.** Scripts own gravity, jumps and air control (the feel
of the game lives in Lua): keep a vertical speed, add gravity to it each frame, and
pass it in the displacement. While grounded, a downhill or downstairs move snaps
back onto the ground, up to the step offset. `Move` takes effect immediately: it
writes the `Transform`, and the next physics tick carries the body there.

The capsule's `height` and `radius` scale with the `Transform` (height by its `y`
scale, radius by the larger of `x` and `z`), `center` is its offset in the entity's
local space, and it stays upright however the entity turns. A `Collider` on the
same entity is superseded by it; colliders on child entities (hitboxes) ride its
body. Its body is kinematic: between moves it sits wherever its `Transform` says,
and other characters, rays and triggers meet the same capsule `Move` sweeps.
Without a physics world (edit mode), `Move` moves the entity unobstructed.

**Crouching** is a smaller `height` (and, to keep the feet planted, a lower
`center`). Before standing back up, ask `CanStand(id, height)`: whether the capsule
at that height, its bottom where it is now, would overlap anything.

The collision flags are what the last `Move` touched: **below** (the ground, under
the lower cap's centre), **sides** and **above** (a ceiling). `IsGrounded` is
whether that move ended on the ground; `GetGroundNormal` is the ground's normal
then (`0, 1, 0` when airborne). Getters return a neutral default (`false`, zeros,
`0, 1, 0`) without a controller; setters and `Move` are then no-ops. Add one with
`Scene.AddComponent(id, "CharacterController")`. `Debug.Snapshot` reports it as
`character_controller`, the grounded state included.

| Function | Signature | Returns |
|---|---|---|
| `CharacterController.Move` | `(id, dx, dy, dz)` | `below, sides, above`: what the move touched |
| `CharacterController.IsGrounded` | `(id)` | whether the last move ended on the ground |
| `CharacterController.GetCollisionFlags` | `(id)` | `below, sides, above` from the last move |
| `CharacterController.GetGroundNormal` | `(id)` | `x, y, z`: the ground's normal after the last move |
| `CharacterController.CanStand` | `(id, height)` | whether the capsule fits at `height`, bottom kept |
| `CharacterController.GetHeight` / `SetHeight` | `(id)` / `(id, metres)` | full capsule height (≥ 0; under `2 * radius` it is a sphere) |
| `CharacterController.GetRadius` / `SetRadius` | `(id)` / `(id, metres)` | capsule radius (> 0) |
| `CharacterController.GetCenter` / `SetCenter` | `(id)` / `(id, x, y, z)` | the capsule's centre, local to the entity |
| `CharacterController.GetStepOffset` / `SetStepOffset` | `(id)` / `(id, metres)` | the tallest step climbed (≥ 0) |
| `CharacterController.GetSlopeLimit` / `SetSlopeLimit` | `(id)` / `(id, degrees)` | the steepest walkable slope (0..180) |
| `CharacterController.GetSkinWidth` / `SetSkinWidth` | `(id)` / `(id, metres)` | the contact gap (> 0) |
| `CharacterController.GetMinMoveDistance` / `SetMinMoveDistance` | `(id)` / `(id, metres)` | moves shorter than this do nothing (≥ 0) |

A NaN or infinite value (`math.huge`) takes the setter's floor: `0`, or `0.0001` for
the radius and skin width. `SetCenter` with a NaN or infinite part keeps the old centre.

### Hits and pushing

Each `Move` calls **`OnControllerColliderHit(id, hit)`** on the scripts of the
controller's entity (Unity's callback, #1879), before `Move` returns: once per
collider the move touched, in the order the sweep met them. **Static colliders
count**, as in Unity: a move that presses into the floor (its gravity) reports
the floor, so a script that only cares about walls checks `hit.normal.y`. Not
every grounded frame does: a move whose small downward part stays inside the
skin gap touches nothing. A move that touched nothing calls nothing; a
move root motion makes (an `Animator` driving the controller) reports its hits
with the tick's `OnAnimationEvent`s instead. A handler that calls `Move` on its
own controller moves it, but that nested move reports no hits (it would
recurse). A handler's error stops the rest and is raised by the `Move` call.

| `hit` field | What |
|---|---|
| `id` | the entity whose collider was hit (Unity's `hit.collider` / `gameObject`) |
| `root` | the hierarchy root of `id` |
| `rigidbody` | the entity whose Rigidbody `id` rides (kinematic or not: read `is_kinematic` with `Component.Get`); `nil` for static geometry or another character |
| `point` | `{x, y, z}`: where the collider was hit, in world space |
| `normal` | `{x, y, z}`: the hit surface's outward normal |
| `moveDirection` | `{x, y, z}`: the direction of the `Move` (unit length) |
| `moveLength` | how far the controller had travelled in this `Move` when it hit |
| `material` | the physics material asset `id`'s collider names, or `nil` |

**A controller pushes nothing on its own**: it stops at a crate as at a wall.
Pushing is a script, as in Unity, so a game decides what yields (a door can
refuse). The engine bundles Unity's documented one as
`assets/scripts/controller_push.lua`: add it beside the walking script and each
dynamic body the controller walks into takes the walk's direction as its
velocity, times its `push_power` field (default `2` m/s), keeping its own
vertical speed. It skips kinematic bodies, static geometry and hits while moving
down (`moveDirection.y < -0.3`: standing on the crate). `moveDirection` is the
whole move's, gravity included, so a slow walk pressed hard into the ground
(3 m/s with the player's `-1.33` m/s ground stick) reads as moving down and
pushes nothing; the bundled player walks at 5 m/s and pushes. Its core:

```lua with=id:CharacterController
local Push = { fields = { push_power = { type = "number", default = 2.0 } } }
function Push.OnControllerColliderHit(id, hit)
    local body = hit.rigidbody
    if not body then return end
    local rb = Component.Get(body, "RigidBody")
    if not rb or rb.is_kinematic or hit.moveDirection.y < -0.3 then return end
    local _, vy, _ = Physics.GetVelocity(body)
    local d, p = hit.moveDirection, Push.push_power
    Physics.SetVelocity(body, d.x * p, vy, d.z * p)
end
return Push
```

Defaults are Unity's: height `2`, radius `0.5`, centre `0, 0, 0`, step offset `0.3`,
slope limit `45`, skin width `0.08`, min move distance `0.001`.

```lua with=id:CharacterController
-- Walk, fall and jump: the script owns the vertical speed.
local Walker = { vy = 0.0 }
local GRAVITY, JUMP, SPEED = 20.0, 7.0, 5.0
function Walker.Update(id, dt)
    local vy = Walker.vy
    if CharacterController.IsGrounded(id) and vy < 0 then vy = -1.0 end
    if CharacterController.IsGrounded(id) and Input.IsKeyDown("SPACE") then vy = JUMP end
    vy = vy - GRAVITY * dt
    local below, sides, above = CharacterController.Move(id, SPEED * dt, vy * dt, 0)
    if above and vy > 0 then vy = 0 end -- bumped a ceiling
    Walker.vy = vy
end

-- Crouch, and stand only where there is room.
CharacterController.SetHeight(id, 1.0)
CharacterController.SetCenter(id, 0, -0.5, 0)
if CharacterController.CanStand(id, 2.0) then
    CharacterController.SetHeight(id, 2.0)
    CharacterController.SetCenter(id, 0, 0, 0)
end
```
