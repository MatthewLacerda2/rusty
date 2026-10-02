# Scripting API reference

The stable surface available to Lua gameplay scripts, the console REPL, and
bot-players — the rusty equivalent of Unity's Scripting Reference. One surface,
three callers: anything documented here works identically in a `.lua` entity
script, a console line, and a headless harness scenario.

Entities are addressed by a numeric `id` (`u32`). `Vector3` values cross the
boundary as three `f32`s (`x, y, z`) rather than a table. Getters that miss
return a sensible default (zeros, or `(1,1,1)` for scale) instead of erroring.

> **Maintenance:** this file is currently **hand-written** and must be kept in
> sync with `src/scripting/mod.rs` and `src/scripting/bindings/`. A generator
> that emits it from self-describing bindings (with a CI drift check) is a tracked
> follow-up — see the PR for issue #28.

> **Faithfulness:** every setter here is expected to be *observed* by a downstream
> system — a renderer/sim read or a `SceneData` round-trip. When you add a setter,
> name its read-site (or add a round-trip test) in the same change and record it in
> [`api-faithfulness.md`](../api-faithfulness.md), so the surface never grows a
> write-only no-op that silently fails headless authoring (issue #178).

---

## Driving a headless session

For agentic use there is a **fourth caller**: a long-lived, headless, **edit-mode**
engine process you talk to over a command channel. It holds a live world and the
same evaluator the console uses, so every namespace below resolves through it
identically — there is no separate, thinner headless surface.

```
cargo run --bin session --features dev               # boot + seed the default scene
cargo run --bin session --features dev -- <scene>    # boot a specific scene file
cargo run --bin session --features dev -- --empty    # start from an empty scene
```

**Protocol.** One Lua command per line on **stdin**; one JSON response per command
on **stdout**, in lockstep:

- success: `{"ok":true,"result":"<rendered value, or empty for a statement>"}`
- failure: `{"ok":false,"error":"<message>"}`

A line is evaluated as an expression first (so it echoes its value), then as a
statement. **State persists for the life of the process**: globals set on one line
are visible on the next, and any world mutation (a loaded scene, baked nav, an
imported mesh, a moved transform) stays put across commands. The session runs in
**edit mode** — it does *not* force play — and a failed command is reported in its
response **without** tearing the session down. The channel ends at EOF.

```
> pid = Scene.FindEntityByName("Player")   ->  {"ok":true,"result":""}
> Transform.SetPosition(pid, 7, 8, 9)      ->  {"ok":true,"result":""}
> Transform.GetPosition(pid)               ->  {"ok":true,"result":"7, 8, 9"}
```

Note: `local` bindings are scoped to their own line; use a **global** (no `local`)
to carry a value across commands, as with `pid` above.

### Attaching to a windowed playtest

The same command channel is also exposed by the **windowed** engine (`cargo run
--features dev`), so an agent can attach to a *running, rendered* playtest and drive
it live — not just the headless `session` binary. The framing and protocol are
**identical** to the headless session above; only the transport differs:

- **Linux / macOS:** a unix domain socket. Path is `$RUSTY_CMD_SOCK` if set, else
  `$XDG_RUNTIME_DIR/rusty.sock`, else `/tmp/rusty.sock`.
- **Windows:** a TCP listener on `127.0.0.1`. Address is `$RUSTY_CMD_ADDR` if set,
  else `127.0.0.1:8787`.

The chosen address/path is logged to the engine console on boot (`[cmd] command
channel listening on …`). Connect, then send one Lua command per line and read one
JSON response per command, exactly as with the headless session. Commands are
evaluated on the engine's main loop (once per frame), so they never race the sim. A
bind failure is non-fatal — the window simply runs without the channel. The channel
is **dev-only** and absent from ship builds.

---

## Script lifecycle callbacks

The MonoBehaviour half of the surface: the functions a `.lua` entity script
*implements* (as keys on the table it returns) and the engine calls. These are
the **only** callbacks dispatched — a function under any other name (a
`FixedUpdate`, an `OnDestroy`, …) is simply never called, with no error.
Every callback is optional; a script defines any subset, and exposing at least
one of them is what marks a script as a MonoBehaviour the inspector's Add
Component menu offers.

| Callback | Signature | When it runs |
|---|---|---|
| `Awake` | `Awake(id)` | Once per script instance, when it first participates in the sim while its entity is `active`: at play-enter for active scene entities, at the head of the next tick's script phase for entities spawned during play (see the divergence note below) or loaded by `Scene.Load`, or on the entity's first active tick when it is loaded/spawned disabled. Always the instance's first callback. |
| `Start` | `Start(id)` | Once per script instance, after its `Awake`, immediately before its first `Update` — deferred while the owning entity is inactive, so an object instantiated disabled initializes when first enabled (the spawn-pool pattern). |
| `Update` | `Update(id, dt)` | Every frame of play while the owning entity is `active`, after the instance's `Start` has run. `dt` is the frame's scaled delta (`Time.deltaTime`); under the headless harness / `Time.Step` it is the fixed step. |
| `LateUpdate` | `LateUpdate(id, dt)` | Every frame of play while the owning entity is `active`, after **every** script's `Update` and after this tick's physics, animation and particle steps have resolved — the post-physics hook. Same scaled `dt` as `Update` (`Time.deltaTime`) and the same deterministic `(entity, script index)` order, still inside the one `FixedUpdate` sim tick — it is **not** a render-rate callback. Reach for it when a script must read *this* tick's settled transforms: follow-cameras, look-ats, aim, recoil recovery, and other post-move corrections that visibly lag by a step if run in `Update`. |
| `OnTriggerEnter` | `OnTriggerEnter(id, other)` | Once, on the first frame an overlap involving a trigger collider (`is_trigger`) exists — before that frame's `OnTrigger`. A solid collider, static or not, never fires the trigger hooks: its contacts are `OnCollision*`. |
| `OnTrigger` | `OnTrigger(id, other)` | After the physics step, once per overlapping pair involving a trigger collider (`is_trigger`). It is the overlap "stay": it repeats every frame the overlap persists, including the frame `OnTriggerEnter` fires. |
| `OnTriggerExit` | `OnTriggerExit(id, other)` | Once, on the first frame a previously overlapping pair no longer overlaps — after that frame's `OnTrigger` dispatches (no stay fires for the ended pair). |
| `OnCollisionEnter` | `OnCollisionEnter(id, other, contact)` | Once, when a solid (non-trigger) contact between the entity's collider and another begins — the tick the solver first pushes the two apart, so `contact.impulse` is the impact. At least one side must be a dynamic body: a kinematic body against static geometry never collides (Unity's rule). Before that tick's `OnCollisionStay`. |
| `OnCollisionStay` | `OnCollisionStay(id, other, contact)` | After the physics step, once per touching solid pair, every frame the contact persists — including the frame `OnCollisionEnter` fires (the same edge rule as `OnTrigger`). |
| `OnCollisionExit` | `OnCollisionExit(id, other)` | Once, on the first frame the pair's surfaces no longer touch (or either collider is deactivated) — after that frame's `OnCollisionStay`. No `contact`: nothing is touching any more. |
| `OnJointBreak` | `OnJointBreak(id, force, torque)` | Once, on the tick the entity's `Joint` carried more than its non-zero `break_force` (newtons) or `break_torque` (newton-metres) — after that tick's collision callbacks. `force` / `torque` are what it carried. The `Joint` component is already destroyed (Unity), so the bodies are free. |
| `OnEnable` | `OnEnable(id)` | When the owning entity becomes `active`: on its **first** activation — between `Awake` and `Start`, so the first-tick order is `Awake → OnEnable → Start` — and again on every later inactive→active transition (e.g. a script's `Scene.SetActive(id, true)` re-enabling a pooled object). Detected by diffing the entity's `active` flag against the previous tick, so it fires **exactly once** per rising edge. |
| `OnDisable` | `OnDisable(id)` | When the owning entity becomes inactive: once on each `active`→inactive transition (e.g. `Scene.Deactivate`), and once more — immediately **before** `OnDestroy` — when an *active* entity is destroyed. Fires **exactly once** per falling edge. While disabled, the entity receives no other gameplay callback (no `Update`/`LateUpdate`/`OnTrigger`/`OnCollision*`). |
| `OnDestroy` | `OnDestroy(id)` | Once, when the entity is removed **during play** — by `Scene.DestroyEntity`, or because `Scene.Load` unloaded its scene (unless it was marked `Scene.DontDestroyOnLoad`) — after its `OnDisable` if it was active. The entity is still readable during the callback (removal happens just after). See the divergence note: **Stop does not fire `OnDestroy`.** |
| `OnPointerEnter` | `OnPointerEnter(id, event)` | UI (#420): the pointer moved onto the entity or one of its descendants. Fires on every entity from the hit one up to the root that defines it, deepest first — moving from a button onto its own label does not re-enter the button. |
| `OnPointerExit` | `OnPointerExit(id, event)` | UI: the pointer left the entity and all its descendants (deepest first). |
| `OnPointerDown` | `OnPointerDown(id, event)` | UI: a mouse button went down over the entity. Bubbles — see below. |
| `OnPointerUp` | `OnPointerUp(id, event)` | UI: the button pressed over the entity was released, wherever the pointer is now (or a drag took the press over). |
| `OnPointerClick` | `OnPointerClick(id, event)` | UI: the press and the release were both over the entity. The button menus and bots use. |
| `OnBeginDrag` | `OnBeginDrag(id, event)` | UI: a pointer held down on the entity moved 10 pixels (Unity's drag threshold). |
| `OnDrag` | `OnDrag(id, event)` | UI: every tick the dragged pointer moves; `event.delta` is the motion. |
| `OnEndDrag` | `OnEndDrag(id, event)` | UI: the dragging button was released. |
| `OnScroll` | `OnScroll(id, event)` | UI: the wheel turned over the entity; `event.delta.y` is the lines scrolled (positive = away from the user). |
| `OnSelect` | `OnSelect(id)` | UI: the entity became the focused (selected) one — by a click, keyboard navigation or `UI.SetSelected`. |
| `OnDeselect` | `OnDeselect(id)` | UI: the entity stopped being focused (fires before the new focus's `OnSelect`). |
| `OnSubmit` | `OnSubmit(id)` | UI: Enter was pressed while the entity is focused. |
| `OnCancel` | `OnCancel(id)` | UI: Escape was pressed while the entity is focused. |
| `OnMove` | `OnMove(id, event)` | UI (#422): an arrow key was pressed while the entity is focused. `event` is `{ direction, x, y }` — `"Up"`, `"Down"`, `"Left"` or `"Right"` and its unit vector (y-up). Defining it **takes the arrows off navigation**: focus stays put, and the script moves it itself with `UI.FindSelectable` when it wants (a slider steps its value on Left / Right and navigates on Up / Down). |

`id` is always the **owning** entity's id (the entity the script is attached
to); `other` is the other entity in the overlap or contact. Both name the entity
that **owns the collider** — the part of a compound body, never its root (see
[compound colliders](Physics.md)); the collision callbacks go to scripts on that
part.

**Collision callbacks (#448).** `contact` is the pair's strongest contact point
as the receiver sees it (every point per pair is out of scope):

- `point` — `{x, y, z}` world-space contact point.
- `normal` — `{x, y, z}` unit normal pointing out of `other`, into the receiver — a ball landing on a floor reads `(0, 1, 0)`, the floor reads `(0, -1, 0)`.
- `relativeVelocity` — `{x, y, z}` `other`'s velocity minus the receiver's at the point, taken before this tick's solve — the closing speed of an impact (a ball falling at 5 m/s onto a floor reads `y = 5`).
- `impulse` — Total normal impulse (N·s) the solver applied across the pair this tick; divide by `Time.deltaTime` for force.
- `otherBody` — The entity owning `other`'s rigid body (its compound root; `other` itself when it is its own body).

A contact *touches* once the surfaces are within 5 mm; rapier's speculative
contacts further apart are not reported.

**UI callbacks (#420).** The pointer family's `event` is a table:
`button` (`"Left"`, `"Right"` or `"Middle"`), `position = {x, y}` (UI screen
pixels, bottom-left origin — the frame of `UI.GetRect(id).screen`; `(-1, -1)`
while the cursor is locked), `delta = {x, y}` (pointer motion since last tick,
or the wheel for `OnScroll`), `target` (the entity the pointer actually hit —
the callback may be running on an ancestor of it), and `canvas_position = {x, y}` /
`canvas_delta = {x, y}` — the pointer and its motion on the **receiving** entity's
canvas, in its reference units (the frame of `UI.GetRect(id)`'s `x, y`), on any
canvas: the screen point over the scale factor, or where the pointer's camera ray
crosses a world canvas's plane — the screen centre while the cursor is locked
(#429). `canvas_position` is absent when the pointer is on neither. Map pointers
onto an element with these, never `position` against `GetRect(id).screen`: they
also work on world canvases. Pointer events **bubble**: a
press, release and click go to one entity, the nearest one from the hit entity
upward whose scripts define any of the three (or that carries a `Selectable`); a
drag goes to the nearest defining a drag callback; the wheel to the nearest
defining `OnScroll`. A `Selectable` that is not interactable cannot *start* an
interaction — press, drag, scroll, submit and cancel stop at it without firing.
Focus callbacks (`OnSelect` … `OnMove`) go to the focused entity itself. How
hits are found and what a Selectable adds is in [`docs/ui.md`](../ui.md).

**Deterministic dispatch order** (enforced in `src/scripting/lifecycle.rs`, so
headless replays stay byte-identical):

- Init is two-phase, at play-enter and again at the head of every tick's
  script phase: **all** pending `Awake`s run first, then **all** pending
  `Start`s — so a `Start` can safely read state another script set up in
  `Awake`, matching Unity's contract. Each phase, and `Update`, runs in
  ascending `(entity id, script index)` order.
- `LateUpdate` is the tick's tail: **all** `Update`s across every entity run
  first, then nav, physics, animation and particles resolve, and only then does
  `LateUpdate` run — in the same ascending `(entity id, script index)` order,
  with the same scaled `dt`. So within one tick a script's `Update` observes the
  pre-physics transform and its `LateUpdate` observes the post-physics one.
- Each `Awake` and `Start` fires **exactly once** per script instance, and
  `Awake` always precedes every other callback on that instance (a trigger
  callback never reaches a script whose `Awake` hasn't run).
- The trigger callbacks dispatch in a fixed per-frame phase order — all
  `OnTriggerEnter`, then all `OnTrigger`, then all `OnTriggerExit` — with each
  phase's pair list sorted ascending. The collision callbacks follow the same
  tick's triggers in the same shape — all `OnCollisionEnter`, then all
  `OnCollisionStay`, then all `OnCollisionExit`, each list sorted ascending by
  `(low id, high id)`. Every trigger and collision callback notifies both
  sides of its pair — A about B, then B about A — and an entity carrying
  several scripts is notified in ascending script-index order. The tick's
  `OnJointBreak`s come last, ascending by joint entity id.
- `OnEnable`/`OnDisable` are detected at the head of the script phase by diffing
  each instance's `active` state against the previous tick, in the order
  `OnDisable` (falling edges) → `Awake` → `OnEnable` (rising edges) → `Start` —
  so a first activation runs `Awake → OnEnable → Start`, all before that tick's
  `Update`. `OnDestroy` runs in a dedicated destroy phase at the **tail** of the
  tick (after `LateUpdate`), draining the ids `Scene.DestroyEntity` queued this
  tick: all `OnDisable`s first, then all `OnDestroy`s, in ascending
  `(entity id, script index)` order, before the entities are removed. Every
  edge fires exactly once, so replays stay byte-identical.
- The scene-load phase follows the destroy phase, still at the tick's tail: a
  `Scene.Load` requested this tick unloads the outgoing scene — the same
  teardown, all `OnDisable`s then all `OnDestroy`s in ascending
  `(entity id, script index)` order, for every entity that does not survive —
  and swaps the new scene in. Its scripts `Awake` / `OnEnable` / `Start` at
  the head of the next tick, like spawns.
- The UI callbacks run in their own phase between the init phase and
  `Update` (so a button reacts the same tick, and gameplay's `Update` already
  sees `UI.IsPointerConsumed()` settled): focus changes a script made last tick
  (`OnDeselect` → `OnSelect`), then keyboard navigation (or `OnMove`) and
  `OnSubmit` / `OnCancel`, then the pointer — `OnPointerExit` / `OnPointerEnter`, then each
  button (left, right, middle) in down → up → click → drag order, then
  `OnScroll`. A pointer that arrives and clicks in one tick reads enter → down
  → up → click.
- Right after **all** `Update`s (before nav/physics and `LateUpdate`) comes the
  timer phase: due [`Timer`](Timer.md) invokes fire, waiting coroutines resume
  and running [`Tween`](Tween.md)s write their properties, in ascending
  `(entity id, handle)` order.
- **One active gate for every gameplay hook.** A disabled entity receives *no*
  gameplay callback — not `Update`, `LateUpdate`, or the trigger and collision hooks. Going
  inactive is announced once by `OnDisable`; coming back is announced once by
  `OnEnable`.

> **Divergence from Unity — spawns are queued, not synchronous.** Unity runs
> `Awake` synchronously inside `Object.Instantiate`. In rusty a spawn
> (`Scene.Instantiate`, `Scene.CreateEntity`, …) usually happens *inside*
> another script's callback, mid-dispatch — a synchronous `Awake` would
> re-enter the script runtime. Instead the new entity's scripts are compiled at
> the head of the **next tick's** script phase, where their `Awake` and `Start`
> run before any `Update` of that tick, in the deterministic order above. Code
> must not expect a spawned object's `Awake` to have run on the line after
> `Instantiate` returns — configure the instance through the `Scene` /
> `Transform` verbs and let it initialize next tick (spawning it disabled and
> enabling it when ready defers init the same way).

> **Divergence from Unity — Stop does not fire `OnDestroy`.** Unity calls
> `OnDestroy` on every object when a scene is torn down. rusty does so when
> *gameplay* tears a scene down — `Scene.Load` unloads the outgoing scene with
> `OnDisable` then `OnDestroy`, as Unity does. But leaving Play (Stop)
> **restores the edit-mode snapshot** — it discards all play-mode state and
> rewinds to the authored scene, a reset rather than gameplay. Scripts must not
> observe it, so no `OnDisable`/`OnDestroy` fires on Stop. `OnDestroy` fires
> **only** for an entity removed *during* play, via `Scene.DestroyEntity` (the
> destructive verb) or a `Scene.Load` unload — that is real gameplay. `Scene.Deactivate` (Unity's
> deferred destroy) never fires `OnDestroy`; it flips `active`, so it fires
> `OnDisable` instead.

> **Drift gate:** the callback list is centralized in
> `src/scripting/callbacks.rs` (dispatch and MonoBehaviour discovery both read
> it), and `tests/callback_doc_drift.rs` fails CI when this section and that
> list disagree — the callback sibling of the #280 namespace gate
> (`tests/api_doc_drift.rs`).

---

## Namespaces

One file per namespace, in reference order:

- [`Transform`](Transform.md)
- [`Material`](Material.md)
- [`Animator`](Animator.md)
- [`Input`](Input.md)
- [`Scene`](Scene.md)
- [`Assets`](Assets.md)
- [`Texture`](Texture.md)
- [`Shader`](Shader.md)
- [`Sound`](Sound.md)
- [`Navigation`](Navigation.md)
- [`NavMeshAgent`](NavMeshAgent.md)
- [`NavMeshObstacle`](NavMeshObstacle.md)
- [`Physics`](Physics.md)
- [`Joint`](Joint.md)
- [`CharacterController`](CharacterController.md)
- [`Time`](Time.md)
- [`Random`](Random.md)
- [`Timer`](Timer.md)
- [`Tween`](Tween.md)
- [`Camera`](Camera.md)
- [`Light`](Light.md)
- [`LODGroup`](LODGroup.md)
- [`Probe`](Probe.md)
- [`Reflection`](Reflection.md)
- [`Lighting`](Lighting.md)
- [`Particles`](Particles.md)
- [`Trail`](Trail.md)
- [`Line`](Line.md)
- [`Audio`](Audio.md)
- [`AudioReverbZone`](AudioReverbZone.md)
- [`Decals`](Decals.md)
- [`Layers`](Layers.md)
- [`Graphics`](Graphics.md)
- [`Video`](Video.md)
- [`Canvas`](Canvas.md)
- [`RectTransform`](RectTransform.md)
- [`UI`](UI.md)
- [`Image`](Image.md)
- [`CanvasGroup`](CanvasGroup.md)
- [`RectMask`](RectMask.md)
- [`Mask`](Mask.md)
- [`BackdropFilter`](BackdropFilter.md)
- [`Selectable`](Selectable.md)
- [`LayoutGroup`](LayoutGroup.md)
- [`LayoutElement`](LayoutElement.md)
- [`Text`](Text.md)
- [`Shape`](Shape.md)
- [`Application`](Application.md)
- [`Storage`](Storage.md)
- [`Debug`](Debug.md)

**The UI widget kit** (#422) — Button, Toggle, Toggle Group, Slider, Scrollbar,
Scroll View, Dropdown, Input Field — is not a namespace: each widget is an
engine-shipped Lua **script component**. Build one with `UI.Create` (or
`Scene.Instantiate` of its prefab) and drive it through its script table,
`Scene.GetScript(id, "slider").set_value(0.5)`; each widget's fields and owner API
are in [Widgets in `docs/ui.md`](../ui.md#widgets).

---

## Script field schema (inspector decorators)

The engine's equivalent of Unity's `[SerializeField]` + `[Range]` / `[Tooltip]` /
`[Header]` attributes. A script may `return` an optional `fields` table describing
its inspector-editable serialized fields. The generic Lua Script inspector then
renders a typed control per field — no hand-written egui card needed — and the
values you set persist with the scene.

```lua
return {
  fields = {
    speed   = { type = "number", range = {0, 10}, default = 3.0, tooltip = "units/sec", header = "Movement" },
    jumps   = { type = "number", default = 2 },
    canFly  = { type = "boolean", default = false },
    label   = { type = "string", default = "Rusty" },
    -- A bare value is shorthand: its type is inferred and it becomes the default.
    health  = 100,
  },
  Start  = function(id) end,
  Update = function(id, dt) end,
}
```

At Play — before `Start` runs — each field is written as a key on the table the
script returns (the inspector override, or the schema `default`). So write the
script in the `local M = {} … return M` form and read the configured value off
that captured table:

```lua
local M = {}
M.speed = 0  -- overwritten by the inspector value at load
function M.Update(id, dt)
  local x = Transform.GetPosition(id)
  Transform.SetPosition(id, x + M.speed * dt, 0, 0)
end
return M
```

**Supported `type` values** (omit `type` and it's inferred from `default`):

| `type` | Inspector control | Stored as |
|---|---|---|
| `"number"` | drag value, or a **slider** when `range` is given | `f64` |
| `"boolean"` (`"bool"`) | checkbox | `bool` |
| `"string"` (`"text"`) | single-line text edit | `String` |

**Decorators** (all optional):

| Key | Effect |
|---|---|
| `range = {min, max}` | Renders a slider clamped to `[min, max]` (numbers only). |
| `default = <value>` | Initial value before the inspector overrides it; also fixes the inferred type. |
| `tooltip = "..."` | Hover text on the control. |
| `header = "..."` | A bold section label drawn above the field. |

Fields with no metadata fall back to a default control inferred from their value.
Fields are listed in the inspector sorted by name (Lua table order is
unspecified). Edited values are stored per script instance in the scene file, so
two entities running the same script can carry different field values.
