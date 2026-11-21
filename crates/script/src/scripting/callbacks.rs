/// Deliberately not Unity's synchronous `Awake`: an entity spawned during play
/// gets it at the head of the next tick's script phase (`docs/api/index.md`).
pub const AWAKE: &str = "Awake";
pub const START: &str = "Start";
pub const UPDATE: &str = "Update";
pub const LATE_UPDATE: &str = "LateUpdate";
pub const ON_TRIGGER_ENTER: &str = "OnTriggerEnter";
pub const ON_TRIGGER: &str = "OnTrigger";
pub const ON_TRIGGER_EXIT: &str = "OnTriggerExit";
pub const ON_COLLISION_ENTER: &str = "OnCollisionEnter";
pub const ON_COLLISION_STAY: &str = "OnCollisionStay";
pub const ON_COLLISION_EXIT: &str = "OnCollisionExit";
pub const ON_CONTROLLER_COLLIDER_HIT: &str = "OnControllerColliderHit";
pub const ON_JOINT_BREAK: &str = "OnJointBreak";
pub const ON_ANIMATION_EVENT: &str = "OnAnimationEvent";
pub const ON_STATE_ENTER: &str = "OnStateEnter";
pub const ON_STATE_UPDATE: &str = "OnStateUpdate";
pub const ON_STATE_EXIT: &str = "OnStateExit";
pub const ON_PERCEPTION_UPDATED: &str = "OnPerceptionUpdated";
pub const ON_TAKE_DAMAGE: &str = "OnTakeDamage";
pub const ON_ENABLE: &str = "OnEnable";
pub const ON_DISABLE: &str = "OnDisable";
/// Deliberately **not** fired on play-exit (Stop restores the edit snapshot, a reset,
/// not gameplay): a divergence from Unity's scene-teardown `OnDestroy`.
pub const ON_DESTROY: &str = "OnDestroy";

pub const ON_POINTER_ENTER: &str = "OnPointerEnter";
pub const ON_POINTER_EXIT: &str = "OnPointerExit";
pub const ON_POINTER_DOWN: &str = "OnPointerDown";
pub const ON_POINTER_UP: &str = "OnPointerUp";
pub const ON_POINTER_CLICK: &str = "OnPointerClick";
pub const ON_BEGIN_DRAG: &str = "OnBeginDrag";
pub const ON_DRAG: &str = "OnDrag";
pub const ON_END_DRAG: &str = "OnEndDrag";
pub const ON_SCROLL: &str = "OnScroll";
pub const ON_SELECT: &str = "OnSelect";
pub const ON_DESELECT: &str = "OnDeselect";
pub const ON_SUBMIT: &str = "OnSubmit";
pub const ON_CANCEL: &str = "OnCancel";
/// An arrow key was pressed while the entity is focused (Unity's `IMoveHandler`):
/// `(id, { direction, x, y })`. Defining it takes the arrows off navigation — focus
/// stays put and the script moves it itself (`UI.FindSelectable`) if it wants.
pub const ON_MOVE: &str = "OnMove";

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
    ON_CONTROLLER_COLLIDER_HIT,
    ON_JOINT_BREAK,
    ON_ANIMATION_EVENT,
    ON_STATE_ENTER,
    ON_STATE_UPDATE,
    ON_STATE_EXIT,
    ON_PERCEPTION_UPDATED,
    ON_TAKE_DAMAGE,
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
    ON_MOVE,
    ON_DISABLE,
    ON_DESTROY,
];
