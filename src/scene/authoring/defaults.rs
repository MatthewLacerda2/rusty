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
    CollisionDetection, LightComponent, LightType, MaterialAsset, MaterialComponent,
    NavMeshAgentComponent, RigidBodyComponent, Scene, Tonemap, VisualCorrectionComponent,
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
        angular_velocity: Vec3::ZERO,
        use_gravity: true,
        collision_detection: CollisionDetection::Discrete,
    }
}

/// Default `MaterialAsset` (the data a fresh library material starts from — the
/// Add-Component / "Material" menu's values: white base color, dielectric, mid
/// roughness, no maps).
pub fn default_material() -> MaterialAsset {
    MaterialAsset::default()
}

/// Convert an imported [`crate::asset::MaterialData`] (the asset-layer DTO) into a
/// renderable [`MaterialAsset`] for the scene's material library. This is the bridge
/// between the pure-data asset layer and the components layer; it lives in `scene`
/// because only this layer may see both.
///
/// glTF packs metallic + roughness into ONE `metallicRoughnessTexture` (metallic in
/// B, roughness in G), so the single `data.metallic_roughness_map` is mapped to BOTH
/// `metallic_map` AND `roughness_map`; the shader samples `.b` / `.g` from each, so
/// pointing both at the same image reads the correct channel. The base-color alpha is
/// dropped (the renderer's base color is RGB).
pub fn material_asset_from_import(data: &crate::asset::MaterialData) -> MaterialAsset {
    MaterialAsset {
        base_color: [data.base_color[0], data.base_color[1], data.base_color[2]],
        base_color_map: data.base_color_map.clone(),
        metallic: data.metallic,
        metallic_map: data.metallic_roughness_map.clone(),
        roughness: data.roughness,
        roughness_map: data.metallic_roughness_map.clone(),
        normal_map: data.normal_map.clone(),
        emissive: data.emissive,
        emissive_map: data.emissive_map.clone(),
        // Imported glTF/OBJ materials default to Opaque (#242): the importer does not
        // yet read glTF `alphaMode`/`alphaCutoff`, so transparency is authored after
        // import via the inspector or the `Material` API.
        ..MaterialAsset::default()
    }
}

/// Attach a `MaterialComponent` to entity `id` referencing a freshly-created default
/// library material (keyed `entity_{id}_material`), inserting it into the scene's
/// library. Returns `false` if the entity does not exist. Material is the one
/// "Add Component" verb that creates a shared library asset, not just entity data.
pub fn attach_default_material(scene: &mut Scene, id: u32) -> bool {
    let key = format!("entity_{id}_material");
    let attached = scene.world.set_material(
        id,
        Some(MaterialComponent {
            material: key.clone(),
        }),
    );
    if !attached {
        return false;
    }
    scene.materials.insert(key, default_material());
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

/// Default `VisualCorrectionComponent` (the Add-Component menu's values).
/// `VisualCorrection` declares `requires(Camera)` (#359), so every surface that adds
/// it auto-adds a `Camera` if missing — a correction stack is inert without a camera
/// to correct, so the dependency is enforced rather than left to chance.
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

#[cfg(test)]
mod tests {
    use super::material_asset_from_import;
    use crate::asset::MaterialData;

    #[test]
    fn import_conversion_fans_mr_map_to_both_channels() {
        let data = MaterialData {
            name: "Bricks".to_string(),
            base_color: [0.8, 0.4, 0.2, 0.5],
            metallic: 0.25,
            roughness: 0.75,
            base_color_map: Some("albedo.png".to_string()),
            metallic_roughness_map: Some("mr.png".to_string()),
            normal_map: Some("n.png".to_string()),
            emissive_map: Some("e.png".to_string()),
            emissive: [0.1, 0.2, 0.3],
        };
        let mat = material_asset_from_import(&data);

        // Base-color alpha is dropped; rgb carried through.
        assert_eq!(mat.base_color, [0.8, 0.4, 0.2]);
        assert_eq!(mat.metallic, 0.25);
        assert_eq!(mat.roughness, 0.75);
        assert_eq!(mat.base_color_map.as_deref(), Some("albedo.png"));
        // The single glTF MR texture fans out to BOTH engine maps (#203).
        assert_eq!(mat.metallic_map.as_deref(), Some("mr.png"));
        assert_eq!(mat.roughness_map.as_deref(), Some("mr.png"));
        assert_eq!(mat.normal_map.as_deref(), Some("n.png"));
        assert_eq!(mat.emissive, [0.1, 0.2, 0.3]);
        assert_eq!(mat.emissive_map.as_deref(), Some("e.png"));
    }
}
