//! src/scene/authoring_defaults.rs — Default component values for authoring.
//!
//! The single source of truth for the values the inspector's "Add Component" menu
//! and the hierarchy toolbar's light primitives use. Split out of
//! `scene::authoring` to keep that module under the size cap; both the editor and
//! the `Scene.AddComponent` API read these so a component added either way is
//! identical.

use glam::Vec3;

use crate::scene::{
    AnimatorComponent, CameraComponent, ClearFlags, ColliderComponent, ColliderShape,
    HealthComponent, LightComponent, LightType, NavMeshAgentComponent, RigidBodyComponent,
    TextureComponent, Tonemap, VisualCorrectionComponent,
};

/// Construct a `LightComponent` with the standard cone defaults. Shared by the
/// toolbar's light primitives and the Add-Component `Light` default.
pub(crate) fn light(
    light_type: LightType,
    color: Vec3,
    intensity: f32,
    range: f32,
) -> LightComponent {
    LightComponent {
        light_type,
        color,
        intensity,
        range,
        inner_cone: 30.0,
        outer_cone: 45.0,
    }
}

/// Default `LightComponent` (the Add-Component menu's values).
pub fn default_light() -> LightComponent {
    light(LightType::Point, Vec3::ONE, 1.5, 10.0)
}

/// Default `HealthComponent` (the Add-Component menu's values).
pub fn default_health() -> HealthComponent {
    HealthComponent {
        current_health: 100.0,
        max_health: 100.0,
        is_dead: false,
    }
}

/// Default `AnimatorComponent` (the Add-Component menu's values).
pub fn default_animator() -> AnimatorComponent {
    AnimatorComponent {
        current_clip: "Idle".to_string(),
        speed: 2.0,
        is_playing: true,
        ..Default::default()
    }
}

/// Default `ColliderComponent` (the Add-Component menu's values).
pub fn default_collider() -> ColliderComponent {
    ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size: Vec3::ONE },
        is_trigger: false,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    }
}

/// Default `RigidBodyComponent` (the Add-Component menu's values).
pub fn default_rigidbody() -> RigidBodyComponent {
    RigidBodyComponent {
        active: true,
        is_kinematic: false,
        mass: 1.0,
        velocity: Vec3::ZERO,
        use_gravity: true,
    }
}

/// Default `TextureComponent` (the Add-Component menu's values).
pub fn default_texture() -> TextureComponent {
    TextureComponent {
        path: String::new(),
        is_dirty: true,
        metallic: 0.0,
        roughness: 0.5,
        metallic_map: None,
        roughness_map: None,
        color: [1.0, 1.0, 1.0],
    }
}

/// Default `NavMeshAgentComponent` (the Add-Component menu's values).
pub fn default_nav_agent() -> NavMeshAgentComponent {
    NavMeshAgentComponent {
        active: true,
        radius: 0.5,
        target: Vec3::new(8.0, 1.0, 8.0),
        speed: 3.0,
        acceleration: 5.0,
        stopping_distance: 0.5,
        velocity: Vec3::ZERO,
        ..Default::default()
    }
}

/// Default `CameraComponent` (the Add-Component menu's values).
pub fn default_camera() -> CameraComponent {
    CameraComponent {
        active: true,
        fov: 45.0,
        near: 0.1,
        far: 200.0,
        culling_mask: u32::MAX,
        render_order: 0,
        clear_flags: ClearFlags::Skybox,
        motion_blur_active: true,
        motion_blur_samples: 64,
    }
}

/// Default `VisualCorrectionComponent` (the Add-Component menu's values). The
/// editor only offers this entry when a `Camera` is present; the API applies it
/// unconditionally, mirroring the inspector's defaulted value (a visual-correction
/// stack with no camera is simply inert).
pub fn default_visual_correction() -> VisualCorrectionComponent {
    VisualCorrectionComponent {
        active: true,
        bloom_active: true,
        bloom_intensity: 1.0,
        bloom_threshold: 0.8,
        exposure: 0.0,
        contrast: 1.0,
        saturation: 1.0,
        ssr_active: true,
        ssr_quality: "High".to_string(),
        ssr_temporal_upsampling: true,
        tonemap: Tonemap::Aces,
        gamma: 2.2,
    }
}
