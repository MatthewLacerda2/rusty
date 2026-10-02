//! src/components/character_controller.rs — CharacterController (#451).
//!
//! Unity's `CharacterController`: an upright capsule that moves only when a
//! script calls `CharacterController.Move` with a displacement, and moves by
//! **collide-and-slide** — it stops at walls and slides along them, climbs
//! steps up to `step_offset`, walks up slopes up to `slope_limit` and slides back
//! off steeper ones. It applies no gravity: scripts own gravity, jumps and air
//! control, and read the result back (`is_grounded`, the collision flags, the
//! ground normal).
//!
//! The capsule is the entity's collider in the physics world — `height` and
//! `radius` scale with the Transform (height by `y`, radius by the larger of `x`
//! and `z`), `center` is its offset in local space, and it stays upright however
//! the entity turns. A Collider on the same entity is superseded by it. The body
//! is kinematic: between `Move` calls it sits wherever its Transform says.
//!
//! The runtime fields (`is_grounded`, `collision_flags`, `ground_normal`) are what
//! the last `Move` found. They are never saved.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// `Move` touched something below the capsule (Unity `CollisionFlags.Below`).
pub const COLLIDED_BELOW: u8 = 1;
/// `Move` touched something on the capsule's sides (`CollisionFlags.Sides`).
pub const COLLIDED_SIDES: u8 = 2;
/// `Move` touched something above the capsule (`CollisionFlags.Above`).
pub const COLLIDED_ABOVE: u8 = 4;

/// A collide-and-slide capsule driven by displacement. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CharacterControllerComponent {
    /// Full capsule height, caps included (metres, before scale).
    pub height: f32,
    /// Capsule radius (metres, before scale).
    pub radius: f32,
    /// The capsule's centre, in the entity's local space.
    pub center: Vec3,
    /// The tallest step (stair, curb) a move climbs without being blocked.
    pub step_offset: f32,
    /// The steepest slope (degrees from flat) a move walks up; steeper ones block
    /// and slide the character back down.
    pub slope_limit: f32,
    /// The gap kept between the capsule and what it touches.
    pub skin_width: f32,
    /// A move shorter than this does nothing.
    pub min_move_distance: f32,
    /// Whether the last move ended on the ground.
    #[serde(skip)]
    pub is_grounded: bool,
    /// What the last move touched: a mask of the `COLLIDED_*` bits.
    #[serde(skip)]
    pub collision_flags: u8,
    /// The surface normal under the character after the last move (`Vec3::Y`
    /// when not grounded).
    #[serde(skip, default = "up")]
    pub ground_normal: Vec3,
}

fn up() -> Vec3 {
    Vec3::Y
}

impl Default for CharacterControllerComponent {
    /// Unity's defaults: a 2 m tall, 0.5 m radius capsule centred on the entity.
    fn default() -> Self {
        Self {
            height: 2.0,
            radius: 0.5,
            center: Vec3::ZERO,
            step_offset: 0.3,
            slope_limit: 45.0,
            skin_width: 0.08,
            min_move_distance: 0.001,
            is_grounded: false,
            collision_flags: 0,
            ground_normal: Vec3::Y,
        }
    }
}
