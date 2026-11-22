use glam::Vec3;
use serde::{Deserialize, Serialize};

pub const COLLIDED_BELOW: u8 = 1;
pub const COLLIDED_SIDES: u8 = 2;
pub const COLLIDED_ABOVE: u8 = 4;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CharacterControllerComponent {
    pub height: f32,
    pub radius: f32,
    pub center: Vec3,
    pub step_offset: f32,
    pub slope_limit: f32,
    pub skin_width: f32,
    pub min_move_distance: f32,
    #[serde(skip)]
    pub is_grounded: bool,
    #[serde(skip)]
    pub collision_flags: u8,
    #[serde(skip, default = "up")]
    pub ground_normal: Vec3,
}

fn up() -> Vec3 {
    Vec3::Y
}

impl Default for CharacterControllerComponent {
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

impl crate::FirstClass for CharacterControllerComponent {
    const NAME: &'static str = "CharacterController";
    const ALIASES: &'static [&'static str] = &["character"];
}
