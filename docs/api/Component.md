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
| `Component.Fields` | `(name)` | one row per field, in the order a saved scene writes them: `{ name, type, default, writable, read_only }` |

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
A table-valued field (`Collider.shape`, `Camera.target_texture`) is written whole:
read it with `Get`, change it, `Set` it back. Runtime state (a particle system's
live particles, a trail's points) is not a field and never appears.

### The inspector's rules, not a backdoor

- **Each field is written through the op the inspector's card calls**, so the
  clamping, sanitizing and side effects are the card's own: `mass = 5000` is
  stored as the card's upper bound, `1000`.
- **A field no op writes is read-only.** `Component.Fields` says so
  (`writable = false`, and `read_only` says why or names the verb that writes it,
  such as `Transform.SetLocalPosition` or `Animator.SetFloat`); `Set` on it is an
  error carrying the same reason. Simulation state (an agent's `velocity`, an
  animator's playhead) is read-only too.
- **Strict names:** an unknown component or field is an error naming the valid
  ones.
- **All or nothing:** every field is checked, then written in the order
  `Component.Fields` lists them; when one fails (a bad value, an unknown enum
  name) nothing is written. The order matters where one op resets another: an
  `AudioReverbZone`'s `params` are written before its `preset`, so a preset named
  beside them wins.
- **Where it lands:** the live world, as the inspector's write does. In edit mode
  it is saved with the scene; in Play it is discarded on Stop with the rest of the
  play-mode world, as in Unity.
- **Clearing a value:** a Lua table cannot hold `nil`, so a field that can be
  absent (a `Joint`'s `connected_body`) is cleared through its named setter, or
  by an empty string where the op reads one as nothing (an `Image`'s `texture`).

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
local t = Component.Get(cam, "Transform")               -- position, rotation, scale
local ok, err = pcall(Component.Set, cam, "Camera", { fvo = 60 })
assert(not ok and err:find("fov"))                      -- the error lists the fields
```
