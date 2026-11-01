//! src/scene/authoring/fields/kinds/mod.rs — every component `Component.*` reaches, by name.
//!
//! One [`Kind`] per first-class component (each of `Entity`'s `Option<…Component>`
//! fields) plus the mandatory `Transform`; the field tables live beside, grouped
//! by area.

mod fx;
mod graphic;
mod nav;
mod physics;
mod render;
mod ui;

use super::introspect::field_type;
use super::table::{kind, Kind};
use crate::components::animator::Playback;
use crate::components::*;
use crate::scene::authoring::{ui::shader as ui_shader, ComponentKind as K};

/// The reason on a field no inspector op writes yet; #1368 gates them in.
const NO_OP: &str = "no inspector op writes it yet (#1368)";

// Not sorted: Transform and Mesh first, then the Add Component menu's order.
/// Every component, by the name `Component.*` answers to first.
pub const KINDS: &[Kind] = &[
    kind::<TransformComponent>("Transform", None, "transform"),
    kind::<MeshComponent>("Mesh", Some(K::Mesh), "mesh"),
    kind::<LightComponent>("Light", Some(K::Light), "light"),
    Kind {
        field_type: animator_field_type,
        ..kind::<AnimatorComponent>("Animator", Some(K::Animator), "animator")
    },
    kind::<ColliderComponent>("Collider", Some(K::Collider), "collider"),
    kind::<RigidBodyComponent>("RigidBody", Some(K::RigidBody), "rigidbody"),
    kind::<MaterialComponent>("Material", Some(K::Texture), "material"),
    kind::<NavMeshAgentComponent>("NavMeshAgent", Some(K::NavMeshAgent), "nav_agent"),
    kind::<NavMeshObstacleComponent>("NavMeshObstacle", Some(K::NavMeshObstacle), "nav_obstacle"),
    kind::<OffMeshLinkComponent>("OffMeshLink", Some(K::OffMeshLink), "offmesh_link"),
    kind::<NavMeshModifierVolumeComponent>(
        "NavMeshModifierVolume",
        Some(K::NavMeshModifierVolume),
        "nav_modifier",
    ),
    kind::<CameraComponent>("Camera", Some(K::Camera), "camera"),
    kind::<ParticleEmitterComponent>("Particles", Some(K::Particles), "particles"),
    kind::<PostProcessingComponent>("PostProcessing", Some(K::PostProcessing), "post_processing"),
    kind::<AudioSourceComponent>("AudioSource", Some(K::Audio), "audio"),
    kind::<ReverbZoneComponent>("AudioReverbZone", Some(K::ReverbZone), "reverb_zone"),
    kind::<CanvasComponent>("Canvas", Some(K::Canvas), "canvas"),
    kind::<RectTransformComponent>("RectTransform", Some(K::RectTransform), "rect_transform"),
    kind::<ImageComponent>("Image", Some(K::Image), "image"),
    kind::<CanvasGroupComponent>("CanvasGroup", Some(K::CanvasGroup), "canvas_group"),
    kind::<RectMaskComponent>("RectMask", Some(K::RectMask), "rect_mask"),
    kind::<MaskComponent>("Mask", Some(K::Mask), "mask"),
    kind::<BackdropFilterComponent>("BackdropFilter", Some(K::BackdropFilter), "backdrop_filter"),
    kind::<TextComponent>("Text", Some(K::Text), "text"),
    kind::<SelectableComponent>("Selectable", Some(K::Selectable), "selectable"),
    kind::<LayoutGroupComponent>("LayoutGroup", Some(K::LayoutGroup), "layout_group"),
    kind::<LayoutElementComponent>("LayoutElement", Some(K::LayoutElement), "layout_element"),
    kind::<JointComponent>("Joint", Some(K::Joint), "joint"),
    kind::<CharacterControllerComponent>(
        "CharacterController",
        Some(K::CharacterController),
        "character_controller",
    ),
    kind::<LodGroupComponent>("LODGroup", Some(K::LodGroup), "lod_group"),
    kind::<LightProbeProxyVolumeComponent>(
        "LightProbeProxyVolume",
        Some(K::LightProbeProxyVolume),
        "light_probe_proxy_volume",
    ),
    kind::<TrailComponent>("Trail", Some(K::Trail), "trail"),
    kind::<LineComponent>("Line", Some(K::Line), "line"),
    kind::<ShapeComponent>("Shape", Some(K::Shape), "shape"),
];

/// An Animator field's type: its own derive's, else, for the fields it flattens
/// from its `Playback` (which serde reads as a map, typeless), `Playback`'s.
fn animator_field_type(name: &'static str) -> (bool, Option<&'static str>) {
    match field_type::<AnimatorComponent>(name) {
        (false, None) => field_type::<Playback>(name),
        known => known,
    }
}

/// The component `name` names: its own name in any case, else any name
/// `Scene.AddComponent` takes. An unknown name is an error listing the names.
pub fn find(name: &str) -> Result<&'static Kind, String> {
    let added = K::parse(name);
    KINDS
        .iter()
        .find(|k| k.name.eq_ignore_ascii_case(name) || (added.is_some() && k.add == added))
        .ok_or_else(|| {
            let names: Vec<&str> = KINDS.iter().map(|k| k.name).collect();
            format!(
                "unknown component '{name}' (components: {})",
                names.join(", ")
            )
        })
}

/// A UI graphic's shader, whole, through the `UI.SetShader` / `SetShaderParam`
/// ops: `None` clears it; a name keeps the params when it is unchanged, and each
/// param given is then written (checked against the baked shader's layout). A
/// renamed shader drops the old one's params, so those still in the table at
/// their old values (a `Get` table whose `name` alone was edited) are not written.
fn set_ui_shader(slot: &mut Option<UiShader>, shader: Option<UiShader>) -> Result<(), String> {
    let Some(mut shader) = shader else {
        ui_shader::set_shader(slot, None);
        return Ok(());
    };
    if let Some(old) = slot.as_ref().filter(|old| old.name != shader.name) {
        shader
            .params
            .retain(|name, value| old.params.get(name) != Some(value));
    }
    ui_shader::set_shader(slot, Some(shader.name));
    if shader.params.is_empty() {
        return Ok(());
    }
    let layout = ui_shader::layout(slot)?;
    for (name, value) in shader.params {
        ui_shader::set_param(slot, &layout, &name, value)?;
    }
    Ok(())
}
