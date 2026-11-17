//! crates/scene/src/create/defaults.rs — Default component values.
//!
//! The single source of truth for the values the inspector's "Add Component" menu
//! and the hierarchy toolbar's light primitives use. Both the editor and the
//! `Scene.AddComponent` API read these (through `edit::authoring`'s re-exports), so
//! a component added either way is identical; the base builds with them too (a
//! ragdoll's bodies, a primitive's collider).

use glam::Vec3;

use crate::{
    AnimatorComponent, CameraComponent, ClearFlags, ColliderComponent, ColliderShape,
    CollisionDetection, LightComponent, LightType, MaterialAsset, MaterialComponent,
    NavMeshAgentComponent, PostProcessingComponent, RigidBodyComponent, Scene,
};
use rusty_components::material::entity_material_key;
use rusty_components::Playback;

/// The cone angles (degrees) a new light carries, and a spot seeds when it has none.
pub const DEFAULT_CONES: (f32, f32) = (30.0, 45.0);

/// Construct a `LightComponent` with the standard cone defaults. Shared by the
/// toolbar's light primitives and the Add-Component `Light` default.
pub(crate) fn light(
    light_type: LightType,
    color: Vec3,
    intensity: f32,
    range: f32,
) -> LightComponent {
    LightComponent {
        area_size: glam::Vec2::ONE,
        light_type,
        color,
        intensity,
        range,
        inner_cone: DEFAULT_CONES.0,
        outer_cone: DEFAULT_CONES.1,
        cast_shadows: false,
        mode: Default::default(),
        shadow_fade_distance: 0.0,
    }
}

/// Default `LightComponent` (the Add-Component menu's values).
pub fn default_light() -> LightComponent {
    light(LightType::Point, Vec3::ONE, 1.5, 10.0)
}

/// Default `AnimatorComponent` (the Add-Component menu's values).
pub fn default_animator() -> AnimatorComponent {
    AnimatorComponent {
        base: Playback {
            current_clip: "Idle".to_string(),
            ..Default::default()
        },
        speed: 1.0,
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
        material: Default::default(),
        material_asset: None,
        shared_mesh: None,
        collision_mesh: None,
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
        angular_velocity: Vec3::ZERO,
        use_gravity: true,
        collision_detection: CollisionDetection::Discrete,
        drag: 0.0,
        angular_drag: rusty_components::DEFAULT_ANGULAR_DRAG,
    }
}

/// Default `MaterialAsset` (the data a fresh library material starts from — the
/// Add-Component / "Material" menu's values: white base color, dielectric, mid
/// roughness, no maps).
pub fn default_material() -> MaterialAsset {
    MaterialAsset::default()
}

/// Attach a `MaterialComponent` to entity `id` referencing its own library material
/// (keyed [`entity_material_key`]), creating a default one in the scene's library
/// when none is there. A surviving asset under that key (the entity's material from
/// before a remove, possibly shared by others) is kept, never reset: the same rule as
/// `Material.Set*`'s resolve-or-create (#997), and Unity's, where adding a renderer
/// never touches a material asset. Returns `false` if the entity does not exist.
/// Material is the one "Add Component" verb that creates a shared library asset, not
/// just entity data.
pub fn attach_default_material(scene: &mut Scene, id: u32) -> bool {
    if !scene
        .world
        .set_material(id, Some(MaterialComponent::for_entity(id)))
    {
        return false;
    }
    scene
        .materials
        .entry(entity_material_key(id))
        .or_insert_with(default_material);
    true
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
        fov: rusty_components::rendering::camera::DEFAULT_FOV,
        near: rusty_components::rendering::camera::DEFAULT_NEAR,
        far: rusty_components::rendering::camera::DEFAULT_FAR,
        fov_axis: Default::default(),
        culling_mask: u32::MAX,
        render_order: 0,
        clear_flags: ClearFlags::Skybox,
        motion_blur_active: true,
        motion_blur_quality: Default::default(),
        motion_blur_samples: rusty_components::CameraComponent::MAX_MOTION_BLUR_SAMPLES,
        motion_blur_intensity: CameraComponent::DEFAULT_MOTION_BLUR_INTENSITY,
        motion_blur_max_length: CameraComponent::DEFAULT_MOTION_BLUR_MAX_LENGTH,
        fxaa_active: true,
        taa_active: false,
        taa_quality: Default::default(),
        taa_base_blend_factor: CameraComponent::DEFAULT_TAA_BASE_BLEND_FACTOR,
        taa_variance_clamp_scale: CameraComponent::DEFAULT_TAA_VARIANCE_CLAMP_SCALE,
        taa_jitter_scale: 1.0,
        taa_mip_bias: 0.0,
        taa_contrast_adaptive_sharpening: 0.0,
        taa_anti_flicker: CameraComponent::DEFAULT_TAA_ANTI_FLICKER,
        projection: Default::default(),
        use_occlusion_culling: true,
        target_texture: None,
    }
}

/// Default `PostProcessingComponent` (the Add-Component menu's values).
/// `PostProcessing` declares `requires(Camera)` (#359), so every surface that adds
/// it auto-adds a `Camera` if missing — post-processing is inert without a camera
/// to process, so the dependency is enforced rather than left to chance.
pub fn default_post_processing() -> PostProcessingComponent {
    PostProcessingComponent::default()
}
