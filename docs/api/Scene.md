## `Scene`

The structural-authoring surface: the API equivalent of the editor's GameObject
menu (create), the Hierarchy's Destroy and the inspector's parenting + Add
Component menu. Every verb routes through the shared `scene::authoring` module, so
it behaves identically to the editor and uses the same default values.

| Function | Signature | Returns |
|---|---|---|
| `Scene.FindEntityByName` | `(name)` | `id` of the first entity so named that is active in the hierarchy, as Unity's `GameObject.Find` (or `nil` if none, as `null` there) |
| `Scene.CreateEntity` | `(name, [primitive])` | new entity `id` |
| `Scene.Deactivate` | `(id)` | — |
| `Scene.DestroyEntity` | `(id)` | `true` if an entity existed at `id` |
| `Scene.AddComponent` | `(id, kind)` | `true` if the entity exists |
| `Scene.RemoveComponent` | `(id, kind)` | `true` if the entity exists |
| `Scene.AddScript` | `(id, path, [fields])` | `true` if the entity exists — see below |
| `Scene.SetParent` | `(id, parent_id)` | — (errors on a parenting cycle); a `nil` parent unparents, as Unity's `SetParent(null)` |
| `Scene.ClearParent` | `(id)` | — |
| `Scene.SetActive` | `(id, active)` | `true` if an entity existed at `id` — Unity's `SetActive`; its callbacks follow the edge (`OnEnable` / `OnDisable`) |
| `Scene.IsActive` | `(id)` | Unity's `activeSelf`: the entity's own `active` flag (`false` for a missing or `nil` id; an inactive ancestor still hides it) |
| `Scene.IsActiveInHierarchy` | `(id)` | Unity's `activeInHierarchy`: `true` when the entity and every ancestor are active (`false` for a missing or `nil` id) — what decides whether it draws, simulates and ticks |
| `Scene.SetStatic` | `(id, static)` | `true` if an entity existed at `id` — the inspector's **Static** checkbox. Unlike Unity's editor-only `GameObject.isStatic`, rusty's flag is live: from the next tick a static entity is a fixed physics body (a Rigidbody stops moving), its shadow moves into the cached static bake, and probe lighting skips it. The navmesh and the lightmap read it at their next bake, so rebake after changing it (`Lighting.md`) |
| `Scene.IsStatic` | `(id)` | the entity's Static flag (`false` for a missing or `nil` id) |
| `Scene.GetParent` | `(id)` | the parent's `id`, or `nil` (also for a `nil` id) |
| `Scene.GetChildren` | `(id)` | array of child ids, in hierarchy order (empty for a `nil` id) |
| `Scene.FindChild` | `(id, path)` | the descendant at name path `path` (`"Viewport/Content"`), or `nil` (also for a `nil` id) |
| `Scene.GetScript` | `(id, name)` | the entity's live script instance named `name`, or `nil` (also for a `nil` id) — see below |
| `Scene.Save` | `([path])` | the written path |
| `Scene.Load` | `(path)` | `nil`, or in edit mode a warning string when the scene loaded but its lighting sidecar did not (errors, naming `path`, if there is no file there) |
| `Scene.GetActivePath` | `()` | the current scene file, or `nil` |
| `Scene.DontDestroyOnLoad` | `(id)` | `true` if an entity existed at `id` (errors outside play) |
| `Scene.SavePrefab` | `(rootId, path)` | the written path |
| `Scene.Instantiate` | `(path, …)` — see below | new entity `id` |
| `Scene.InstantiateUnpacked` | `(prefabPath, [parentId])` | new entity `id` (an unlinked copy) |
| `Scene.RevertPrefabOverrides` | `(instanceRootId)` | — (drops overrides, restores the source) |
| `Scene.ReimportPrefab` | `(instanceRootId)` | — (re-baseline from source, re-apply overrides) |
| `Scene.ApplyPrefabToSource` | `(instanceRootId)` | count of entities written back into the `.prefab` |
| `Scene.ApplyPrefabFieldToSource` | `(entityId, key)` | — (writes one override to the `.prefab`) |
| `Scene.RevertPrefabField` | `(entityId, key)` | `true` if an override was reverted, `false` if there was none at `key` |
| `Scene.ListPrefabOverrides` | `(instanceRootId)` | array of `"localId:key"` strings (a JSON pointer, `+/component`, `-/component`) |

**Active, self and in the hierarchy (#1363).** As in Unity, `SetActive(pivot, false)`
hides and stops the whole subtree under `pivot`: its children stop drawing (meshes,
skinned meshes, particles, lights, decals, UI), simulating (bodies, colliders, nav
agents, audio) and ticking (scripts, Animators), and each child's scripts get
`OnDisable`. The children's own flags are untouched, so `IsActive(child)` stays
`true` while `IsActiveInHierarchy(child)` reads `false`; reactivating the pivot
brings back exactly the children that were active on their own. An `AudioSource`
stops its voice when its entity goes inactive and, when `play_on_start` is set,
plays again when it comes back (Unity's play-on-enable). Meshes and textures stay
uploaded while hidden, so showing a holstered weapon costs no load.

**Hierarchy reads (#422).** `FindChild` walks one level per `/`-separated
segment, taking the first child with that name in hierarchy order — Unity's
`transform.Find`. A widget script finds its own parts this way (a slider's
`"Handle Slide Area/Handle"`), so nothing stores an id a prefab stamp would
invalidate. `GetScript(id, name)` is Unity's `GetComponent<T>()` for script
components: `name` is the script's file stem (`"button"` for `button.lua`), and the
result is the very table the script's callbacks run on — so an owning script reads
a widget's state and hands it callbacks (`Scene.GetScript(b, "button").on_click =
function(id) … end`). It is `nil` outside play, and for an entity spawned this tick
until its scripts load at the head of the next (see the spawn note in
[`index.md`](index.md)). From a scenario, which runs in a VM of its own, the
result is a handle instead: fields copied by value, and a function field a proxy
that runs the game script's function in the gameplay VM (`GetScript(p,
"fp_controller"):Refill(30)`, #1580; `docs/testing.md`, *What a scenario can
call*). A call's side effects stand even when its return cannot cross back (a
function, or a table holding one): the error says the call itself ran.

**`primitive`** (optional) is one of the GameObject menu's primitives,
case-insensitive: `Box`, `Sphere`, `Plane`, `Cylinder`, `Capsule` (meshes) or
`PointLight`, `DirectionalLight`, `SpotLight`. The sizes are Unity's: the Box is
1 × 1 × 1, the Sphere diameter 1, the Cylinder diameter 1 and height 2 along Y
(−1..1), and the Capsule radius 0.5, full height 2, along Y. The Plane is 15 × 15 on XZ, one quad; for vertices a
vertex shader can move (`vertex_waves`, [`Shader.md`](Shader.md)), cut it into a grid
with `Component.Set(id, "Mesh", { subdivisions = 64 })`: 1–256 quads per side,
`(n + 1)²` vertices, the same size and UVs, saved with the scene (the Inspector's
Mesh card has the same field). A count out of range, or on another shape, is an
error. Omit `primitive` (or pass an unknown name) to create a bare entity carrying
only its mandatory `Transform` — the menu's **Create Empty**.

**`kind`** is one of the Add Component menu's first-class components,
case-insensitive: `Light`, `Animator`, `Collider`, `RigidBody`,
`Texture` (alias `Material`), `NavMeshAgent`, `NavMeshObstacle` (alias
`NavObstacle`), `OffMeshLink` (alias `NavMeshLink`), `NavMeshModifierVolume` (alias
`NavModifier`), `Camera`, `Particles`,
`PostProcessing`, `Audio` (alias `AudioSource`), `AudioReverbZone` (alias
`ReverbZone`), `Canvas`, `RectTransform`,
`Image`, `CanvasGroup`, `RectMask` (alias `RectMask2D`), `Mask`,
`BackdropFilter` (alias `Backdrop`), `Text` (alias
`TextMeshPro`), `Shape`, `Selectable`, `LayoutGroup`, `LayoutElement` (alias
`ContentSizeFitter`), `Joint` (aliases `FixedJoint`, `HingeJoint`,
`CharacterJoint` — all add a default `Fixed` joint; set its kind with
`Joint.SetKind`), `CharacterController` (alias `Character`), `LODGroup` (alias `LOD`), `Trail` (alias `TrailRenderer`),
`Line` (alias `LineRenderer`).
Each is added with the inspector's default values; adding an
existing kind replaces it. (Scripts attach by path, not as a defaulted kind: see
`AddScript`.)

**`AddScript(id, path, [fields])`** is the Add Component menu's script entries:
it appends the script at `path` (project-relative, as the inspector stores it:
`"assets/scripts/ui/dropdown_item.lua"`) to the entity's scripts, never replacing
one, since an entity holds many. `fields` sets the script's `fields` values away
from their defaults, as the inspector does: numbers, booleans and strings only
(anything else is an error and attaches nothing), and a name the script does not
declare is ignored. In Play the script loads at the head of the next tick, like a
spawned entity's, so `GetScript` sees it from then on. The engine's Dropdown
attaches its list items' click script this way.

**Component dependencies (`RequireComponent`).** A first-class component may
*declare that it requires another* — rusty's analog of Unity's
`[RequireComponent(typeof(T))]`. The rule is one declaration enforced identically on
every surface (editor Add menu, this API, and scene load):

- **Adding** a dependent auto-adds its requirement with defaults if missing, so
  `AddComponent` never fails on a missing dependency — `Scene.AddComponent(id,
  "PostProcessing")` on a camera-less entity also attaches a `Camera`.
- **Removing** a required component cascades to the dependents that require it —
  `Scene.RemoveComponent(id, "Camera")` also removes the entity's `PostProcessing`.
- **Loading** a scene validates the same rule: a hand-edited document with a
  dependent but no requirement gets the requirement auto-added (with a console
  warning), so the loaded scene always satisfies the rule.

The declared dependencies are **`PostProcessing` requires `Camera`** (a
color/bloom/SSR post-processing stack is inert without a camera to process) and
**`Image`, `Text`, `Shape`, `RectMask`, `Mask`, `BackdropFilter`, `Selectable`,
`LayoutGroup` and `LayoutElement` require `RectTransform`** (a graphic fills, a mask clips to, and a layout arranges
a rect — Unity's `Graphic`, `RectMask2D`, `Mask` and layout components declare the same),
and **`Joint` requires `RigidBody`** (a joint constrains a body — Unity's `Joint` too).

**`Scene.Deactivate`** is Unity's deferred `Object.Destroy`: it sets `active =
false` but leaves the entity in the scene. **`Scene.DestroyEntity`** is the
editor's Destroy button: it actually removes the entity **and its whole subtree**,
as Unity's `Object.Destroy` does (#1283): its children and theirs, so a skinned
entity's bones and what hangs from them, and a bone's descendant bones (see
[`Animator`](Animator.md), *Bones are GameObjects*). In Play every script on the
subtree gets `OnDisable` (if active) then `OnDestroy`, each pass children before
parents. Unity documents no order here: the one public test (Unity 2017.4) saw
`OnDisable` children first but `OnDestroy` parents first, so rusty's `OnDestroy`
order is its own choice, kept the same as `OnDisable`'s (#1298). An engine destroy
in Play (a stale hitbox `Physics.GenerateHitboxes` drops) is queued the same way.
To keep a child, unparent it first: `Scene.SetParent(child, nil)`.

**Nothing keeps naming a destroyed entity** (#1264). Every reference to it is dropped, live and in the next
save: the selection, `DontDestroyOnLoad`, its lightmap entry, and each component
field that names it, which falls back to that field's `nil`. A `Joint` to it then
holds to the world (as Unity's does), a `Selectable`'s target graphic becomes the
Selectable itself, a sub-emitter slot or an LOD renderer is gone, a
[marker](RectTransform.md) that followed it loses its target and stays hidden (its
scripts keep running, so it can be given a target again), and a linked prefab
instance's override that named it becomes `nil` (an override in a list, such as an
LOD renderer, drops that element; the instance's other overrides are untouched). A
`Scene.Load` treats what it leaves behind the same way: an entity carried across by
`DontDestroyOnLoad` keeps no reference to an entity that did not survive, its
skeleton included: a bone left behind (one a script moved under an entity that was
not carried) is a missing slot, never an id the new scene's entities could be
given. A
reference to a bone is kept by the bone's name, so it survives a save, a load and a
re-import of the model.

**`Scene.Save([path])`** persists the live world. With no path it writes back to
the current scene file (the file the editor/session loaded); an explicit path
writes there and becomes the new current file (Save As). It errors if no path is
given and no current scene file is set, and during Play: the live world then holds
the play state, never the authored scene, so a save is refused and the file is left
as it was (#1340, Unity's rule).

### Loading scenes (#432)

**`Scene.Load(path)`** is Unity's `SceneManager.LoadScene`: it replaces the one
active scene with the scene file at `path`. There is always exactly one active
scene — additive loading is not supported.

- **During play it is deferred to the tick's tail.** The call only records the
  request (a second call in the same tick replaces the first); the swap happens
  in the scene-load phase after the destroy phase, so it never happens in the
  middle of a callback and a replay swaps on the same tick every time. Code after
  the call still sees the old scene, and `Scene.GetActivePath()` still names the
  old file until the swap.
- **Unload lifecycle.** Every script of the outgoing scene gets `OnDisable` (if
  its entity was active) then `OnDestroy`, exactly as `Scene.DestroyEntity`
  would, in the same order (children before parents). Its timers and coroutines stop, its sounds stop, and UI focus, hover
  and press state on it is dropped. Physics and the navmesh are rebuilt from the
  new scene. The new scene's scripts `Awake` / `OnEnable` / `Start` at the head
  of the next tick, before any `Update` — like spawned entities.
- **`Scene.DontDestroyOnLoad(id)`** keeps the entity **and its children** alive
  across loads — the music player, the game manager, the persistent HUD canvas.
  Survivors keep their **ids**, components and running scripts, timers and
  coroutines; their scripts do not re-run `Awake`. A new-scene entity whose id a
  survivor already holds gets a fresh id instead. A survivor whose parent does
  not survive becomes a root, and the materials it uses come along unless the
  new scene defines one of the same name. Physics state (velocities) restarts
  from the survivor's transform, since the physics world is rebuilt. The mark
  lasts until the entity is destroyed or play stops; it is an error in edit mode.
- **Decals** go away with the scene, except those spawned with `persist = true`
  (`Decals.md`, #1518): those stay on a surviving owner, are re-attached by
  hierarchy path to the incoming entity where their owner was left behind, or
  stay put in world space, keeping their id and age.
- **Edit mode:** `Scene.Load` is the API face of the editor's File ▸ Open — the
  scene loads immediately (no callbacks: no scripts run in edit mode) and becomes
  the current scene file that `Scene.Save()` writes back to. If the scene document
  loads but its lighting sidecar does not, the scene is still loaded and still the
  current file (probes keep zero SH); `Load` returns the warning as a string.
- **Editor Play:** a `Scene.Load` swaps the running world, and **Stop restores
  the scene Play was pressed in** (and its scene file), not the one play ended in
  — Stop rewinds to the edit snapshot, as always. The standalone player has no
  Stop; it simply keeps running the loaded scene.
- **Errors:** a path with no file errors at the call, naming the path. A file that
  exists but fails to parse at the swap is reported to the console, and the
  current scene keeps running.
- **Loading screens:** the load is synchronous at the tick's tail — there is no
  async/streamed loading. For a one-frame "Loading…" screen, activate a loading
  canvas, then call `Scene.Load` on the next tick.

### Prefabs

A **prefab** is a configured GameObject — a root entity plus its whole child
subtree, every component configured and every asset reference intact — saved to its
own `.prefab` asset so it can be stamped into any scene. It is the configure-once,
stamp-many template (Unity's prefab) and the runtime spawn primitive a wave-spawner
script calls.

Two flavours of instance, matching Unity:

- **Linked** (the default) — the instance keeps a live link back to the `.prefab`. On
  every scene load and on an explicit re-import, the instance is re-baselined from the
  current source so a later edit to the prefab propagates into all its instances. Per-
  instance edits are **overrides** the moment they are made, and survive that propagation.
- **Unpacked** (the v1 copy) — an independent snapshot with no link back to the asset;
  editing the source never touches it.

**`Scene.SavePrefab(rootId, path)`** extracts the subtree rooted at `rootId` and
writes it to `path` (a `.prefab` JSON document with local 0-based ids, the referenced
material slice, and no GPU buffers). Returns the written path; errors if no entity
has that id. This is the same verb the hierarchy's right-click **Save as Prefab**
runs.

**`Scene.Instantiate(path, …)`** is the single spawn verb; it dispatches on whether
`path` is a `.prefab` or an importable asset reference, so prefabs and assets share
one name and never drift.

- **`Scene.Instantiate(prefabPath, [parentId])`** — loads a `.prefab` and stamps a
  **linked** instance (the default): fresh deterministic ids, the prefab's materials
  merged into the scene library (an identical existing material is reused; a name-
  conflicting one is inserted under a uniquified name and the instance's references are
  rewritten to it), the new root parented under `parentId` (or the scene root when
  omitted), and a live link to `prefabPath` stamped on **every** entity of the instance.
  Returns the new root's id.

- **`Scene.InstantiateUnpacked(prefabPath, [parentId])`** — the same stamp **without**
  the link: the v1 independent copy. Use it when you want a one-off that must not track
  later source edits.

- **`Scene.Instantiate(assetRef, [name], [x, y, z])`** — spawns one entity from an
  importable model sub-object reference (`path::sub_object`, the `reference` field
  `Assets.Manifest()` hands back), placed at `(x, y, z)` (the origin when omitted) and
  named `name` (the sub-object id when omitted). This is the API equivalent of dragging
  a glTF into the scene from Unity's Project window: it runs the importer to build the
  mesh, applies the sub-object's glTF material (deduped into the shared library, or the
  engine default when the glTF names none), and syncs a mesh collider when the asset's
  `.meta` sidecar records one. Returns the new entity's id; errors (without spawning)
  when the reference is malformed, the file fails to import, or the sub-object is absent.
  A reference splits on its **first** `::`: the sub-object name after it may itself
  hold `::` (`hero.glb::Body::Mesh`, taken verbatim from the model), so the path before
  it must end in a model extension and must not hold `::`. That is rusty's rule, not
  the OS's (macOS and Linux allow `:` in names); a project folder whose own name holds
  `::` is fine, because references are project-relative. A model under a folder such
  as `assets/my::kit/` is refused with an error naming the path and the rule, never
  spawned with a mesh that loads empty.
  A converted source (`.fbx`, `.blend`) is never converted during Play: as Unity refreshes its
  assets before Play and its runtime loads never import, entering Play brings every
  converted source under `assets/` up to date (and every instance in the scene with
  it), and Play then reads that import. A source changed during Play keeps its last
  import until Stop converts it. A source whose conversion failed plays from its
  earlier import, with a warning, when it has one; with none, or when it appeared
  during Play, the load fails with "import it in edit mode first", and a mistyped
  path with "file not found". The standalone player refreshes and holds the same way.
  A sub-object name that is the **base of a `_LOD<n>` set** (`props.glb::Crate` for
  objects named `Crate_LOD0`, `Crate_LOD1`) spawns the whole set as one entity carrying
  an `LODGroup`, one child per level, and returns the group's id — see
  [`LODGroup`](LODGroup.md).

In every case the geometry is stored only as a reference and rehydrated on the next
scene load, exactly like a primitive's `primitive_type` — no GPU buffers are inlined.

#### Linked-instance overrides (revert / reimport / apply-to-source)

A linked instance's divergence from the source is its **diff** against the source as
the instance last took it (when it was stamped, loaded or reimported), held the way
Unity holds a prefab instance's overrides, in three kinds. It covers every present and
future component with no per-component code. Each override has a **key**, the name
`ListPrefabOverrides` lists it by and the field verbs take:

- **Property modifications**, one per changed leaf, keyed by its JSON pointer
  (`"/transform/position/0"`). A list's elements are addressed by index, and a list
  whose length changed has one **size entry**, `"<list>/#size"` (Unity's `Array.size`):
  removing a script is a size entry (plus the leaves of the scripts that moved up a
  slot), adding one is a size entry plus one entry holding the new script whole. The
  elements both lists hold keep following the source, so a source edit to a script
  the instance didn't touch still reaches it. A list whose length changed is **one
  override to Apply or Revert**, as Unity applies a whole array: `ApplyPrefabFieldToSource`
  or `RevertPrefabField` on its size entry or on any entry under that list takes the
  size and every entry of the list together. A size over 65 536 is skipped with a
  warning. A field the leaf diff can't split (an
  enum switched variant, such as a collider's shape) is one entry holding it whole.
- **Added components** (Unity's *added components*), keyed `"+/<component>"`
  (`"+/light"`): a component the instance has and the source lacks, held whole, so a
  Light or a Collider added to an instance child keeps all its values.
- **Removed components** (Unity's *removed components*), keyed `"-/<component>"`
  (`"-/camera"`): a source component the instance removed.

An entity id inside any of them (a Joint's `connected_body`, inside an added Joint
too) follows its entity: it is remapped when the bones respawn on Stop or a load, and
written to the source as the source's own id by the apply-to-source verbs.

As in Unity, **an edit to an instance is an override the moment it is made**, whatever
made it: the gizmo, the Inspector, a Lua setter such as `Transform.SetPosition`, the
MCP. There is nothing to record. The overrides save and load with the scene, survive
Play (leaving Play restores the edit-mode world exactly), and are re-applied when the
source changes, so a source edit still reaches every field the instance did not edit.
The instance **root's position and rotation are always the instance's own**: they never
come from the source and are never overrides. A source applied to during Play reaches
the edit-mode instances on Stop, each keeping its own edits.

The overrides are exactly the instance's current diff: an edit the author undoes (a
component removed and re-added, a collider shape switched back, an added component
removed) leaves no override behind. So an override on a component or field the source
has **removed** is dropped; Unity keeps it as an "unused override", rusty doesn't.
While an instance's source file is **missing** (it was loaded without one),
the instance keeps the overrides it was saved with, but an edit made meanwhile is not
tracked as an override until the source is back: the edit stays in the scene, and the
source, once it returns, overwrites it on the next load or reimport.

Edits flow in two directions. **On-instance** verbs keep edits local to this instance;
the **apply-to-source** verbs push them up into the shared `.prefab` so *other* instances
get them.

On-instance:

- **`Scene.RevertPrefabOverrides(instanceRootId)`** — drop every override and rebuild
  the instance straight from the current source (it becomes a pristine copy).
- **`Scene.ReimportPrefab(instanceRootId)`** — re-baseline from the source and re-apply
  the overrides on top: non-overridden fields pick up the latest source,
  overridden ones win. This is the same propagation that runs automatically on scene
  load.
- **`Scene.ListPrefabOverrides(instanceRootId)`** — the read verb: the instance's
  overrides as `"localId:key"` strings, sorted (e.g. `"1:+/light"`, `"1:-/camera"`,
  `"1:/scripts/#size"`, `"1:/transform/position/0"`).

Apply-to-source (the Unity "Apply → Prefab" direction):

- **`Scene.ApplyPrefabToSource(instanceRootId)`** — write *every* override back
  into the source `.prefab` document on disk, then clear them on the instance (it now
  matches the source). Other instances of that prefab pick the change up on their next
  scene load, `ReimportPrefab` or leaving Play. Returns the number of entities written.
- **`Scene.ApplyPrefabFieldToSource(entityId, key)`** — write *one* override back
  into the source (a leaf, a list's size, an added component, or a removal), then drop
  just that override and reimport so it re-tracks the source (the instance's other
  overrides are untouched). `key` is one `ListPrefabOverrides` lists, without the
  `localId:` prefix. A key in a list whose length changed applies that whole list
  (its size entry and every entry under it): applying after removing the first of
  two scripts leaves the source with only the second. `entityId` may be any instance
  entity — root or child; the verb
  resolves the instance root itself. Errors if the entity carries no override at
  `key`.

Its per-field opposite:

- **`Scene.RevertPrefabField(entityId, key)`** — Unity's *Revert* on one override:
  drop just that override and reimport, so what it covered tracks the source again (a
  reverted `"+/light"` takes the added light away, a reverted `"-/camera"` brings the
  source's camera back, a reverted size entry or element of a list whose length
  changed gives the whole list back to the source).
  The instance's other overrides are untouched. `entityId` may be any instance
  entity. Returns `false`, changing nothing, when there is no override at `key`. The
  prefab card's per-field revert button.

The verbs error if `instanceRootId` (or, for the field form, `entityId`) is not a
**linked** instance entity. **Scope (#216):** a flat instance of a flat prefab —
added/removed/reparented child entities and nested prefabs are out of scope (propagation
matches entities by the link's source `local_id`, adding/removing nothing structurally).
Saving an already-linked instance back out with `SavePrefab` bakes it down into a fresh
flat prefab (no nested links).
