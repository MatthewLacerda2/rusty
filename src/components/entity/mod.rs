//! src/components/entity.rs — the GameObject document shape.
//!
//! `Entity` is no longer live storage (#345 split it into real per-component
//! hecs columns — see `ecs::world`); it survives as the *document* shape one
//! GameObject assembles into/out of for serialization, prefab diffing, and
//! the editor/inspector's "view the whole GameObject" ergonomics. `transform`
//! is the one mandatory field (Transform is never optional); every other
//! component is `Option<…>`. Originally moved verbatim from the legacy
//! `core/scene.rs`.

use serde::{Deserialize, Serialize};

mod repr;

use super::ShapeComponent;
use super::{
    AnimatorComponent, AudioSourceComponent, CameraComponent, CanvasComponent,
    CanvasGroupComponent, ColliderComponent, ImageComponent, LayoutElementComponent,
    LayoutGroupComponent, LightComponent, MaterialAsset, MaterialComponent, MeshComponent,
    NavMeshAgentComponent, ParticleEmitterComponent, RectMaskComponent, RectTransformComponent,
    RigidBodyComponent, ScriptComponent, SelectableComponent, TextComponent, TransformComponent,
    VisualCorrectionComponent,
};
use super::{BackdropFilterComponent, MaskComponent};
use super::{CharacterControllerComponent, JointComponent, LineComponent, LodGroupComponent};
use super::{SubEmitters, TrailComponent};

/// The live link from an instance entity back to the `.prefab` it was stamped from
/// (#216). Set on EVERY entity of a linked instance — not just the root — so that
/// reordering the source's entities can never scramble which source entity an
/// instance entity corresponds to: the correlation is the explicit `local_id` (the
/// source's own 0-based local id), independent of order. The root anchors the
/// `source` path; non-root entities carry the same path so each can be matched on
/// its own. Absent (`None`) on a plain entity or a v1 *unpacked* copy, which has no
/// link back to the asset.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PrefabLink {
    /// Path to the source `.prefab` asset (the same string for every entity of one
    /// instance).
    pub source: String,
    /// The corresponding source entity's LOCAL id (0-based; root = 0). This is the
    /// stable correlation key against `PrefabData::entities`, order-independent.
    pub local_id: u32,
    /// This entity's recorded per-instance overrides: a generic field diff against a
    /// fresh copy of the source entity, as `json-pointer-path -> value` (e.g.
    /// `"/transform/position/0" -> 5.0`). It auto-covers every present/future
    /// first-class component with no per-component code, and covers component
    /// add/remove too (a `None`↔`Some` is just a diff at that component's path).
    /// Propagation rebuilds the entity from the fresh source baseline, then re-applies
    /// these paths on top, so non-overridden fields track the source while overridden
    /// ones are preserved. `#[serde(default)]` so a link saved before overrides
    /// existed loads with none.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub overrides: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(from = "repr::EntityRepr")]
pub struct Entity {
    pub id: u32,
    pub name: String,
    pub active: bool,
    pub is_static: bool,
    /// Index into the project's shared Layers registry (Unity's per-object layer).
    /// Defaults to 0 (`"Default"`); `#[serde(default)]` so pre-#90 scenes load.
    /// Groundwork for the collision matrix (#91) and camera culling masks (#92) —
    /// it carries no behaviour on its own yet.
    #[serde(default)]
    pub layer: u8,
    pub transform: TransformComponent,
    pub mesh: Option<MeshComponent>,
    /// Reference to a library material by name (the first-class component). The
    /// material DATA is a shared asset in `Scene.materials`; this only points at it.
    pub material: Option<MaterialComponent>,
    /// Transient migration carrier: a legacy inline material lifted out of an old
    /// scene by `From<EntityRepr>`, to be inserted into the scene's material library
    /// by `apply_scene_data` (which is where the library is reachable). Never
    /// serialized; always `None` on a freshly-built/loaded runtime entity.
    #[serde(skip)]
    pub pending_material: Option<MaterialAsset>,
    /// An entity can carry MANY scripts, each its own MonoBehaviour-equivalent
    /// Lua lifecycle table (#83). `#[serde(default)]` plus the legacy `script`
    /// field in `EntityRepr` keep pre-#83 single-`script` scenes loadable.
    #[serde(default)]
    pub scripts: Vec<ScriptComponent>,
    pub animator: Option<AnimatorComponent>,
    pub light: Option<LightComponent>,
    pub collider: Option<ColliderComponent>,
    pub rigidbody: Option<RigidBodyComponent>,
    pub nav_agent: Option<NavMeshAgentComponent>,
    pub camera: Option<CameraComponent>,
    pub visual_correction: Option<VisualCorrectionComponent>,
    #[serde(default)]
    pub particles: Option<ParticleEmitterComponent>,
    /// Per-entity 2D audio emitter (#212). `#[serde(default)]` so pre-#212 scenes
    /// load with no audio source.
    #[serde(default)]
    pub audio: Option<AudioSourceComponent>,
    /// UI root (#417). `#[serde(default)]` so pre-#417 scenes load with no canvas.
    #[serde(default)]
    pub canvas: Option<CanvasComponent>,
    /// 2D placement inside the parent rect (#417), beside the mandatory Transform.
    /// `#[serde(default)]` so pre-#417 scenes load without one.
    #[serde(default)]
    pub rect_transform: Option<RectTransformComponent>,
    /// UI graphic (#418). `#[serde(default)]` so pre-#418 scenes load without one.
    #[serde(default)]
    pub image: Option<ImageComponent>,
    /// UI subtree alpha + interaction flags (#418). `#[serde(default)]` for pre-#418
    /// scenes.
    #[serde(default)]
    pub canvas_group: Option<CanvasGroupComponent>,
    /// Clips the UI subtree to this rect (#418). `#[serde(default)]` for pre-#418
    /// scenes.
    #[serde(default)]
    pub rect_mask: Option<RectMaskComponent>,
    /// Clips the UI subtree to this entity's graphic (#428). `#[serde(default)]` for
    /// pre-#428 scenes.
    #[serde(default)]
    pub mask: Option<MaskComponent>,
    /// Frosted glass behind this UI graphic (#426). `#[serde(default)]` for pre-#426
    /// scenes.
    #[serde(default)]
    pub backdrop_filter: Option<BackdropFilterComponent>,
    /// UI text label (#419). `#[serde(default)]` for pre-#419 scenes.
    #[serde(default)]
    pub text: Option<TextComponent>,
    /// Interactive UI element (#420). `#[serde(default)]` for pre-#420 scenes.
    #[serde(default)]
    pub selectable: Option<SelectableComponent>,
    /// Arranges the UI children in a row, column or grid (#421). `#[serde(default)]`
    /// for pre-#421 scenes.
    #[serde(default)]
    pub layout_group: Option<LayoutGroupComponent>,
    /// Layout sizes and content fitting for a UI element (#421). `#[serde(default)]`
    /// for pre-#421 scenes.
    #[serde(default)]
    pub layout_element: Option<LayoutElementComponent>,
    #[serde(default)]
    pub joint: Option<JointComponent>,
    /// Collide-and-slide capsule mover (#451). `#[serde(default)]` for pre-#451
    /// scenes.
    #[serde(default)]
    pub character_controller: Option<CharacterControllerComponent>,
    /// Level-of-detail group over renderer entities (#472). `#[serde(default)]` for
    /// pre-#472 scenes.
    #[serde(default)]
    pub lod_group: Option<LodGroupComponent>,
    /// Ribbon along the entity's recent path (#441). `#[serde(default)]` for
    /// pre-#441 scenes.
    #[serde(default)]
    pub trail: Option<TrailComponent>,
    /// Ribbon through authored points (#441). `#[serde(default)]` for pre-#441
    /// scenes.
    #[serde(default)]
    pub line: Option<LineComponent>,
    /// Texture-free SDF UI graphic (#425). `#[serde(default)]` for pre-#425 scenes.
    #[serde(default)]
    pub shape: Option<ShapeComponent>,
    /// Live link back to the source `.prefab` for a *linked* prefab instance (#216).
    /// `None` on a plain entity or a v1 unpacked copy. Carried on every entity of an
    /// instance. `#[serde(default)]` so pre-#216 scenes load with no link.
    #[serde(default)]
    pub prefab_link: Option<PrefabLink>,
    pub parent_id: Option<u32>,
    /// Document-only (#453): the bone of the parent's skeleton this entity hangs
    /// from. Bones are rebuilt from the model on load, so a child of `hand_r` is
    /// saved under the skinned entity with `parent_bone: "hand_r"` and re-parented
    /// by name once the skeleton is back. Always `None` on a live entity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_bone: Option<String>,
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
            camera: None,
            visual_correction: None,
            particles: None,
            audio: None,
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
            trail: None,
            line: None,
            shape: None,
            prefab_link: None,
            parent_id: None,
            parent_bone: None,
            children: Vec::new(),
        }
    }

    /// Rewrite every entity reference a component holds (a Selectable's targets,
    /// #420; a Joint's connected body, #449; an emitter's sub-emitters, #439; an LODGroup's
    /// renderers, #472; a marker's target, #429) through `map` — how the references
    /// follow the entity when a prefab is saved, stamped or propagated.
    pub fn remap_refs(&mut self, map: &dyn Fn(u32) -> Option<u32>) {
        self.selectable.iter_mut().for_each(|s| s.remap_refs(map));
        self.joint.iter_mut().for_each(|j| j.remap_refs(map));
        self.particles.iter_mut().for_each(|p| p.remap_refs(map));
        self.lod_group.iter_mut().for_each(|g| g.remap_refs(map));
        self.rect_transform
            .iter_mut()
            .for_each(|r| r.remap_refs(map));
    }

    /// Whether `pointer` (a JSON pointer into the entity document, as prefab
    /// overrides key their leaves) names one of those entity references.
    pub fn is_ref_pointer(pointer: &str) -> bool {
        let under = |prefix: &str, refs: &[&str]| {
            pointer
                .strip_prefix(prefix)
                .is_some_and(|rest| refs.contains(&rest))
        };
        under("/selectable", &SelectableComponent::REF_POINTERS)
            || under("/joint", &JointComponent::REF_POINTERS)
            || under("/particles", &SubEmitters::REF_POINTERS)
            || pointer
                .strip_prefix("/lod_group")
                .is_some_and(LodGroupComponent::is_ref_pointer)
            || under("/rect_transform", &RectTransformComponent::REF_POINTERS)
    }
}
