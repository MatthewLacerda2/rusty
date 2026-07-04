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
    ON_DISABLE,
    ON_DESTROY,
];
