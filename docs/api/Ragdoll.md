## `Ragdoll`

Turn an animated character into a physics-driven body — on death, or a big hit —
and back (#466, `src/api/ragdoll.rs`). Unity's Ragdoll Wizard plus the switch every
Unity project writes by hand.

**Build.** `Ragdoll.Build(id)` (or **Build Ragdoll** on the inspector's Mesh card)
puts a `Rigidbody` on every bone of `id`'s skinned mesh that has a
[hitbox](Physics.md) — generating the hitboxes first when there are none — and a
[`Joint`](Joint.md) from each to its nearest ragdolled ancestor bone. The root
(the hips) is free. It returns `{ bone_name = bone_id }`.

- **Joints from a humanoid preset**, keyed by bone name, Mixamo first
  (`mixamorig:LeftForeArm`: the namespace and a `Left`/`Right` prefix are ignored,
  case too). Knees and elbows are **hinges**; hips, spine, neck, head, shoulders,
  arms, hands and feet are **ball** joints. Like Unity's wizard, each joint's
  axis is the one its bone flexes about, so the twist range is the flex range (a
  knee bends back up to 130° and not forward, an elbow forward up to 140°), swing 1
  is about the bend direction (a spine bending sideways, an arm raised) and swing 2
  about the bone itself. A bone the preset doesn't name gets a ±30° ball joint, so
  any rig ragdolls. The character must face **+Z** in model space (the glTF
  convention) for "forward" to be right; an `Armature` turned above the mesh (a
  Z-up Blender export) is undone first (#1822).
- **Mass** (`Ragdoll.Build(id, { mass = 70 })`, kilograms, the default) is split
  over the bodies by hitbox volume, so the chest outweighs a hand.
- Joined bones never collide with each other (`enable_collision` off); the other
  pairs do, so an arm lands on the chest rather than through it.
- **Ordinary components.** The bodies start kinematic, following the Animator.
  Tweak any joint or mass with `Joint.*` / the inspector; they are saved with the
  scene, by bone name (bones get fresh ids on every load, and a joint's connected
  bone is re-bound by name). Building again rebuilds them; a bone that no longer
  has a hitbox loses its Rigidbody and Joint.

**Switch.** `Ragdoll.Enable(id)` hands the bones from the Animator to physics:
every body turns dynamic at the pose it is in, **moving at the velocity its
animation gave it**, so a running character tumbles forward instead of freezing
and dropping. `Ragdoll.Disable(id)` hands them back: the bodies turn kinematic and
stop, and the Animator poses the bones again from the next tick (no blend).
Both take the skinned entity **or any ancestor of it** — the `root` a `Raycast`
returns — and return how many bodies switched. `Ragdoll.IsEnabled(id)` is `true`
while the ragdoll is on as a whole (`Enable`), so scripts can read it as "dead": a
hit reaction or a `SetSimulated` branch leaves it `false`.

- **Who poses a ragdolled bone.** The tick runs the physics step, then the
  Animator. Right after the Animator, every bone carried by a dynamic body is put
  back where its body is, so physics overrides the clip and `LateUpdate` scripts
  and skinning see the ragdoll. IK ([`Animator`](Animator.md), *Inverse
  kinematics*) skips any constraint touching such a bone, so physics wins over IK
  too. The Animator keeps running for the bones without a body (fingers, say).
- **Layers.** Alive, the hitboxes are on the `Hitbox` layer, which collides with
  nothing, so the character's capsule carries them. `Enable` moves them to the
  `Ragdoll` layer — created on first use, colliding with every layer except
  `Hitbox` — so the body lands on the floor and on other bodies; `Disable` moves
  them back. A shot that should hit corpses adds `Ragdoll` to its layer mask.
- **The capsule stays.** A character's `CharacterController` (or movement
  collider) is not touched: on death, deactivate it or move it to a layer the
  `Ragdoll` layer ignores, or the corpse lands on its own capsule (Unity's way).
- Throw the body the way the shot went with
  [`Physics.AddImpulseAtPosition`](Physics.md) on the bone the ray hit, **after**
  `Enable` (which sets each body's velocity).

```lua
function OnShot(root, bone, px, py, pz, dx, dy, dz)
  Ragdoll.Enable(root)
  Physics.AddImpulseAtPosition(bone, dx * 80, dy * 80, dz * 80, px, py, pz)
end
```

### Powered ragdoll and hit reactions

A shot that doesn't kill should **whip the part it hit** the way the bullet went,
and the enemy keeps fighting (#1895). That is physics blended with animation, one
branch of the body at a time — Unreal's `SetAllBodiesBelowSimulatePhysics`,
`PhysicsBlendWeight` and physical animation; Unity has no built-in equivalent. A
**branch** is a ragdoll bone and every ragdoll bone below it. Each call takes the
character (or any ancestor) and a bone of it — the `bone` a raycast returns, or an
id from `Ragdoll.Build`'s table — and returns how many bodies (or joints) it set.

- `Ragdoll.SetSimulated(id, bone, on)` switches the branch at `bone` to physics,
  or back to the Animator, and leaves the rest of the body as it is: the branch
  hangs from its animated parent by its joint. Each body starts moving at the
  velocity its animation gave it. A partly simulated branch **stays on the
  `Hitbox` layer**, which collides with nothing, so a flinching arm can't snag a
  door frame; only `Enable` moves bodies to `Ragdoll`. Switching off moves a
  branch on `Ragdoll` back to `Hitbox`.
- `Ragdoll.SetDrive(id, bone, { strength, damping })` sets every joint of the
  branch's [drive](Joint.md): a motor pulling each simulated body toward the
  rotation its **Animator** gives it relative to its parent (a bone the clip
  doesn't key is held to the pose it had when it went simulated). `strength` is
  a stiffness in 1/s² (rapier's acceleration-based motor: it scales with the
  bone's own mass, but not with the limbs hanging below it, so an upper arm
  carrying its forearm needs more than a hand). `0` is today's limp ragdoll; a
  few thousand lets a hit knock an arm well back and still brings it most of the
  way home within half a second; tens of thousands hold the animated pose
  tightly. `damping` (1/s) defaults to `2·√strength`. A field the table leaves
  out keeps each joint's current value, so `{ damping = 80 }` changes only the
  damping; the one exception is a call that changes `strength` and names no
  `damping`, which then takes `2·√strength`. A drive set any other way
  (`Joint.SetDriveStrength`, the inspector) chases the animation the same way
  once its body is simulated. A full `Enable` is driven
  the same way (a powered ragdoll), under a mirrored rig too (the target is the
  turn the bone is drawn with). The motors are set before the physics step, which
  runs before the Animator, so a drive chases the animated pose of the tick
  before: one tick behind. A driven body holding still is not woken by its drive,
  so a powered corpse at rest falls asleep.
- `Ragdoll.SetBlendWeight(id, bone, w)` (`0..1`) draws each simulated bone of
  the branch that far between its animated pose (`0`) and its body's (`1`, the
  default): translation lerped, rotation slerped, as a crossfade. The bodies
  themselves stay physics'; only what renders (and what the next tick's raycasts
  hit) blends. Setting it on a parent sets its whole branch. A weight is the
  bone's setting, like Unreal's `PhysicsBlendWeight`: it stays through a
  reaction ending, `Disable` and `SetSimulated(false)`, and goes only with the
  ragdoll (the character despawned, a scene load, a rebuild that takes the
  bone's body).
- `Ragdoll.HitReact(id, bone, ix, iy, iz, px, py, pz [, opts])` is the one call a
  game makes, from `OnTakeDamage` (see [`Damage`](Damage.md)): it simulates a
  branch, drives it, applies the impulse `(ix, iy, iz)` (N·s) at the world point
  `(px, py, pz)` and sets its weight to 1, then over `duration` eases the weight
  back to 0 (smoothstep) and hands the branch back to the Animator on a tick it is
  already drawn animated, so it never snaps. Options: `duration` (seconds, `0.4`),
  `strength` (`5000`) and `damping` (`2·√strength`), and `up_to` (a bone: its branch
  reacts instead). Returns `true`, or `false` — with one warning per entity —
  when `id` has no ragdoll at that bone. A NaN or infinite impulse or point
  starts the reaction but pushes nothing; a NaN or infinite `strength` or
  `damping` counts as `0`, a NaN or infinite `duration` as the default. The
  impulse goes on the hit body when it is simulated; when it isn't (a hips shot,
  whose branch starts below the hips, or an `up_to` branch that doesn't hold the
  hit), on the reacting branch's body nearest the point.
  - **Which branch.** By default the hit body's ragdolled parent's: a forearm shot
    moves the arm, a chest shot the torso. The hips never react: a hit on a bone
    whose parent is the hips reacts from the hit bone, and a hit on the hips
    from the hips' child body nearest the point. A bone with no body (a finger)
    counts as its nearest ragdolled ancestor.
  - **Repeated and overlapping hits.** A second hit on the same branch restarts
    its reaction. Several branches react at once; where they nest (a chest
    hit, then an arm hit), a bone is drawn at the **largest** weight of the
    reactions holding it, with that reaction's drive, so the arm takes its own
    fresh reaction and nothing jumps when either one ends. A reaction only hands back the bones it
    switched itself: a branch a script simulated, or a full `Enable`, takes the
    impulse and nothing else.
  - **IK.** A bone drawn blended (a reacting one, or one under
    `SetBlendWeight`) keeps its IK: IK solves on the animated side, and the
    blend is drawn over the result, so an aiming enemy hit in the chest keeps
    its aim and the reaction ends where the IK'd pose is. A fully simulated
    bone (weight 1, or `Enable`) is physics' alone, and IK skips it. A drive
    chases the clip's pose before IK, so an aiming arm under a reaction sags
    toward the clip by the weight, and comes back with no snap as it eases out.
  - **Time.** Reactions run on the fixed tick, so they pause with the game, scale
    with `Time.timeScale`, and two runs of the same hits give the same bones.
- **Runtime state.** Simulated branches, weights and running reactions are not
  saved, like `Enable`'s switch; `Enable` and `Disable` (the whole body) drop the
  reactions and partial switches they cover. A drive is a field of each `Joint`,
  so it is saved: set a character's drives once (in `Start`, or the inspector).

```lua
-- A shot flinches the limb it hit, harder the more damage it did.
function OnTakeDamage(id, amount, event)
  local hit = event.hit
  if event.kind == "point" and hit.bone then
    local d, p, s = hit.direction, hit.point, 0.4 * amount
    Ragdoll.HitReact(id, hit.bone, d.x * s, d.y * s, d.z * s,
      p.x, p.y, p.z, { duration = 0.35 })
  end
end
```

A get-up (blending a fallen ragdoll back to a get-up clip) and dismemberment are
not part of this.

| Function | Signature | Returns |
|---|---|---|
| `Ragdoll.Build` | `(id [, { mass = kg }])` | `{ [bone_name] = bone_id, .. }`; errors without a skinned mesh |
| `Ragdoll.Enable` | `(id)` | how many bodies switched to physics |
| `Ragdoll.Disable` | `(id)` | how many bodies switched back to the Animator |
| `Ragdoll.IsEnabled` | `(id)` | `bool`: on as a whole (`Enable`); a reaction or a branch is not |
| `Ragdoll.SetSimulated` | `(id, bone, on)` | how many bodies of the branch switched |
| `Ragdoll.SetDrive` | `(id, bone, { strength, damping })` | how many joints of the branch it set |
| `Ragdoll.SetBlendWeight` | `(id, bone, w)` | how many bones of the branch it set (`w` clamped to `0..1`) |
| `Ragdoll.HitReact` | `(id, bone, ix, iy, iz, px, py, pz [, { duration, strength, damping, up_to }])` | `true`, or `false` without a ragdoll there |
