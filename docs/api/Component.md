## `Component`

Every serialized field of an entity's first-class components, read and written by
name (#1369) — Unity's public fields, which a script reads and writes with no
per-field binding. Where a namespace has a named setter (`Physics.SetMass`,
`Camera.SetFov`) both reach the same field; `Component` reaches every field, and
every field added later, with nothing new to bind.

| Function | Signature | Returns |
|---|---|---|
| `Component.Get` | `(id, name)` | the component as a table of its fields (`{ mass = 1, use_gravity = true, … }`), or `nil` when the entity has none |
| `Component.Set` | `(id, name, fields)` | — writes the fields named in `fields`, leaving the rest as they are; an error when the entity has no such component |
| `Component.Fields` | `(name)` | one row per field, in the order `Set` writes them: `{ name, type, default, writable, read_only }`; `type` is a Lua type, `"optional <type>"` for a field that can hold nothing |

**`name`** is any component name [`Scene.AddComponent`](Scene.md) takes (in any
case, aliases too), plus `Transform` and `Mesh`. Each answers to one name first:
`Transform`, `Mesh`, `Light`, `Animator`, `Collider`, `RigidBody`, `Material`,
`NavMeshAgent`, `NavMeshObstacle`, `OffMeshLink`, `NavMeshModifierVolume`,
`Camera`, `Particles`, `VisualCorrection`, `AudioSource`, `AudioReverbZone`,
`Canvas`, `RectTransform`, `Image`, `CanvasGroup`, `RectMask`, `Mask`,
`BackdropFilter`, `Text`, `Selectable`, `LayoutGroup`, `LayoutElement`, `Joint`,
`CharacterController`, `LODGroup`, `Trail`, `Line`, `Shape`. Scripts are not a
component here: they attach by path ([`Scene.AddScript`](Scene.md)) and are read
with `Scene.GetScript`.

**Fields** are named as a saved scene spells them (`snake_case`: `use_gravity`,
`render_order`); `Component.Fields` lists them. Values are what the scene file
holds: numbers, booleans, strings, a vector as a list (`{ 0, 1, 0 }`), an enum by
its name (`"Continuous"`, read in any case), a struct as a table of its fields.
Numbers are taken as the named setters take them: a whole-number float is an
integer (`render_order = 2.0`), and a bit mask is any integer kept to its low 32
bits (`culling_mask = -1` is every layer). A list entry that holds nothing (a
`Selectable`'s unset `sprites`) reads as a `nil` hole, and `Set` keeps the
entries after it; `{}` is an empty list where a list is expected.
A table-valued field (`Collider.shape`, `Camera.target_texture`) is written whole:
read it with `Get`, change it, `Set` it back. Runtime state (a particle system's
live particles, a trail's points) is not a field and never appears.

### The inspector's rules, not a backdoor

- **Each field is written through the op the inspector's card calls**, so the
  clamping, sanitizing and side effects are the card's own: `mass = 5000` is
  stored as the card's upper bound, `1000`. A scene or prefab file is held the
  same way when it loads: a hand-edited `mass: 5000` reads back `1000`, though the
  file keeps what it says until the next Save.
- **A field no op writes is read-only.** `Component.Fields` says so
  (`writable = false`, and `read_only` says why or names the verb that writes it,
  such as `Transform.SetLocalPosition` or `Animator.SetFloat`); `Set` on it is an
  error carrying the same reason. Simulation state (an agent's `velocity`, an
  animator's playhead) is read-only too.
- **Strict names:** an unknown component or field is an error naming the valid
  ones.
- **Read it, change it, `Set` it back.** A field left as `Get` returned it is
  skipped: no op runs, and a read-only field is not an error. So handing back a
  whole table from `Get` writes only what the script edited, even a frame or more
  later: a `velocity` the physics moved since is not put back, an animator's
  `time` that ticked is not an error, an edited `AudioReverbZone`'s `params` stay
  (its `preset` becomes `Custom`), and an unchanged Animator `graph` does not
  restart the state machine. `Get` remembers what it returned in the table's
  metatable; a table written by hand, copied, or read from another entity skips
  only the fields already holding the value given.
- **Fields one op writes together are written together.** A light's
  `inner_cone` / `outer_cone`, a Text's `auto_size` / `auto_size_min` /
  `auto_size_max`, an outline's width and colour: their op runs once, with each
  field's new value, or its current one when the call does not name it. So
  `{ inner_cone = 70, outer_cone = 80 }` widens both, even past the old outer cone.
- **All or nothing:** every field is checked, then written in the order
  `Component.Fields` lists them; when one fails (a bad value, an unknown enum
  name) nothing is written. The order matters where one op resets another: an
  `AudioReverbZone`'s `params` are written before its `preset`, so a preset that
  changes in the same call wins (it loads its own params).
- **Param maps insert, they don't replace.** A `VisualCorrection`'s `post_params`
  and a UI graphic's `shader.params` write each key given; a key removed from the
  table keeps its value. Renaming a UI `shader` drops the old shader's params, and
  the ones still in a `Get` table at their old values are not carried over.
- **Where it lands:** the live world, as the inspector's write does. In edit mode
  it is saved with the scene; in Play it is discarded on Stop with the rest of the
  play-mode world, as in Unity.
- **Clearing a value:** a Lua table cannot hold `nil`, so a field that can be
  absent (a `Joint`'s `connected_body`) is cleared through its named setter, or
  by an empty string where the op reads one as nothing (an `Image`'s `texture`,
  an Animator's `graph`).

```lua with=crate:RigidBody
local rb = Component.Get(crate, "RigidBody")
Component.Set(crate, "RigidBody", { mass = rb.mass * 4, collision_detection = "Continuous" })
assert(Component.Get(crate, "rigidbody").collision_detection == "Continuous")

-- What an inspector lists: names, types, defaults, and what can be written.
for _, f in ipairs(Component.Fields("Camera")) do
  print(f.name, f.type, f.writable, f.read_only)
end
```

```lua with=cam:Camera
Component.Set(cam, "Camera", { fov = 68, render_order = 1, clear_flags = "DepthOnly" })
local c = Component.Get(cam, "Camera")                  -- read it, change it, Set it back
c.fov = 70
Component.Set(cam, "Camera", c)
local t = Component.Get(cam, "Transform")               -- position, rotation, scale
local ok, err = pcall(Component.Set, cam, "Camera", { fvo = 60 })
assert(not ok and tostring(err):find("fov"))            -- the error lists the fields
```
