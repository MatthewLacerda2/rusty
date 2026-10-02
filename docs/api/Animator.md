## `Animator`

Drive an entity's animation clips. Clips are imported from the entity's skinned
glTF mesh (its `animations`); the named `clip` selects one to play. A keyframe
sampler poses the skeleton each fixed step, so playback — including a looping
clip's wrap — is deterministic. A non-looping clip holds its last frame at the
end.

The `Set*` parameter setters write the animator's named, typed **graph
parameters** (Unity's Animator parameters): the variables the animation graph's
transition conditions read each fixed step to decide state changes (#312).
A parameter is created on first set; once a name holds a type, a set of a
*different* type is ignored (a console warning, never an error). Parameter
values are saved with the entity, and `Debug.Snapshot` reports them under the
animator's `parameters` key.

**Animation graphs (#316).** An animator may reference an **`AnimationGraph`**
asset (a `.animgraph` file, #315) — a state machine whose nodes each play one
clip or a blend tree (below) and whose edges carry conditions over the parameters above. While a graph
is assigned and enabled, the engine evaluates it automatically each fixed step:
the **active node's** outgoing edges are checked in authored (priority) order,
and the **first** edge whose conditions *all* hold fires — a crossfade into the
target node's clip over the edge's `transition_duration` (0 is a hard cut),
adopting the target's `is_loop` flag and per-node speed (stacked on the
component's own speed multiplier). A `Trigger` condition on the fired edge is
consumed (auto-cleared) exactly once. An edge with no conditions never fires on
its own — jump it explicitly with `PlayNode`. On the first step after a graph
is assigned, the animator **binds** it: declared parameter defaults are seeded
(values a script already wrote win) and the entry node starts. Scripts just set
parameters; the character animates itself. `Debug.Snapshot` reports the binding
under the animator's `graph` / `graph_enabled` / `node` keys, and the graph
path + active state are saved with the entity.

| Function | Signature | Notes |
|---|---|---|
| `Animator.Play` | `(id, clip)` | Hard-cut the base layer to `clip` from its start (also releases a `Pause`). Raw clip-level control: it does not consult the graph — jump graph states with `PlayNode`. |
| `Animator.PlayAnimation` | `(id, clip)` | Like `Play`, but honest and queryable: returns `true` only when the entity's mesh actually carries a clip named `clip` (and an animator to play it); `false` otherwise, without side effects (`Play` silently no-ops on a missing clip). |
| `Animator.Crossfade` | `(id, clip, duration)` | Blend out of the current clip over `duration` seconds (a zero/negative duration, or a fade into the current clip, degrades to `Play`). |
| `Animator.Stop` | `(id)` | Halt playback (freezes the pose). |
| `Animator.Pause` | `(id)` | Hold the playhead where it is: the pose freezes but playback stays active (a hold, not a `Stop`). |
| `Animator.Resume` | `(id)` | Release a `Pause`, continuing from the held playhead. |
| `Animator.SetLooping` | `(id, loop)` | `loop = true` wraps the playhead at the current clip's end so it repeats seamlessly (idle/run/walk cycles); `false` (the default) holds the last frame. Persists across `Play`/`Crossfade`; entering a graph node overwrites it with the node's `is_loop`. |
| `Animator.SetBool` | `(id, name, value)` | Set the `Bool` parameter `name` (e.g. `Animator.SetBool(id, "isGrounded", true)`). |
| `Animator.SetFloat` | `(id, name, value)` | Set the `Float` parameter `name` (e.g. `Animator.SetFloat(id, "speed", 4.2)`). |
| `Animator.SetInt` | `(id, name, value)` | Set the `Int` parameter `name` (truncating: Lua numbers convert to a 32-bit integer). |
| `Animator.SetTrigger` | `(id, name)` | Latch the one-shot `Trigger` parameter `name`. The graph evaluator auto-clears it the moment it fires a transition; until then an unconsumed trigger stays latched — scripts never reset it by hand. |
| `Animator.SetGraph` | `(id, path)` | Assign the `.animgraph` asset at `path` to drive this animator (`""` clears it). Resets the active state, so evaluation re-binds — seeding declared defaults and entering the new graph's entry node — on the next fixed step. |
| `Animator.SetGraphEnabled` | `(id, enabled)` | Pause/resume graph auto-evaluation. Disabled, the graph is inert data and the animator stays under direct `Play`/`Crossfade` control; re-enabling resumes from the same active node. |
| `Animator.PlayNode` | `(id, node [, layer])` | Jump straight to the named graph node (state) of `layer` (default: the base layer), bypassing all conditions: a hard cut into its clip or blend tree, adopting its loop flag and speed. Returns `true` on success; `false` (with a console warning for a bad graph, unknown layer or unknown node) otherwise. |
| `Animator.GetBone` | `(id, name)` | The id of the bone GameObject named `name` (the glTF joint name, e.g. `"hand_r"`) in the entity's skeleton, or `nil` when the entity has no skinned mesh or no such bone. Bones are ordinary entities — see *Bones are GameObjects* below. |
| `Animator.GetCurrentNode` | `(id [, layer])` | The active graph node's name in `layer` (default: the base layer), or `nil` when there is no animator/graph/such layer or evaluation hasn't bound it yet. |
| `Animator.SetLayerWeight` | `(id, layer, weight)` | Set an extra layer's weight, clamped to `[0, 1]` (0 hides it, 1 applies it fully); returns `true` when set. `layer` is an index (1 = the first extra layer) or the layer's name. The base layer (0) is always at full weight, so it is refused with a warning, as is an unknown layer. |
| `Animator.GetLayerWeight` | `(id, layer)` | A layer's live weight (`1` for the base layer), or `nil` for an unknown layer. |
| `Animator.AddTwoBoneIK` | `(id, name, root, mid, tip)` | Add (or replace) a **two-bone IK** constraint named `name` over three bones of the entity's skeleton, by bone name: `root` (upper arm / thigh), `mid` (forearm / shin), `tip` (hand / foot). Saved with the animator at full weight; it does nothing until it has a target. Returns `true`; `false` with a warning when the entity has no animator or a bone doesn't exist. See *Inverse kinematics*. |
| `Animator.AddAimIK` | `(id, name, bones [, opts])` | Add (or replace) an **aim chain** named `name`: `bones` lists bone names root → tip (`{"spine", "chest", "neck", "head"}`), and the last bone's aim axis turns toward the target. `opts`: `weights` (each bone's share of the turn, parallel to `bones`; default even), `axis` (`{x, y, z}` in the last bone's local space; default `{0, 0, 1}`, glTF forward), `clamp` (the most the aim leaves the animated aim, in degrees; default 180) and `weight` (default 1). Same return as `AddTwoBoneIK`. |
| `Animator.RemoveIK` | `(id, name)` | Remove the IK constraint `name`, putting back the pose it last wrote; `true` when there was one. |
| `Animator.SetIKTarget` | `(id, name, x, y, z)` | Set constraint `name`'s target to a world-space point (Unity's `SetIKPosition` / `SetLookAtPosition`). Returns `true`; `false` with a warning for an unknown constraint. Targets are runtime state, not saved. |
| `Animator.SetIKTargetEntity` | `(id, name, target)` | Make constraint `name` reach for entity `target`'s live world position every step (a gun's foregrip, the player's head). Same return as `SetIKTarget`. |
| `Animator.SetIKHint` | `(id, name, x, y, z)` | Set a two-bone constraint's **hint** (pole) to a world point: the elbow or knee bends toward it (Unity's `SetIKHintPosition`). Ignored by aim chains. Same return as `SetIKTarget`. |
| `Animator.SetIKHintEntity` | `(id, name, hint)` | Like `SetIKHint`, following entity `hint`'s live world position. |
| `Animator.SetIKWeight` | `(id, name, weight)` | Set constraint `name`'s weight, clamped to `[0, 1]` (Unity's `SetIKPositionWeight` / `SetLookAtWeight`). 0 leaves the animated pose exactly as it was. Saved. Same return as `SetIKTarget`. |
| `Animator.GetIKWeight` | `(id, name)` | Constraint `name`'s weight, or `nil` when there is none. |

### Blend trees (#457)

A graph node can play a **blend tree** instead of one clip: several clips at
once, weighted by one or two `Float` parameters. Scripts only set the
parameters (`Animator.SetFloat(id, "speed", v)`); the weights follow each fixed
step. The children play **phase-synced**: one shared normalized time, each child
sampled at that fraction of its own length, so a walk and a run blended together
keep their feet in step. The tree's cycle length is the weight-averaged length
of its children.

| Kind | Reads | Weights |
|---|---|---|
| `Simple1D` | `parameter` | Children on a line by `threshold`. The two around the value cross-blend linearly (`speed` 0 idle, 2 walk, 6 run: at 4 it is half walk, half run); past either end the end child plays alone. |
| `FreeformDirectional2D` | `parameter_x`, `parameter_y` | Children at `position` points read as directions × speeds (forward, back, strafes, diagonals, an optional idle at the origin). Gradient-band interpolation in polar space, so between forward and a strafe the *direction* turns rather than the speed dropping. |
| `FreeformCartesian2D` | `parameter_x`, `parameter_y` | The same in plain x/y, for two axes that are not a direction (aim pitch × lean). |

The weights always sum to 1. A parameter that is unset reads 0.

```json
{ "name": "Locomotion", "is_loop": true,
  "blend_tree": { "FreeformDirectional2D": {
    "parameter_x": "velX", "parameter_y": "velZ",
    "children": [ { "clip": "Idle",       "position": [0, 0] },
                  { "clip": "WalkFwd",    "position": [0, 2] },
                  { "clip": "WalkBack",   "position": [0, -2] },
                  { "clip": "StrafeLeft", "position": [-2, 0] },
                  { "clip": "StrafeRight","position": [2, 0] } ] } } }
```

A node has a `clip` or a `blend_tree`, never both. The graph refuses to load
when a tree reads a parameter that isn't a declared `Float`, has no children, or
puts two children on the same point. Nested trees are not supported.

### Layers and avatar masks (#457)

The graph's top-level `nodes`/`edges`/`entry` are the **base layer** (layer 0).
A `layers` list stacks further state machines over it, in order, each with its
own `nodes`, `edges` and `entry`, evaluated each fixed step against the same
shared parameters:

- **`weight`** (0..1, default 1): how much the layer shows. Change it at runtime
  with `Animator.SetLayerWeight`; fade a reload layer in and out by tweening it.
- **`blending`**: `Override` (default) blends toward the layer's pose by the
  weight; `Additive` adds the layer's motion **measured against that motion's
  first frame** (Unity's default reference pose) — translation offset, rotation
  delta in the bone's local space, scale ratio — scaled by the weight. A clip
  whose bones never leave their first-frame pose adds nothing.
- **`mask`**: bone names, each meaning that bone and its whole subtree
  (`["spine_01"]` is "spine and up"). Empty is the whole body.

A layer only touches bones that are inside its mask **and** keyed by what it
plays; every other bone keeps the pose from the layers below. So an upper-body
layer fires and reloads while the base layer's legs keep running, and an
additive flinch layer stacks on any pose. A trigger can fire a transition in
every layer that reads it in the same step; it is cleared after all layers have
been evaluated.

```json
{ "parameters": { "speed": { "Float": 0 }, "Fire": "Trigger" },
  "nodes": [ ... ], "edges": [ ... ], "entry": "Locomotion",
  "layers": [
    { "name": "UpperBody", "weight": 1, "blending": "Override", "mask": ["spine_01"],
      "nodes": [ { "name": "Aim", "clip": "RifleAim", "is_loop": true },
                 { "name": "Shoot", "clip": "RifleFire" } ],
      "edges": [ { "from": "Aim", "to": "Shoot", "transition_duration": 0.05,
                   "conditions": [ { "Trigger": { "parameter": "Fire" } } ] } ],
      "entry": "Aim" } ] }
```

Layers are addressed by index (0 = base, 1 = the first entry of `layers`) or by
name: `Animator.SetLayerWeight(id, "UpperBody", 0.5)`,
`Animator.GetCurrentNode(id, "UpperBody")`, `Animator.PlayNode(id, "Shoot",
"UpperBody")`. A graph written before layers existed is simply a graph with no
extra layers.

### Bones are GameObjects (#453)

Instantiating a skinned model spawns its **skeleton as child entities** of the
skinned entity: one entity per joint, named after the glTF joint node, arranged
in the joint hierarchy (Unity's model). Each bone has a `Transform` and nothing
else, and is an ordinary entity in every API — `Transform.*`, `Scene.SetParent`,
`Physics`, the snapshot.

- **The Animator writes the pose.** Each fixed step it writes the sampled local
  Transform of every bone its clips animate, across all layers (a crossfade blends the two poses in
  TRS space: translation and scale lerp, rotation slerps, so limbs keep their
  length). A bone no playing clip animates is left alone.
- **Later writers override it, in a fixed order each tick.** The Animator writes
  the clip pose; a ragdolled bone is then put back where its physics body is;
  `LateUpdate` scripts (procedural recoil, a hand-made aim, setting IK targets)
  run next; IK (below) bends the result last, skipping any bone a ragdoll's
  dynamic body carries (physics wins); hitboxes then follow the bones. The mesh is
  skinned from the bones once per frame after all of them, so what moved a bone
  last is what renders.
- **Attach by parenting.** A gun goes under `hand_r`:
  `Scene.SetParent(gun, Animator.GetBone(enemy, "hand_r"))`.
- **Saving.** Bones are rebuilt from the model on load, so the scene never
  stores the skeleton. It stores what you authored on it: a bone moved away from
  its rest pose is saved as a per-bone **override** (by bone name), and anything
  parented under a bone is saved with that bone's name. Both re-bind by name when
  the scene loads, so a re-exported skeleton keeps them; an override or
  attachment whose bone no longer exists is dropped with a warning in the log
  (the attachment stays under the skinned entity). A bone's `Rigidbody` and
  `Joint` (a ragdoll's, #466 — see [`Ragdoll`](Ragdoll.md)) are saved by bone
  name too, the joint's connected bone included. Any other component added to a
  bone itself is not saved: put a hitbox `Collider` on a child of the bone —
  `Physics.GenerateHitboxes(id)` does exactly that for every bone (see
  `Physics`, *Per-bone hitboxes*, #464).
- **Destroying** the skinned entity destroys its skeleton and what hangs from it.

`Debug.Snapshot()` leaves bones out by default; `Debug.Snapshot({ bones = true })`
includes them. The editor's Hierarchy shows a skeleton collapsed.

### Inverse kinematics (#461)

IK bends the animated pose toward targets a script chooses, so an enemy aims
its torso and head at you whatever its legs are doing, a support hand stays on
the foregrip, and feet land on stairs. It is configured **on the Animator**: a
list of named constraints over the skeleton's bones (by bone name), saved with
the entity. Two kinds, after Unity's Animation Rigging:

- **Two-bone IK** (`AddTwoBoneIK`, Unity's `TwoBoneIK`) — an arm or a leg. The
  `root` and `mid` bones rotate so the `tip` lands on the target; the tip keeps
  its own local rotation. Out of reach, the limb straightens and points at the
  target. The elbow or knee bends toward the **hint** when one is set, else it
  keeps the animation's bend plane. The **weight** pulls the target from where
  the animation put the tip (0.5 reaches half-way).
- **Aim chain** (`AddAimIK`, Unity's `MultiAim` / `SetLookAt*`) — a short chain
  (spine → chest → neck → head) turns so its last bone's aim axis points at the
  target. Each bone takes its `weights` share of the turn, the turn never
  leaves the animated aim by more than `clamp` degrees, and the **weight** scales
  what is left.

**Targets are runtime state** (Unity sets IK goals every frame): set them from a
script with `SetIKTarget` (a point) or `SetIKTargetEntity` (followed every step).
A constraint without a target leaves the pose alone, and a weight of 0 is an
exact no-op.

**When it runs.** Each fixed step: the Animator poses the bones, ragdolled
bones are put back where physics left them, `LateUpdate` scripts run (the place to set this tick's targets), then **IK**, then
hitboxes follow the bones (so a shot hits the IK'd pose, `Physics`, *Per-bone
hitboxes*), and the mesh is skinned in `Render`. Aim chains solve before limbs,
so a hand reaching for a gun held by the aimed arm reaches where the aim put it.
IK post-processes each step's pose: what it wrote last step is put back first
(unless something else has moved the bone since), so a bone no clip animates
does not drift. A constraint touching a bone carried by a **dynamic
`Rigidbody`** (a ragdolled limb) is skipped: physics wins. IK runs in Play only.

```lua
-- Start: an enemy that looks at the player and keeps its left hand on the gun.
Animator.AddAimIK(id, "Look", { "spine", "chest", "neck", "head" },
  { weights = { 0.2, 0.3, 0.2, 0.3 }, clamp = 70 })
Animator.AddTwoBoneIK(id, "LeftHand", "upperarm_l", "lowerarm_l", "hand_l")
Animator.SetIKTargetEntity(id, "LeftHand", foregrip)
-- LateUpdate: track the player.
local x, y, z = Transform.GetPosition(player)
Animator.SetIKTarget(id, "Look", x, y + 1.6, z)
```

Foot placement is a two-bone constraint per leg whose target a script sets from
a downward `Physics.Raycast`; the engine does no ground probing of its own.
