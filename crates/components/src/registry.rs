//! crates/components/src/registry.rs — the first-class component registry (#2023).
//!
//! A first-class component is declared once: its type, beside which it implements
//! [`FirstClass`] (the names scripts and the snapshot know it by), and one row of
//! `first_class!` here. Every list the engine keeps per component is generated
//! from that row, by a consumer macro handed to `first_class!`, the way
//! `entity::with_ref_components` feeds its own lists:
//!
//! - `Entity`'s fields and `Entity::new` (`components::entity`);
//! - the accessor family on `World` and the hecs bundle in and out (`ecs::access`,
//!   `ecs::bundle`), and the change channels (`ecs::changes::channel`);
//! - `ComponentKind`, its name parsing, and the dependency verbs' dispatch
//!   (`scene::authoring::components`, `scene::authoring::dependency::dispatch`);
//! - `Component.*`'s `KINDS` table (`scene::authoring::fields::kinds`);
//! - the snapshot's inventory and fields (`api::snapshot`);
//! - the Add Component menu (`editor::inspector::components::add`).
//!
//! What stays hand-written is each component's own: its inspector card (and its
//! menu entry, `AddEntry`, beside it), its API namespace, its field table, its
//! snapshot value and its systems.
//!
//! ## Adding a component
//!
//! 1. Its type, in its category's folder (`pub mod` in that folder's `mod.rs`),
//!    implementing [`FirstClass`] and, as its Add Component default, `Default`
//!    (or an arm in `dispatch.rs`'s `added!` naming another default).
//! 2. Its row below. The compiler then asks for the three traits the generated
//!    lists need, each pointing at this file: `Inspect` (an `inspect!` row in
//!    `scene::authoring::fields::kinds::<area>`), `Snapshot` (a `snapshot!` row in
//!    `api::snapshot::<area>`) and `AddEntry` (beside its inspector card).
//! 3. Its inspector card and API namespace, which the component gate
//!    (`tools/lint -- --components`) asks for; its `docs/api/` page; and the
//!    scene format's fingerprint (`tests/scene_format/scene.fingerprint`), which a
//!    new stored field changes.
//!
//! A row reads `field: path::to::Type => Kind [get_mut has set take ids_with];`:
//! the `Entity` field (also the scene key, the change channel's name and the
//! getter), the type under `crate::components`, the `ComponentKind` variant, and
//! the rest of its accessor family (macros cannot build an identifier, so the row
//! names them). A consumer receives each row with the type's path made absolute.
//! The rows are sorted, so two branches adding a component touch different lines;
//! their order is `Entity`'s field order, the order a saved scene writes its keys.
//! `Transform` is mandatory and not a row; neither are the engine-only fields (id,
//! name, flags, scripts, the prefab link, the hierarchy).

/// What scripts, the scene and the snapshot call one first-class component.
/// Implemented beside the component's type.
pub trait FirstClass: 'static {
    /// The name `Component.*` answers to first, and the one `Scene.AddComponent`'s
    /// error lists.
    const NAME: &'static str;
    /// The other names `Scene.AddComponent` takes, lowercase. [`FirstClass::NAME`]
    /// and the `ComponentKind` variant's own name are always taken (any case).
    const ALIASES: &'static [&'static str] = &[];
    /// The name in the snapshot's component inventory (Unity's class name where it
    /// differs, `Rigidbody`, `TrailRenderer`).
    const INVENTORY: &'static str = Self::NAME;
}

/// Calls `$callback!` with every first-class component row (see the module doc).
/// Exported at the crate's root and re-exported here, so callers name it
/// `components::registry::first_class`.
#[macro_export]
macro_rules! first_class {
    ($callback:ident) => {
        $crate::first_class! { @rows $callback
            // sorted: one row per component, so two branches adding one touch different lines (#2023)
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
