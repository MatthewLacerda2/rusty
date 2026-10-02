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
  any rig ragdolls. The character must face **+Z** in its own space (the glTF
  convention) for "forward" to be right.
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
while any body is simulated.

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

Dismemberment, powered (motor-driven) ragdolls and blending back to animation for a
get-up are not part of this.

| Function | Signature | Returns |
|---|---|---|
| `Ragdoll.Build` | `(id [, { mass = kg }])` | `{ [bone_name] = bone_id, .. }`; errors without a skinned mesh |
| `Ragdoll.Enable` | `(id)` | how many bodies switched to physics |
| `Ragdoll.Disable` | `(id)` | how many bodies switched back to the Animator |
| `Ragdoll.IsEnabled` | `(id)` | `bool` |
