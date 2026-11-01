//! src/components/entity/mod.rs — the GameObject document shape.
//!
//! `Entity` is no longer live storage (#345 split it into real per-component
//! hecs columns — see `ecs::world`); it survives as the *document* shape one
//! GameObject assembles into/out of for serialization, prefab diffing, and
//! the editor/inspector's "view the whole GameObject" ergonomics. `transform`
//! is the one mandatory field (Transform is never optional); every other
//! component is `Option<…>`. Originally moved verbatim from the former
//! `core/scene.rs`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::ShapeComponent;
use super::{
    AnimatorComponent, AudioSourceComponent, CameraComponent, CanvasComponent,
    CanvasGroupComponent, ColliderComponent, ImageComponent, LayoutElementComponent,
    LayoutGroupComponent, LightComponent, MaterialAsset, MaterialComponent, MeshComponent,
    NavMeshAgentComponent, ParticleEmitterComponent, PostProcessingComponent, RectMaskComponent,
    RectTransformComponent, RigidBodyComponent, ScriptComponent, SelectableComponent,
    TextComponent, TransformComponent,
};
use super::{
    BackdropFilterComponent, MaskComponent, NavMeshObstacleComponent, OffMeshLinkComponent,
};
use super::{CharacterControllerComponent, JointComponent, LineComponent, LodGroupComponent};
use super::{LightProbeProxyVolumeComponent, NavMeshModifierVolumeComponent};
use super::{ReverbZoneComponent, TrailComponent};

mod override_refs;
mod prefab_link;
mod refs;

pub use override_refs::visit_override_refs;
pub use prefab_link::{PrefabLink, PrefabOverrides, SIZE_TOKEN};
pub(crate) use refs::with_ref_components;
pub use refs::{escape_token, EntityRefs, RefVisit};

// Not sorted (#911): field order is the order a saved scene writes its keys in.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    pub id: u32,
    pub name: String,
    pub active: bool,
    pub is_static: bool,
    /// Index into the project's shared Layers registry (Unity's per-object layer).
    /// Defaults to 0 (`"Default"`). Groundwork for the collision matrix (#91) and
    /// camera culling masks (#92) — it carries no behaviour on its own yet.
    #[serde(default)]
    pub layer: u8,
    pub transform: TransformComponent,
    pub mesh: Option<MeshComponent>,
    /// Reference to a library material by name (the first-class component). The
    /// material DATA is a shared asset in `Scene.materials`; this only points at it.
    pub material: Option<MaterialComponent>,
    /// Transient staging slot: a default library material the editor's Add menu
    /// attached at the World level (which holds no library), to be inserted into the
    /// scene's material library once one is reachable (the inspector, or
    /// `apply_world_data` when a snapshot is restored). Never serialized; always
    /// `None` on a loaded entity.
    #[serde(skip)]
    pub pending_material: Option<MaterialAsset>,
    /// An entity can carry MANY scripts, each its own MonoBehaviour-equivalent Lua
    /// lifecycle table (#83).
    #[serde(default)]
    pub scripts: Vec<ScriptComponent>,
    pub animator: Option<AnimatorComponent>,
    pub light: Option<LightComponent>,
    pub collider: Option<ColliderComponent>,
    pub rigidbody: Option<RigidBodyComponent>,
    pub nav_agent: Option<NavMeshAgentComponent>,
    /// Carves the navmesh or is avoided by agents (#456).
    #[serde(default)]
    pub nav_obstacle: Option<NavMeshObstacleComponent>,
    /// An authored off-mesh link (#462).
    #[serde(default)]
    pub offmesh_link: Option<OffMeshLinkComponent>,
    /// A navigation-area modifier volume (#460).
    #[serde(default)]
    pub nav_modifier: Option<NavMeshModifierVolumeComponent>,
    pub camera: Option<CameraComponent>,
    pub post_processing: Option<PostProcessingComponent>,
    #[serde(default)]
    pub particles: Option<ParticleEmitterComponent>,
    /// Per-entity 2D audio emitter (#212).
    #[serde(default)]
    pub audio: Option<AudioSourceComponent>,
    /// Listener-position reverb volume (#469).
    #[serde(default)]
    pub reverb_zone: Option<ReverbZoneComponent>,
    /// UI root (#417).
    #[serde(default)]
    pub canvas: Option<CanvasComponent>,
    /// 2D placement inside the parent rect (#417), beside the mandatory Transform.
    #[serde(default)]
    pub rect_transform: Option<RectTransformComponent>,
    /// UI graphic (#418).
    #[serde(default)]
    pub image: Option<ImageComponent>,
    /// UI subtree alpha + interaction flags (#418).
    #[serde(default)]
    pub canvas_group: Option<CanvasGroupComponent>,
    /// Clips the UI subtree to this rect (#418).
    #[serde(default)]
    pub rect_mask: Option<RectMaskComponent>,
    /// Clips the UI subtree to this entity's graphic (#428).
    #[serde(default)]
    pub mask: Option<MaskComponent>,
    /// Frosted glass behind this UI graphic (#426).
    #[serde(default)]
    pub backdrop_filter: Option<BackdropFilterComponent>,
    /// UI text label (#419).
    #[serde(default)]
    pub text: Option<TextComponent>,
    /// Interactive UI element (#420).
    #[serde(default)]
    pub selectable: Option<SelectableComponent>,
    /// Arranges the UI children in a row, column or grid (#421).
    #[serde(default)]
    pub layout_group: Option<LayoutGroupComponent>,
    /// Layout sizes and content fitting for a UI element (#421).
    #[serde(default)]
    pub layout_element: Option<LayoutElementComponent>,
    #[serde(default)]
    pub joint: Option<JointComponent>,
    /// Collide-and-slide capsule mover (#451).
    #[serde(default)]
    pub character_controller: Option<CharacterControllerComponent>,
    /// Level-of-detail group over renderer entities (#472).
    #[serde(default)]
    pub lod_group: Option<LodGroupComponent>,
    /// Lights the mesh from a grid of probe samples over its bounds (#1672).
    #[serde(default)]
    pub light_probe_proxy_volume: Option<LightProbeProxyVolumeComponent>,
    /// Ribbon along the entity's recent path (#441).
    #[serde(default)]
    pub trail: Option<TrailComponent>,
    /// Ribbon through authored points (#441).
    #[serde(default)]
    pub line: Option<LineComponent>,
    /// Texture-free SDF UI graphic (#425).
    #[serde(default)]
    pub shape: Option<ShapeComponent>,
    /// Live link back to the source `.prefab` for a *linked* prefab instance (#216).
    /// `None` on a plain entity or a v1 unpacked copy. Carried on every entity of an
    /// instance.
    #[serde(default)]
    pub prefab_link: Option<PrefabLink>,
    pub parent_id: Option<u32>,
    /// Document-only (#453): the bone of the parent's skeleton this entity hangs
    /// from, then that bone's ancestor bones, nearest first. Bones are rebuilt from
    /// the model on load, so a child of `hand_r` is saved under the skinned entity
    /// with `parent_bone: ["hand_r", "forearm_r", …]` and re-parented by name once
    /// the skeleton is back; when the model no longer has `hand_r`, the nearest
    /// ancestor it still has takes it (#1252). Always empty on a live entity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parent_bone: Vec<String>,
    /// Document-only (#1264): each component id leaf (by its pointer, see `refs`)
    /// that names a bone, saved as that bone's name, then its ancestor bones',
    /// nearest first, the way `parent_bone` keeps a bone child. The leaf itself
    /// names the skinned entity, and is re-bound to the bone by name once the
    /// skeleton is rebuilt (`scene::skeleton::bone_refs`). Always empty on a live
    /// entity.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bone_refs: BTreeMap<String, Vec<String>>,
    pub children: Vec<u32>,
}

impl Entity {
    pub fn new(id: u32, name: String) -> Self {
        Self {
            id,
            name,
            active: true,
            is_static: false,
            layer: 0,
            transform: TransformComponent::default(),
            mesh: None,
            material: None,
            pending_material: None,
            scripts: Vec::new(),
            animator: None,
            light: None,
            collider: None,
            rigidbody: None,
            nav_agent: None,
            nav_obstacle: None,
            offmesh_link: None,
            nav_modifier: None,
            camera: None,
            post_processing: None,
            particles: None,
            audio: None,
            reverb_zone: None,
            canvas: None,
            rect_transform: None,
            image: None,
            canvas_group: None,
            rect_mask: None,
            mask: None,
            backdrop_filter: None,
            text: None,
            selectable: None,
            layout_group: None,
            layout_element: None,
            joint: None,
            character_controller: None,
            lod_group: None,
            light_probe_proxy_volume: None,
            trail: None,
            line: None,
            shape: None,
            prefab_link: None,
            parent_id: None,
            parent_bone: Vec::new(),
            bone_refs: BTreeMap::new(),
            children: Vec::new(),
        }
    }
}
