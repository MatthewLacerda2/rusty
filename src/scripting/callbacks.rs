//! src/scripting/callbacks.rs — the script lifecycle-callback names, in one place.
//!
//! These are the MonoBehaviour-style functions the engine looks up on the table a
//! script returns. `lifecycle` dispatches them, `discovery` uses the list to
//! recognize a MonoBehaviour, and the doc-drift gate
//! (`tests/callback_doc_drift.rs`, #309) checks `docs/scripting-api.md` against
//! it — one list, so dispatch, discovery, and doc can never disagree about which
//! callbacks exist.

/// Called once per script instance when it first participates in the sim
/// (#322): at play-enter for active scene entities, at the head of the next
/// tick's script phase for entities spawned during play (a deliberate
/// divergence from Unity's synchronous `Awake` — see `docs/scripting-api.md`),
/// or on the first active tick for entities loaded disabled. Always the
/// instance's first callback.
pub const AWAKE: &str = "Awake";
/// Called once per script instance, after its `Awake`, immediately before its
/// first `Update` — deferred while the owning entity is inactive.
pub const START: &str = "Start";
/// Called every frame of play while the owning entity is active.
pub const UPDATE: &str = "Update";
/// Called every frame of play while the owning entity is active, after **every**
/// script's `Update` and after this tick's physics, animation and particle steps
/// have resolved — the post-physics hook (#324). Same scaled `dt` and same
/// deterministic `(entity, script index)` order as `Update`, still inside the one
/// `FixedUpdate` sim tick (it is not a render-rate callback). Lets a follow-cam /
/// look-at / aim / recoil script read *this* tick's settled transforms.
pub const LATE_UPDATE: &str = "LateUpdate";
/// Called once, the tick a trigger-overlap pair involving the owning entity
/// begins (#310) — before that tick's `OnTrigger`.
pub const ON_TRIGGER_ENTER: &str = "OnTriggerEnter";
/// Called once per trigger-overlap pair involving the owning entity, each frame
/// the overlap persists (the "stay" callback, including the enter tick).
pub const ON_TRIGGER: &str = "OnTrigger";
/// Called once, the tick after a trigger-overlap pair involving the owning
/// entity ends (#310) — after that tick's `OnTrigger` dispatches.
pub const ON_TRIGGER_EXIT: &str = "OnTriggerExit";
/// Called once, the tick a solid (non-trigger) contact involving the owning
/// entity's collider begins (#448) — `(id, other, contact)`, before that tick's
/// `OnCollisionStay`.
pub const ON_COLLISION_ENTER: &str = "OnCollisionEnter";
/// Called once per touching solid pair involving the owning entity, each tick
/// the contact persists (including the enter tick) — `(id, other, contact)`.
pub const ON_COLLISION_STAY: &str = "OnCollisionStay";
/// Called once, the tick after a solid contact involving the owning entity ends
/// (#448) — `(id, other)`, after that tick's `OnCollisionStay` dispatches.
pub const ON_COLLISION_EXIT: &str = "OnCollisionExit";
/// Called once, the tick the owning entity's `Joint` breaks past its
/// `break_force` / `break_torque` (#449) — `(id, force, torque)`, after that
/// tick's collision callbacks. The component is already gone.
pub const ON_JOINT_BREAK: &str = "OnJointBreak";
/// Called when the owning entity becomes active (#323): on its first activation
/// — between `Awake` and `Start`, so the first-tick order is
/// `Awake → OnEnable → Start` — and again on every later inactive→active edge.
/// Detected by diffing the entity's `active` flag against the previous tick, so
/// it fires exactly once per rising edge.
pub const ON_ENABLE: &str = "OnEnable";
/// Called when the owning entity becomes inactive (#323): once on each
/// active→inactive edge, and once more — immediately before `OnDestroy` — when
/// an active entity is destroyed. Fires exactly once per falling edge.
pub const ON_DISABLE: &str = "OnDisable";
/// Called once when the entity is removed during play via `Scene.DestroyEntity`
/// (#323), after its `OnDisable` if it was active. Deliberately **not** fired on
/// play-exit (Stop restores the edit snapshot — a reset, not gameplay), a
/// documented divergence from Unity's scene-teardown `OnDestroy`.
pub const ON_DESTROY: &str = "OnDestroy";

// --- UI callbacks (#420). The event system (`ui::events`) decides who receives
// them: pointer, drag and scroll events bubble from the hit entity to its nearest
// ancestor-or-self defining the callback; focus events go to the focused entity.
// Each is dispatched at the head of the script phase, after `Start` and before
// `Update`. The pointer family is called `(id, event)`, the focus family `(id)`.

/// The pointer entered the entity or one of its descendants (Unity's `IPointerEnterHandler`).
pub const ON_POINTER_ENTER: &str = "OnPointerEnter";
/// The pointer left the entity and all its descendants.
pub const ON_POINTER_EXIT: &str = "OnPointerExit";
/// A mouse button went down over the entity.
pub const ON_POINTER_DOWN: &str = "OnPointerDown";
/// The button pressed over the entity was released (wherever the pointer is).
pub const ON_POINTER_UP: &str = "OnPointerUp";
/// A press and release both over the entity.
pub const ON_POINTER_CLICK: &str = "OnPointerClick";
/// A held pointer started moving past the drag threshold.
pub const ON_BEGIN_DRAG: &str = "OnBeginDrag";
/// The dragged pointer moved this tick.
pub const ON_DRAG: &str = "OnDrag";
/// The dragging button was released.
pub const ON_END_DRAG: &str = "OnEndDrag";
/// The mouse wheel turned over the entity.
pub const ON_SCROLL: &str = "OnScroll";
/// The entity became the focused (selected) one.
pub const ON_SELECT: &str = "OnSelect";
/// The entity stopped being the focused one.
pub const ON_DESELECT: &str = "OnDeselect";
/// Enter was pressed while the entity is focused.
pub const ON_SUBMIT: &str = "OnSubmit";
/// Escape was pressed while the entity is focused.
pub const ON_CANCEL: &str = "OnCancel";

/// Every callback the engine dispatches — the ground truth the doc gate reads.
pub const LIFECYCLE_CALLBACKS: &[&str] = &[
    AWAKE,
    ON_ENABLE,
    START,
    UPDATE,
    LATE_UPDATE,
    ON_TRIGGER_ENTER,
    ON_TRIGGER,
    ON_TRIGGER_EXIT,
    ON_COLLISION_ENTER,
    ON_COLLISION_STAY,
    ON_COLLISION_EXIT,
    ON_JOINT_BREAK,
    ON_POINTER_ENTER,
    ON_POINTER_EXIT,
    ON_POINTER_DOWN,
    ON_POINTER_UP,
    ON_POINTER_CLICK,
    ON_BEGIN_DRAG,
    ON_DRAG,
    ON_END_DRAG,
    ON_SCROLL,
    ON_SELECT,
    ON_DESELECT,
    ON_SUBMIT,
    ON_CANCEL,
    ON_DISABLE,
    ON_DESTROY,
];
