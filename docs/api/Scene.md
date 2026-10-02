## `Scene`

The structural-authoring surface: the API equivalent of the editor's GameObject
menu (create), the Hierarchy's Destroy and the inspector's parenting + Add
Component menu. Every verb routes through the shared `scene::authoring` module, so
it behaves identically to the editor and uses the same default values.

| Function | Signature | Returns |
|---|---|---|
| `Scene.FindEntityByName` | `(name)` | `id` (or `0` if none) |
| `Scene.CreateEntity` | `(name, [primitive])` | new entity `id` |
| `Scene.Deactivate` | `(id)` | — |
| `Scene.DestroyEntity` | `(id)` | `true` if an entity existed at `id` |
| `Scene.AddComponent` | `(id, kind)` | `true` if the entity exists |
| `Scene.RemoveComponent` | `(id, kind)` | `true` if the entity exists |
| `Scene.SetParent` | `(id, parent_id)` | — (errors on a parenting cycle) |
| `Scene.ClearParent` | `(id)` | — |
| `Scene.SetActive` | `(id, active)` | `true` if an entity existed at `id` — Unity's `SetActive`; its callbacks follow the edge (`OnEnable` / `OnDisable`) |
| `Scene.IsActive` | `(id)` | the entity's own `active` flag (`false` for a missing id; an inactive ancestor still hides it) |
| `Scene.GetParent` | `(id)` | the parent's `id`, or `nil` |
| `Scene.GetChildren` | `(id)` | array of child ids, in hierarchy order |
| `Scene.FindChild` | `(id, path)` | the descendant at name path `path` (`"Viewport/Content"`), or `nil` |
| `Scene.GetScript` | `(id, name)` | the entity's live script instance named `name`, or `nil` — see below |
| `Scene.Save` | `([path])` | the written path |
| `Scene.Load` | `(path)` | — (errors, naming `path`, if there is no file there) |
| `Scene.GetActivePath` | `()` | the current scene file, or `nil` |
| `Scene.DontDestroyOnLoad` | `(id)` | `true` if an entity existed at `id` (errors outside play) |
| `Scene.SavePrefab` | `(rootId, path)` | the written path |
| `Scene.Instantiate` | `(path, …)` — see below | new entity `id` |
| `Scene.InstantiateUnpacked` | `(prefabPath, [parentId])` | new entity `id` (an unlinked copy) |
| `Scene.RecordPrefabOverrides` | `(instanceRootId)` | count of entities whose overrides were recorded |
| `Scene.RevertPrefabOverrides` | `(instanceRootId)` | — (drops overrides, restores the source) |
| `Scene.ReimportPrefab` | `(instanceRootId)` | — (re-baseline from source, re-apply overrides) |
| `Scene.ApplyPrefabToSource` | `(instanceRootId)` | count of entities written back into the `.prefab` |
| `Scene.ApplyPrefabFieldToSource` | `(entityId, jsonPointer)` | — (writes one leaf to the `.prefab`) |
| `Scene.ListPrefabOverrides` | `(instanceRootId)` | array of `"localId:json-pointer"` strings |

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
[`index.md`](index.md)).

**`primitive`** (optional) is one of the GameObject menu's primitives,
case-insensitive: `Box`, `Sphere`, `Plane`, `Cylinder` (meshes) or `PointLight`,
`DirectionalLight`, `SpotLight`. Omit it (or pass an unknown name) to create a bare
entity carrying only its mandatory `Transform` — the menu's **Create Empty**.

**`kind`** is one of the Add Component menu's first-class components,
case-insensitive: `Light`, `Animator`, `Collider`, `RigidBody`,
`Texture` (alias `Material`), `NavMeshAgent`, `NavMeshObstacle` (alias
`NavObstacle`), `OffMeshLink` (alias `NavMeshLink`), `Camera`, `Particles`,
`VisualCorrection`, `Audio` (alias `AudioSource`), `AudioReverbZone` (alias
`ReverbZone`), `Canvas`, `RectTransform`,
`Image`, `CanvasGroup`, `RectMask` (alias `RectMask2D`), `Mask`,
`BackdropFilter` (alias `Backdrop`), `Text` (alias
`TextMeshPro`), `Shape`, `Selectable`, `LayoutGroup`, `LayoutElement` (alias
`ContentSizeFitter`), `Joint` (aliases `FixedJoint`, `HingeJoint`,
`CharacterJoint` — all add a default `Fixed` joint; set its kind with
`Joint.SetKind`), `CharacterController` (alias `Character`), `LODGroup` (alias `LOD`), `Trail` (alias `TrailRenderer`),
`Line` (alias `LineRenderer`).
Each is added with the inspector's default values; adding an
existing kind replaces it. (Scripts attach by path, not as a defaulted kind — a
separate concern.)

**Component dependencies (`RequireComponent`).** A first-class component may
*declare that it requires another* — rusty's analog of Unity's
`[RequireComponent(typeof(T))]`. The rule is one declaration enforced identically on
every surface (editor Add menu, this API, and scene load):

- **Adding** a dependent auto-adds its requirement with defaults if missing, so
  `AddComponent` never fails on a missing dependency — `Scene.AddComponent(id,
  "VisualCorrection")` on a camera-less entity also attaches a `Camera`.
- **Removing** a required component cascades to the dependents that require it —
  `Scene.RemoveComponent(id, "Camera")` also removes the entity's `VisualCorrection`.
- **Loading** a scene validates the same rule: a hand-edited document with a
  dependent but no requirement gets the requirement auto-added (with a console
  warning), so old scenes keep loading.

The declared dependencies are **`VisualCorrection` requires `Camera`** (a
color/bloom/SSR correction stack is inert without a camera to correct) and
**`Image`, `Text`, `Shape`, `RectMask`, `Mask`, `BackdropFilter`, `Selectable`,
`LayoutGroup` and `LayoutElement` require `RectTransform`** (a graphic fills, a mask clips to, and a layout arranges
a rect — Unity's `Graphic`, `RectMask2D`, `Mask` and layout components declare the same),
and **`Joint` requires `RigidBody`** (a joint constrains a body — Unity's `Joint` too).

**`Scene.Deactivate`** is Unity's deferred `Object.Destroy`: it sets `active =
false` but leaves the entity in the scene. **`Scene.DestroyEntity`** is the
editor's Destroy button: it actually removes the entity.

**`Scene.Save([path])`** persists the live world. With no path it writes back to
the current scene file (the file the editor/session loaded); an explicit path
writes there and becomes the new current file (Save As). It errors if no path is
given and no current scene file is set.

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
  would. Its timers and coroutines stop, its sounds stop, and UI focus, hover
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
- **Edit mode:** `Scene.Load` is the API face of the editor's File ▸ Open — the
  scene loads immediately (no callbacks: no scripts run in edit mode) and becomes
  the current scene file that `Scene.Save()` writes back to.
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
  instance edits are recorded as **overrides** that survive that propagation.
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
  a glTF from the content browser into the scene: it runs the importer to build the
  mesh, applies the sub-object's glTF material (deduped into the shared library, or the
  engine default when the glTF names none), and syncs a mesh collider when the asset's
  `.meta` sidecar records one. Returns the new entity's id; errors (without spawning)
  when the reference is malformed, the file fails to import, or the sub-object is absent.
  A sub-object name that is the **base of a `_LOD<n>` set** (`props.glb::Crate` for
  objects named `Crate_LOD0`, `Crate_LOD1`) spawns the whole set as one entity carrying
  an `LODGroup`, one child per level, and returns the group's id — see
  [`LODGroup`](LODGroup.md).

In every case the geometry is stored only as a reference and rehydrated on the next
scene load, exactly like a primitive's `primitive_type` — no GPU buffers are inlined.

#### Linked-instance overrides (record / revert / reimport / apply-to-source)

A linked instance records its divergence from the source as a **generic field diff** —
a map of `json-pointer-path → value` for each field where the instance differs from a
fresh copy of the source. This auto-covers every present and future component (and
component add/remove — a `None`↔`Some` is just a diff at that path) with no per-
component code. The override set is carried on each instance entity's link, so it
saves and loads with the scene.

Edits flow in two directions. **On-instance** verbs keep edits local to this instance;
the **apply-to-source** verbs push them up into the shared `.prefab` so *other* instances
get them. (Naming note (#268): the on-instance recorder is `RecordPrefabOverrides` —
formerly `ApplyPrefabChanges`, renamed so "apply" unambiguously means apply-to-source.)

On-instance:

- **`Scene.RecordPrefabOverrides(instanceRootId)`** — record the instance's current edits
  as overrides (re-diffs every instance entity against a fresh source baseline and
  stores the result). Call it after editing an instance, while the source is unchanged,
  so the edits are preserved through later propagation. Returns the number of entities
  recorded.
- **`Scene.RevertPrefabOverrides(instanceRootId)`** — drop every override and rebuild
  the instance straight from the current source (it becomes a pristine copy).
- **`Scene.ReimportPrefab(instanceRootId)`** — re-baseline from the source and re-apply
  the recorded overrides on top: non-overridden fields pick up the latest source,
  overridden ones win. This is the same propagation that runs automatically on scene
  load.
- **`Scene.ListPrefabOverrides(instanceRootId)`** — the read verb: the instance's
  recorded override paths as `"localId:json-pointer"` strings (e.g.
  `"0:/transform/position/0"`).

Apply-to-source (the Unity "Apply → Prefab" direction):

- **`Scene.ApplyPrefabToSource(instanceRootId)`** — write *every* recorded override back
  into the source `.prefab` document on disk, then clear them on the instance (it now
  matches the source). Other instances of that prefab pick the change up on their next
  scene load or `ReimportPrefab`. Returns the number of entities written.
- **`Scene.ApplyPrefabFieldToSource(entityId, jsonPointer)`** — write *one* overridden
  leaf back into the source, then drop just that override and reimport so the leaf
  re-tracks the source (the instance's other overrides are untouched). `entityId` may be
  any instance entity — root or child; the verb resolves the instance root itself. Errors
  if the entity carries no override at `jsonPointer`.

The verbs error if `instanceRootId` (or, for the field form, `entityId`) is not a
**linked** instance entity. **Scope (#216):** a flat instance of a flat prefab —
added/removed/reparented child entities and nested prefabs are out of scope (propagation
matches entities by the link's source `local_id`, adding/removing nothing structurally).
Saving an already-linked instance back out with `SavePrefab` bakes it down into a fresh
flat prefab (no nested links).
