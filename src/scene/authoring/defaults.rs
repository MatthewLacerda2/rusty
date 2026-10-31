//! src/scene/authoring_defaults.rs — Default component values for authoring.
//!
//! The single source of truth for the values the inspector's "Add Component" menu
//! and the hierarchy toolbar's light primitives use. Split out of
//! `scene::authoring` to keep that module under the size cap; both the editor and
//! the `Scene.AddComponent` API read these so a component added either way is
//! identical.

use glam::Vec3;

use crate::components::material::entity_material_key;
use crate::components::{Playback, RenderFace};
use crate::scene::{
    AnimatorComponent, CameraComponent, ClearFlags, ColliderComponent, ColliderShape,
    CollisionDetection, LightComponent, LightType, MaterialAsset, MaterialComponent,
    NavMeshAgentComponent, PostProcessingComponent, RigidBodyComponent, Scene,
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
        inner_cone: super::rendering::light::DEFAULT_CONES.0,
        outer_cone: super::rendering::light::DEFAULT_CONES.1,
        cast_shadows: false,
        mode: Default::default(),
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
    // Unity's model importer marks a bump map a material names as a Normal map,
    // and its packed metallic/roughness map linear (#1574), so each compresses
    // as what it holds.
    let asset = MaterialAsset {
        base_color: [data.base_color[0], data.base_color[1], data.base_color[2]],
        base_color_map: data.base_color_map.clone(),
        metallic: data.metallic,
        metallic_map: data.metallic_roughness_map.clone(),
        roughness: data.roughness,
        roughness_map: data.metallic_roughness_map.clone(),
        normal_map: data.normal_map.clone(),
        emissive: data.emissive,
        emissive_map: data.emissive_map.clone(),
        // A double-sided glTF material draws both faces (#1615), as Unity's glTF
        // importers set *Render Face* to Both.
        render_face: match data.double_sided {
            true => RenderFace::Both,
            false => RenderFace::Front,
        },
        // Imported glTF/OBJ materials default to Opaque (#242): the importer does not
        // yet read glTF `alphaMode`/`alphaCutoff`, so transparency is authored after
        // import via the inspector or the `Material` API.
        ..MaterialAsset::default()
    };
    super::material::fit_map_slots(&asset);
    asset
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
        fov: crate::components::rendering::camera::DEFAULT_FOV,
        near: crate::components::rendering::camera::DEFAULT_NEAR,
        far: crate::components::rendering::camera::DEFAULT_FAR,
        culling_mask: u32::MAX,
        render_order: 0,
        clear_flags: ClearFlags::Skybox,
        motion_blur_active: true,
        motion_blur_quality: Default::default(),
        motion_blur_samples: crate::components::CameraComponent::MAX_MOTION_BLUR_SAMPLES,
        motion_blur_intensity: CameraComponent::DEFAULT_MOTION_BLUR_INTENSITY,
        motion_blur_max_length: CameraComponent::DEFAULT_MOTION_BLUR_MAX_LENGTH,
        fxaa_active: true,
        projection: Default::default(),
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
            base_color_map: Some("albedo.png".into()),
            metallic_roughness_map: Some("mr.png".into()),
            normal_map: Some("n.png".into()),
            emissive_map: Some("e.png".into()),
            emissive: [0.1, 0.2, 0.3],
            double_sided: false,
        };
        let mat = material_asset_from_import(&data);
        assert_eq!(mat.render_face, crate::components::RenderFace::Front);
        let both = MaterialData {
            double_sided: true,
            ..data.clone()
        };
        let face = material_asset_from_import(&both).render_face;
        assert_eq!(
            face,
            crate::components::RenderFace::Both,
            "glTF doubleSided"
        );

        // Base-color alpha is dropped; rgb carried through.
        assert_eq!(mat.base_color, [0.8, 0.4, 0.2]);
        assert_eq!(mat.metallic, 0.25);
        assert_eq!(mat.roughness, 0.75);
        assert_eq!(mat.base_color_map, Some("albedo.png".into()));
        // The single glTF MR texture fans out to BOTH engine maps (#203).
        assert_eq!(mat.metallic_map, Some("mr.png".into()));
        assert_eq!(mat.roughness_map, Some("mr.png".into()));
        assert_eq!(mat.normal_map, Some("n.png".into()));
        assert_eq!(mat.emissive, [0.1, 0.2, 0.3]);
        assert_eq!(mat.emissive_map, Some("e.png".into()));
    }

    #[test]
    fn an_imported_normal_map_becomes_a_normal_map() {
        use crate::asset::sidecar::{texture, TextureType};
        let dir = crate::test_temp::dir();
        let png = dir.join("bumps.png");
        image::RgbaImage::new(4, 4).save(&png).unwrap();
        let data = MaterialData {
            normal_map: Some(png.to_string_lossy().into_owned().into()),
            ..MaterialData::default()
        };
        material_asset_from_import(&data);
        assert_eq!(texture(&png).texture_type, TextureType::NormalMap);
    }
}
