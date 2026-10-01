//! src/components/entity/repr.rs — `Entity`'s on-disk deserialization shape.
//!
//! Split from `entity/mod.rs` to keep it under the size cap: the legacy-tolerant
//! `EntityRepr` and its migration into `Entity`.

use serde::Deserialize;

use super::{Entity, PrefabLink};
use crate::components::particle::LegacyEmitter;
use crate::components::{
    AnimatorComponent, AudioSourceComponent, CameraComponent, CanvasComponent,
    CanvasGroupComponent, ColliderComponent, ImageComponent, JointComponent,
    LayoutElementComponent, LayoutGroupComponent, LightComponent, LineComponent, LodGroupComponent,
    MaterialAsset, MaterialComponent, MeshComponent, NavMeshAgentComponent,
    ParticleEmitterComponent, RectMaskComponent, RectTransformComponent, RigidBodyComponent,
    ScriptComponent, SelectableComponent, TextComponent, TextureComponent, TrailComponent,
    TransformComponent, VisualCorrectionComponent,
};
use crate::components::{BackdropFilterComponent, MaskComponent};

/// On-disk shape used only for deserialization, so old single-`script` scenes
/// (pre-#83) keep loading. It accepts both the new `scripts: Vec<…>` and the
/// legacy `script: Option<…>` keys; the `From` impl migrates the singular into
/// the vec. Serialization always goes through `Entity` directly (new `scripts`
/// key), so files written today never carry the legacy field.
#[derive(Deserialize)]
pub(super) struct EntityRepr {
    id: u32,
    name: String,
    active: bool,
    is_static: bool,
    #[serde(default)]
    layer: u8,
    transform: TransformComponent,
    mesh: Option<MeshComponent>,
    /// Legacy pre-#201 inline material, migrated into a library material by `From`.
    #[serde(default)]
    texture: Option<TextureComponent>,
    /// New reference-to-library material (post-#201 scenes carry this directly).
    #[serde(default)]
    material: Option<MaterialComponent>,
    #[serde(default)]
    scripts: Vec<ScriptComponent>,
    /// Legacy singular field (pre-#83), migrated into `scripts` by `From`.
    #[serde(default)]
    script: Option<ScriptComponent>,
    animator: Option<AnimatorComponent>,
    light: Option<LightComponent>,
    collider: Option<ColliderComponent>,
    rigidbody: Option<RigidBodyComponent>,
    nav_agent: Option<NavMeshAgentComponent>,
    camera: Option<CameraComponent>,
    visual_correction: Option<VisualCorrectionComponent>,
    /// Read through the pre-#439 `size_end` migration.
    #[serde(default)]
    particles: Option<LegacyEmitter>,
    #[serde(default)]
    audio: Option<AudioSourceComponent>,
    #[serde(default)]
    canvas: Option<CanvasComponent>,
    #[serde(default)]
    rect_transform: Option<RectTransformComponent>,
    #[serde(default)]
    image: Option<ImageComponent>,
    #[serde(default)]
    canvas_group: Option<CanvasGroupComponent>,
    #[serde(default)]
    rect_mask: Option<RectMaskComponent>,
    #[serde(default)]
    mask: Option<MaskComponent>,
    #[serde(default)]
    backdrop_filter: Option<BackdropFilterComponent>,
    #[serde(default)]
    text: Option<TextComponent>,
    #[serde(default)]
    selectable: Option<SelectableComponent>,
    #[serde(default)]
    layout_group: Option<LayoutGroupComponent>,
    #[serde(default)]
    layout_element: Option<LayoutElementComponent>,
    #[serde(default)]
    joint: Option<JointComponent>,
    #[serde(default)]
    lod_group: Option<LodGroupComponent>,
    #[serde(default)]
    trail: Option<TrailComponent>,
    #[serde(default)]
    line: Option<LineComponent>,
    #[serde(default)]
    shape: Option<crate::components::ShapeComponent>,
    #[serde(default)]
    prefab_link: Option<PrefabLink>,
    parent_id: Option<u32>,
    children: Vec<u32>,
}

impl From<EntityRepr> for Entity {
    fn from(r: EntityRepr) -> Self {
        let mut scripts = r.scripts;
        // Migrate a pre-#83 single `script` into the plural vec, ahead of any
        // (normally empty) new-format scripts, so legacy attachments still run.
        if let Some(legacy) = r.script {
            scripts.insert(0, legacy);
        }
        let (material, pending_material) = migrate_material(r.id, r.material, r.texture);
        Self {
            id: r.id,
            name: r.name,
            active: r.active,
            is_static: r.is_static,
            layer: r.layer,
            transform: r.transform,
            mesh: r.mesh,
            material,
            pending_material,
            scripts,
            animator: r.animator,
            light: r.light,
            collider: r.collider,
            rigidbody: r.rigidbody,
            nav_agent: r.nav_agent,
            camera: r.camera,
            visual_correction: r.visual_correction,
            particles: r.particles.map(ParticleEmitterComponent::from),
            audio: r.audio,
            canvas: r.canvas,
            rect_transform: r.rect_transform,
            image: r.image,
            canvas_group: r.canvas_group,
            rect_mask: r.rect_mask,
            mask: r.mask,
            backdrop_filter: r.backdrop_filter,
            text: r.text,
            selectable: r.selectable,
            layout_group: r.layout_group,
            layout_element: r.layout_element,
            joint: r.joint,
            lod_group: r.lod_group,
            trail: r.trail,
            line: r.line,
            shape: r.shape,
            prefab_link: r.prefab_link,
            parent_id: r.parent_id,
            children: r.children,
        }
    }
}

/// Material migration: a new-format `material` reference is taken verbatim; a
/// legacy inline `texture` becomes a per-entity library material whose data is
/// carried in `pending_material` for `apply_scene_data` to insert by name.
fn migrate_material(
    id: u32,
    material: Option<MaterialComponent>,
    texture: Option<TextureComponent>,
) -> (Option<MaterialComponent>, Option<MaterialAsset>) {
    match (material, texture) {
        (Some(m), _) => (Some(m), None),
        (None, Some(t)) => (
            Some(MaterialComponent {
                material: format!("entity_{id}_material"),
            }),
            Some(MaterialAsset::from_legacy(&t)),
        ),
        (None, None) => (None, None),
    }
}
