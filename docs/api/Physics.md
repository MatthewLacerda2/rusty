## `Physics`

Rigidbody control plus the spatial query surface over the live rapier world —
line casts, volume overlaps, and per-collider point queries (#311). Every query
routes through the same query pipeline the engine uses, so a script's query and
the engine's agree. Casts return
`(hit, entity_id, distance, px, py, pz, nx, ny, nz, bone, bone_name, root)` — the world-space
hit point and outward surface normal (#446), then the **bone** the hit struck, its
name, and the hierarchy **root** of what was hit (#464; see *Per-bone hitboxes*). Each group
is appended, so a caller that takes only the first values is unaffected. On a miss
every number after `hit` is `0` and the rest are `nil`.

| Function | Signature | Returns |
|---|---|---|
| `Physics.GetVelocity` | `(id)` | `x, y, z` |
| `Physics.SetVelocity` | `(id, vx, vy, vz)` | — |
| `Physics.GetAngularVelocity` | `(id)` | `x, y, z` (radians/sec per axis) |
| `Physics.SetAngularVelocity` | `(id, wx, wy, wz)` | — |
| `Physics.AddForce` | `(id, fx, fy, fz)` | — (continuous force, see below) |
| `Physics.AddForceAtPosition` | `(id, fx, fy, fz, px, py, pz)` | — (continuous force at a world point: moves and spins the body) |
| `Physics.AddImpulseAtPosition` | `(id, jx, jy, jz, px, py, pz)` | — (impulse in N·s at a world point — a shot hitting a ragdoll bone) |
| `Physics.SetKinematic` | `(id, is_kinematic)` | — |
| `Physics.GetCollisionDetection` | `(id)` | `"Discrete"` \| `"Continuous"` |
| `Physics.SetCollisionDetection` | `(id, mode)` | — (`mode` is `"Discrete"` or `"Continuous"`) |
| `Physics.Raycast` | `(ox, oy, oz, dx, dy, dz [, ignore_id [, layer_mask]])` | `hit, entity_id, distance, px, py, pz, nx, ny, nz, bone, bone_name, root` |
| `Physics.SphereCast` | `(ox, oy, oz, dx, dy, dz, radius [, ignore_id [, layer_mask]])` | `hit, entity_id, distance, px, py, pz, nx, ny, nz, bone, bone_name, root` |
| `Physics.RaycastAll` | `(ox, oy, oz, dx, dy, dz, max_distance [, layer_mask])` | array of `{id, distance, point = {x,y,z}, normal = {x,y,z}, bone, bone_name, root}`, nearest first |
| `Physics.OverlapSphere` | `(cx, cy, cz, radius [, layer_mask])` | array of entity ids |
| `Physics.OverlapBox` | `(cx, cy, cz, hx, hy, hz [, layer_mask])` | array of entity ids |
| `Physics.OverlapCapsule` | `(x0, y0, z0, x1, y1, z1, radius [, layer_mask])` | array of entity ids |
| `Physics.CheckSphere` | `(cx, cy, cz, radius [, layer_mask])` | `bool` |
| `Physics.CheckBox` | `(cx, cy, cz, hx, hy, hz [, layer_mask])` | `bool` |
| `Physics.ClosestPoint` | `(id, x, y, z)` | `found, cx, cy, cz` |
| `Physics.ContainsPoint` | `(id, x, y, z)` | `bool` |
| `Physics.GetBounds` | `(id)` | `found, min_x, min_y, min_z, max_x, max_y, max_z` |
| `Physics.GetColliderShape` | `(id)` | shape table (see below), or `nil` without a collider |
| `Physics.SetColliderShape` | `(id, shape)` | — (`shape` is the same table; errors on a bad one) |
| `Physics.GetPhysicsMaterial` | `(id)` | `friction, bounciness, friction_combine, bounce_combine` |
| `Physics.SetPhysicsMaterial` | `(id, friction, bounciness [, friction_combine [, bounce_combine]])` | — |
| `Physics.GenerateHitboxes` | `(id [, { bones = {..}, min_size = n, layer = "Hitbox" }])` | `{ [bone_name] = hitbox_id, .. }`; errors without a skinned mesh |

The optional trailing `ignore_id` skips one entity in the cast — pass the shooter's
own id so a shot can't hit its source. The engine has no built-in "don't hit the
player" rule; an entity is hittable simply if it exists **and is active**.

**Inactive entities are invisible to queries (#521).** A deactivated entity
(`SetActive(false)`) is skipped by every query — `Raycast`, `SphereCast`, the
`Overlap*`/`Check*` family, and the engine's own hitscan pass through it, and
`ClosestPoint`/`ContainsPoint` answer as if it had no collider — and a
`CharacterController.Move` passes through it instead of being blocked. Deactivating a
Rigidbody's entity hides its whole body, compound parts included; deactivating
one part hides just that part. Like the rest of the query world, activation is
picked up at the next physics tick, so a query in the same frame as the
`SetActive` still sees the old state. Reactivating makes it hittable again.

The optional `layer_mask` is a Unity-style bitmask (one bit per layer): the query
only reports entities whose layer's bit is set, ignoring all others. Build one from
a layer name with `1 << Layers.NameToIndex("Enemy")`, or OR several together. Omit
it (or pass `nil`) to hit every layer. This is independent of the **Layer Collision
Matrix** (Scene Settings), which governs which layers physically collide.

### Spatial queries (#311)

Every query except `GetBounds` asks the **live rapier world**, so it needs Play
mode — before Play, casts and checks miss/return `false` and overlaps are empty,
exactly like `Raycast`. Unity analogues: `Physics.OverlapSphere` / `OverlapBox` /
`OverlapCapsule` / `CheckSphere` / `CheckBox` / `SphereCast`,
`Collider.ClosestPoint`, and `Collider.bounds`.

- **`SphereCast`** is a raycast with thickness — the melee-swing / thick-projectile
  query. `distance` is how far the sphere's *center* traveled before impact; the
  point is where the sphere touched the struck surface.
- **`RaycastAll`** reports *every* collider the ray crosses within `max_distance`,
  sorted by distance (equal distances by entity id, so the order is
  deterministic). Each entry is where the ray *enters* that collider — the
  building block for wallbangs / over-penetration, which walk the list in Lua.
  A ray that starts inside a collider reports it at distance `0` (zero normal).
  Like the overlaps it takes an optional `layer_mask` but no `ignore_id` — skip
  the shooter's own id while walking the list.

  ```lua
  for _, h in ipairs(Physics.RaycastAll(ox,oy,oz, dx,dy,dz, 100)) do
    if h.id ~= self_id then
      Decals.Spawn(h.point.x, h.point.y, h.point.z, h.normal.x, h.normal.y, h.normal.z)
    end
  end
  ```
- **`OverlapSphere` / `OverlapBox` / `OverlapCapsule`** return the ids of every
  entity whose collider intersects the volume *right now* (the grenade-radius /
  zone-check query), as a Lua array sorted ascending. `OverlapBox` takes
  half-extents (`hx, hy, hz`, Unity's `halfExtents`) and is axis-aligned;
  `OverlapCapsule` spans the two sphere centers `p0`→`p1` with `radius`.
- **`CheckSphere` / `CheckBox`** are the boolean fast-path when you only need
  "is anything there?".
- **`ClosestPoint`** returns the nearest point on that entity's collider to the
  query point; a point inside the collider is its own closest point.
  `found=false` ⇒ the entity has no live collider (edit mode, or no collider) and
  the query point echoes back.
- **`ContainsPoint`** is `true` when the point is inside that entity's collider.
- **`GetBounds`** surfaces the collider's world-space AABB, which the engine keeps
  current on transform edits and each physics step — the one query that also works
  in edit mode. `found=false` ⇒ the entity has no collider component.

`AddForce` applies a **continuous force** (Unity's `ForceMode.Force`): the velocity
change for one call is `F / mass · fixedDeltaTime`, so applying the same force each
`Update` accelerates the body smoothly rather than in dt-independent jumps. It is a
no-op on kinematic bodies. For an instantaneous velocity change, set the velocity
directly with `SetVelocity`.

**Angular velocity** (Unity: `Rigidbody.angularVelocity`) is the other half of a
body's motion state — how fast, and about which axis, it spins, in radians/sec per
axis. `GetAngularVelocity` reads the value rapier integrated this tick (read the
tumble of a knocked prop, or check "is this still rotating?"); `SetAngularVelocity`
injects a spin between ticks (spawn a prop already rotating). Like `SetVelocity` it
takes effect on **dynamic** bodies; on a kinematic/static body it has no solver
effect (rapier drives their motion from the transform), matching Unity.

**Collision detection** (Unity: `Rigidbody.collisionDetectionMode`) chooses how a
body's contacts are found each fixed tick:

- **`"Discrete"`** (the default for every body class) tests overlap only at the
  tick's final pose. Cheap and correct for slow or large bodies.
- **`"Continuous"`** turns on CCD: the body's motion is swept from its previous
  pose to its new pose within the tick and stopped at the time of impact. This is
  what prevents **tunnelling** — a fast, small body (a bullet, a thrown prop)
  crossing thin geometry entirely between two ticks so no overlap ever exists to
  detect. A 100 m/s projectile moves ~1.6 m per 60 Hz tick, so anything thinner
  than that is passed straight through under Discrete.

Sweeping costs more, so flag only what is important or fast-moving for its size and
leave everything else Discrete. A kinematic body is a pure mover, so Continuous
only helps rapier account for its fast motion against dynamic bodies; it never stops
it at a wall. Walls stop a character only through `CharacterController.Move`. The mode is honoured at body build and re-applied
every tick, so flipping it mid-play takes effect immediately.

A rigidbody's **`use_gravity`** flag (authored in the inspector / serialized in the
scene) controls whether a body is pulled by the world's gravity (Unity:
`Rigidbody.useGravity`). On a **dynamic** body, `false` exempts it from gravity
(rapier `gravity_scale = 0`) while still letting it move under velocity and
collisions. A **kinematic** body ignores it, as in Unity: it is a pure mover that
goes exactly where its `Transform` says — it neither falls nor collides-and-slides
against walls (#451). A walking character that should be blocked, step up stairs
and know it is grounded uses a [`CharacterController`](CharacterController.md),
and its script applies gravity itself. An entity with a collider but **no
rigidbody** is likewise a kinematic mover. The flag is honoured at body build and
each tick, so toggling it at runtime takes effect.

### Collider shape and physics material (#447)

A collider's **shape** is a table with a `kind` and that kind's extents — what
`GetColliderShape` returns and `SetColliderShape` takes:

| `kind` | Fields |
|---|---|
| `"Box"` | `x, y, z` — full size |
| `"Sphere"` | `radius` |
| `"Cylinder"` | `radius, height` (along local Y) |
| `"Capsule"` | `radius, height, axis` — `axis` is `"X"`, `"Y"` (default) or `"Z"` |
| `"Mesh"` | `convex` — read-only: baked from an imported mesh, never set from script |

**Capsule** (Unity: `CapsuleCollider`) is the character and limb shape: a
cylinder capped by two hemispheres. As in Unity, `height` is the **full**
end-to-end length, caps included, so a `radius = 0.5, height = 2` capsule stands
2 m tall and rests with its centre 1 m above the floor; a height under
`2 * radius` is a sphere. World scale bakes in Unity's way — the length scales
with the axis' scale, the radius with the larger of the other two. Every extent
must be `> 0`. The shape is read when the body is built (entering Play, or a
hierarchy change); `SetColliderShape` refreshes the bounds (`GetBounds`)
immediately.

The **physics material** (Unity: `PhysicMaterial`) sets how a collider grips
and bounces:

- **`friction`** (`>= 0`, default `0.5`): `0` is ice, `1` is rubber on concrete.
- **`bounciness`** (`0`–`1`, default `0`): `0` absorbs an impact, `1` rebounds at
  full speed. A ball dropped from `h` rebounds to about `bounciness² · h`.
- **`friction_combine` / `bounce_combine`** — how two touching colliders'
  values combine: `"Average"` (default), `"Minimum"`, `"Multiply"` or
  `"Maximum"`. When the two colliders disagree, the later mode in that list wins
  (Unity's priority): a `Maximum`-bounce grenade still bounces off an `Average`
  wall, and a `Minimum`-friction ice floor stays slippery under any boot.

`SetPhysicsMaterial` clamps the coefficients into range, errors on a
non-finite one or an unknown mode name, and treats an omitted mode as
`"Average"`. Unlike the shape, a material edit reaches the solver on the next
physics step, mid-play included. The material is stored **inline** on each
collider rather than as a shared asset — four numbers, no asset to manage;
give each collider of a "family" the same values.

### Colliders and the parent hierarchy (#445)

Physics works in **world space**; a `Transform` is local to its parent. Every
collider is placed at its entity's world pose (its local pose composed through
all its parents), and a collider's world scale is baked into its shape — so a
collider on a child of a parent at `(10, 0, 0)` with local `(1, 0, 0)` sits at
world `x = 11`. Unity semantics:

- **Compound colliders.** A collider on an entity **without** its own Rigidbody
  joins the **nearest ancestor that has a Rigidbody**: it becomes an extra
  collider on that ancestor's body, at its pose relative to the ancestor. The
  whole compound moves, collides and rests as one body — how a multi-part prop
  or a character's per-bone hitboxes are built. Moving a part's `Transform`
  (script, animator) moves its collider on the body. With no Rigidbody ancestor
  the entity is its own body, as before (static, or an implicit kinematic body).
  A `CharacterController` counts as a body here too: colliders under it ride its
  capsule's body.
- **Hits name the part.** `Raycast`, `SphereCast`, the overlaps, `ClosestPoint`,
  `ContainsPoint`, trigger and collision events report the entity that **owns the collider**
  that was hit, never the body's root — so a script can tell a head hit from a
  torso hit. A deactivated part's collider stops generating contacts, trigger and
  collision events and is skipped by every query.
- **A child with its own Rigidbody** is its own body. A **dynamic** one is
  driven by physics: its world pose is what the solver says, and a moving parent
  does **not** carry it — each physics step writes the body's world pose back
  into the child's *local* `Transform`, relative to wherever the parent now is
  (after its parent moves `+10` on x, the child's local x drops by 10).
  A **kinematic** one is transform-driven, so it follows its parent like any
  other child. Static bodies are never written back.
- **Hierarchy changes apply on the next physics step.** Reparenting, adding or
  removing a Collider or Rigidbody, or activating/deactivating a collider
  rebuilds just the affected bodies at the start of the next fixed tick.

### Per-bone hitboxes (#464)

A skinned character's damage model is per body part: a head, a chest, limbs.
`Physics.GenerateHitboxes(id)` (or **Generate Hitboxes** on the inspector's Mesh
card) fits one collider per bone of `id`'s skinned mesh and returns
`{ bone_name = hitbox_id }`:

- **Fitted from the skin.** Each vertex belongs to the bone it is weighted to
  most. A bone's hitbox is the box around its vertices in the bone's own frame:
  a **capsule** along the longest side, or a **box** when the cross-section is
  flat (a chest, a pelvis, a hand). A bone spanning less than `min_size` (model
  units, default `0.08`) — a finger, an eye, a twist bone — hands its vertices to
  its parent, so the hand's hitbox covers the fingers. `bones = {"head", ..}`
  limits generation to those bones (the others fold into their nearest listed
  ancestor). Same skin, same hitboxes: regenerating refits the existing ones in
  place and removes those that no longer qualify.
- **On a child of the bone.** Each hitbox is an ordinary entity named `Hitbox`
  under its bone, carrying a plain `Collider`, so it stays hand-tweakable
  (`SetColliderShape`, `Transform.*`). Bones are rebuilt from the model on load,
  but a child of a bone is saved and re-attached to its bone by name, so the
  hitboxes survive save/load and a re-exported model.
- **On the `Hitbox` layer.** The layer (`layer = ".."` to choose another) is
  created on first use in a free slot, with every Collision Matrix cell off:
  hitboxes never push, block or trigger anything — a `CharacterController`
  walks through them — and only answer queries. An existing layer of that name
  keeps the matrix the project gave it.
- **They follow the animation.** The Animator moves the bone, the bone carries
  the hitbox. After the tick's last bone writer (`LateUpdate`), every collider
  under a bone is moved to its bone's pose for queries, so a cast in the next
  `Update` hits the pose that was on screen.

**Hits name the bone and the character.** `Raycast` / `SphereCast` return the
struck `bone` (the hit entity itself when it is a bone, else its nearest bone
ancestor; `nil` off a skeleton), its `bone_name` (the glTF joint name) and `root`, the top of the hit entity's hierarchy
(Unity's `Transform.root`: the character). `RaycastAll` entries carry `bone`,
`bone_name` and `root`.

**Convention: the capsule moves, the hitboxes take the shots.** Put the
character's movement capsule (its `CharacterController` or collider) on a layer
the shot's `layer_mask` leaves out, and the hitbox layer in it:

```lua
local SHOT = (1 << Layers.NameToIndex("Default")) | (1 << Layers.NameToIndex("Hitbox"))
local hit, id, dist, px, py, pz, nx, ny, nz, bone, part, root =
  Physics.Raycast(ox, oy, oz, dx, dy, dz, self_id, SHOT)
if hit and bone then
  Damage(root, part == "head" and 4 or 1)     -- "head", "spine", ...; multipliers are game-side Lua
end
```

A hitbox under a dynamic `Rigidbody` joins its body as a compound part and
takes a share of its mass; a character driven by a `CharacterController`
(kinematic) is unaffected. **Ragdolls** (#466) turn the hitboxes into jointed
bodies: see [`Ragdoll`](Ragdoll.md). A ragdolled hitbox moves to the `Ragdoll`
layer, so a shot that should hit corpses adds that layer to its mask.

### Mass, and forces at a point (#466)

A Rigidbody's **mass** (kilograms, default `1`) is its body's mass, whatever the
size of its colliders (Unity's `Rigidbody.mass`): it is spread over the body's
solid colliders in proportion to their volume (a trigger carries none), which
also sets the body's centre of mass and inertia. Changing it during Play takes
effect on the next step. So do `SetKinematic` and an entity's layer: the body
switches class in place (keeping its joints and contacts), and a collider follows
its layer's Collision Matrix row.

`AddImpulseAtPosition(id, jx, jy, jz, px, py, pz)` applies an impulse (N·s) at a
world point — Unity's `AddForceAtPosition(…, ForceMode.Impulse)`. It changes the
body's velocity by `J / mass` and its angular velocity by the torque
`(point − centre of mass) × J` through the body's inertia, so a push off-centre
spins it. `AddForceAtPosition` is the continuous-force version (`F · fixedDeltaTime`
per call, like `AddForce`). Both are no-ops on a kinematic body or without a
Rigidbody; outside Play only the linear part applies. Pass the hit point a
`Raycast` returned:

```lua
local hit, _, _, px, py, pz, _, _, _, bone, _, root = Physics.Raycast(ox, oy, oz, dx, dy, dz, me, SHOT)
if hit and bone then
  Ragdoll.Enable(root)                                   -- first: Enable sets each body's velocity
  Physics.AddImpulseAtPosition(bone, dx * 80, dy * 80, dz * 80, px, py, pz)
end
```
