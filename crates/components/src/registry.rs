//! A first-class component is its type, its [`FirstClass`] impl and one `first_class!` row, from
//! which every per-component list is generated (`Entity`'s fields, `World`'s accessors).
//! A new one also brings a `Default` (or an arm in `dispatch.rs`'s `added!`), its inspector card,
//! API namespace and `docs/api/` page, and a new `scene.fingerprint`.

pub trait FirstClass: 'static {
    const NAME: &'static str;
    const ALIASES: &'static [&'static str] = &[];
    const INVENTORY: &'static str = Self::NAME;
    /// Key paths whose absence loads other than the default on purpose, each with why.
    const DEFAULT_EXCEPTIONS: &'static [(&'static str, &'static str)] = &[];
}

// A row: the `Entity` field (also scene key and getter), its type, its `ComponentKind`, then the
// accessor names, since a macro cannot build an identifier.
#[macro_export]
macro_rules! first_class {
    ($callback:ident) => {
        $crate::first_class! { @rows $callback
            // sorted: one row per component, so two branches adding one touch different lines
            ai_perception: ai::perception::AIPerceptionComponent => AIPerception [ai_perception_mut has_ai_perception set_ai_perception take_ai_perception ids_with_ai_perception];
            animator: animator::AnimatorComponent => Animator [animator_mut has_animator set_animator take_animator ids_with_animator];
            audio: audio::source::AudioSourceComponent => Audio [audio_mut has_audio set_audio take_audio ids_with_audio];
            backdrop_filter: ui::backdrop::BackdropFilterComponent => BackdropFilter [backdrop_filter_mut has_backdrop_filter set_backdrop_filter take_backdrop_filter ids_with_backdrop_filter];
            camera: rendering::camera::CameraComponent => Camera [camera_mut has_camera set_camera take_camera ids_with_camera];
            canvas: ui::layout::canvas::CanvasComponent => Canvas [canvas_mut has_canvas set_canvas take_canvas ids_with_canvas];
            canvas_group: ui::layout::canvas_group::CanvasGroupComponent => CanvasGroup [canvas_group_mut has_canvas_group set_canvas_group take_canvas_group ids_with_canvas_group];
            character_controller: physics::character_controller::CharacterControllerComponent => CharacterController [character_controller_mut has_character_controller set_character_controller take_character_controller ids_with_character_controller];
            collider: physics::collider::ColliderComponent => Collider [collider_mut has_collider set_collider take_collider ids_with_collider];
            image: ui::image::ImageComponent => Image [image_mut has_image set_image take_image ids_with_image];
            joint: physics::joint::JointComponent => Joint [joint_mut has_joint set_joint take_joint ids_with_joint];
            layout_element: ui::layout::element::LayoutElementComponent => LayoutElement [layout_element_mut has_layout_element set_layout_element take_layout_element ids_with_layout_element];
            layout_group: ui::layout::group::LayoutGroupComponent => LayoutGroup [layout_group_mut has_layout_group set_layout_group take_layout_group ids_with_layout_group];
            light: rendering::light::LightComponent => Light [light_mut has_light set_light take_light ids_with_light];
            light_probe_proxy_volume: rendering::light_probe_proxy_volume::LightProbeProxyVolumeComponent => LightProbeProxyVolume [light_probe_proxy_volume_mut has_light_probe_proxy_volume set_light_probe_proxy_volume take_light_probe_proxy_volume ids_with_light_probe_proxy_volume];
            line: ribbon::LineComponent => Line [line_mut has_line set_line take_line ids_with_line];
            lod_group: rendering::lod_group::LodGroupComponent => LodGroup [lod_group_mut has_lod_group set_lod_group take_lod_group ids_with_lod_group];
            mask: ui::mask::MaskComponent => Mask [mask_mut has_mask set_mask take_mask ids_with_mask];
            material: material::MaterialComponent => Texture [material_mut has_material set_material take_material ids_with_material];
            mesh: mesh::MeshComponent => Mesh [mesh_mut has_mesh set_mesh take_mesh ids_with_mesh];
            nav_agent: nav::agent::NavMeshAgentComponent => NavMeshAgent [nav_agent_mut has_nav_agent set_nav_agent take_nav_agent ids_with_nav_agent];
            nav_mesh_modifier: nav::object_modifier::NavMeshModifierComponent => NavMeshModifier [nav_mesh_modifier_mut has_nav_mesh_modifier set_nav_mesh_modifier take_nav_mesh_modifier ids_with_nav_mesh_modifier];
            nav_modifier: nav::modifier::NavMeshModifierVolumeComponent => NavMeshModifierVolume [nav_modifier_mut has_nav_modifier set_nav_modifier take_nav_modifier ids_with_nav_modifier];
            nav_obstacle: nav::obstacle::NavMeshObstacleComponent => NavMeshObstacle [nav_obstacle_mut has_nav_obstacle set_nav_obstacle take_nav_obstacle ids_with_nav_obstacle];
            offmesh_link: nav::offmesh_link::OffMeshLinkComponent => OffMeshLink [offmesh_link_mut has_offmesh_link set_offmesh_link take_offmesh_link ids_with_offmesh_link];
            particles: particle::ParticleEmitterComponent => Particles [particles_mut has_particles set_particles take_particles ids_with_particles];
            post_processing: rendering::post_processing::PostProcessingComponent => PostProcessing [post_processing_mut has_post_processing set_post_processing take_post_processing ids_with_post_processing];
            rect_mask: ui::rect_mask::RectMaskComponent => RectMask [rect_mask_mut has_rect_mask set_rect_mask take_rect_mask ids_with_rect_mask];
            rect_transform: ui::layout::rect_transform::RectTransformComponent => RectTransform [rect_transform_mut has_rect_transform set_rect_transform take_rect_transform ids_with_rect_transform];
            reverb_zone: audio::reverb_zone::ReverbZoneComponent => ReverbZone [reverb_zone_mut has_reverb_zone set_reverb_zone take_reverb_zone ids_with_reverb_zone];
            rigidbody: physics::rigidbody::RigidBodyComponent => RigidBody [rigidbody_mut has_rigidbody set_rigidbody take_rigidbody ids_with_rigidbody];
            selectable: ui::selectable::SelectableComponent => Selectable [selectable_mut has_selectable set_selectable take_selectable ids_with_selectable];
            shape: ui::shape::ShapeComponent => Shape [shape_mut has_shape set_shape take_shape ids_with_shape];
            text: ui::text::TextComponent => Text [text_mut has_text set_text take_text ids_with_text];
            trail: ribbon::TrailComponent => Trail [trail_mut has_trail set_trail take_trail ids_with_trail];
        }
    };
    (@rows $callback:ident $($field:ident: $($seg:ident)::+ => $kind:ident [$($acc:ident)*];)*) => {
        $callback! { $($field: $crate::$($seg)::+ => $kind [$($acc)*];)* }
    };
}
pub use crate::first_class;

/// Only this crate calls it.
#[doc(hidden)]
pub fn serde_drift<T: serde::Serialize + serde::de::DeserializeOwned>(
    reference: &T,
) -> Vec<(String, serde_json::Value)> {
    let Ok(whole) = serde_json::to_value(reference) else {
        return vec![("(does not serialise)".to_string(), serde_json::Value::Null)];
    };
    let mut paths = Vec::new();
    key_paths(&whole, &mut Vec::new(), &mut paths);
    paths
        .iter()
        .filter_map(|path| {
            let loaded = serde_json::from_value::<T>(without(&whole, path)).ok()?;
            let loaded = serde_json::to_value(loaded).ok()?;
            (loaded != whole).then(|| (path.join("."), loaded))
        })
        .collect()
}

/// Test hook for `scene::create::defaults`' tests.
#[doc(hidden)]
pub fn default_drift<T: FirstClass + serde::Serialize + serde::de::DeserializeOwned>(
    reference: &T,
) -> Vec<String> {
    let drift = serde_drift(reference);
    let excepted = |path: &str| T::DEFAULT_EXCEPTIONS.iter().any(|(p, _)| *p == path);
    let drifted = |path: &str| drift.iter().any(|(p, _)| p == path);
    let unexplained = drift
        .iter()
        .filter(|(path, _)| !excepted(path))
        .map(|(path, loaded)| format!("{}: {path} (loads {loaded})", T::NAME));
    let stale = T::DEFAULT_EXCEPTIONS
        .iter()
        .filter(|(path, _)| !drifted(path))
        .map(|(path, _)| format!("{}: {path} is excepted but loads its default", T::NAME));
    unexplained.chain(stale).collect()
}

fn key_paths(value: &serde_json::Value, prefix: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    if let serde_json::Value::Object(map) = value {
        for (key, child) in map {
            prefix.push(key.clone());
            out.push(prefix.clone());
            key_paths(child, prefix, out);
            prefix.pop();
        }
    }
}

fn without(value: &serde_json::Value, path: &[String]) -> serde_json::Value {
    let mut value = value.clone();
    if let Some((last, parents)) = path.split_last() {
        let parent = parents
            .iter()
            .try_fold(&mut value, |at, key| at.get_mut(key));
        if let Some(map) = parent.and_then(serde_json::Value::as_object_mut) {
            map.remove(last);
        }
    }
    value
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
