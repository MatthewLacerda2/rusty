//! src/ecs/changes/channel.rs — what may be read through a cursor: a sealed [`Channel`]
//! trait, implemented for every real column type and for the derived markers, so a
//! type that is neither (`ScriptComponent`, whose column is `Vec<ScriptComponent>`)
//! does not compile, and a new marker is a column only by saying so.

use crate::components::AIPerceptionComponent;
use crate::components::{
    AnimatorComponent, AudioSourceComponent, BackdropFilterComponent, CameraComponent,
    CanvasComponent, CanvasGroupComponent, CharacterControllerComponent, ColliderComponent,
    ImageComponent, JointComponent, LayoutElementComponent, LayoutGroupComponent, LightComponent,
    LightProbeProxyVolumeComponent, LineComponent, LodGroupComponent, MaskComponent, MaterialAsset,
    MaterialComponent, MeshComponent, NavMeshAgentComponent, NavMeshModifierComponent,
    NavMeshModifierVolumeComponent, NavMeshObstacleComponent, OffMeshLinkComponent,
    ParticleEmitterComponent, PostProcessingComponent, PrefabLink, RectMaskComponent,
    RectTransformComponent, ReverbZoneComponent, RigidBodyComponent, ScriptComponent,
    SelectableComponent, ShapeComponent, TextComponent, TrailComponent, TransformComponent,
};

use super::super::bundle::Core;

mod sealed {
    pub trait Sealed {}
}

/// A channel a [`super::Cursor`] reads.
pub trait Channel: sealed::Sealed + 'static {
    /// A component column: stamped by every write to it and by every whole-entity
    /// write (placed, overwritten, removed). A derived channel (`false`) is stamped
    /// only where the code that knows its fact says so.
    const COLUMN: bool;
    /// Its name in [`super::World::change_stats`] and `Debug.Stats` (`changes_<name>`).
    const NAME: &'static str;
}

/// The derived channel stamped wherever the set of static entities active in the
/// hierarchy may have changed, with the entities whose membership may have moved:
/// a flag flipped, a static entity (or an ancestor of one) activated, deactivated
/// or re-parented, a static entity placed or removed. Its revision is
/// [`super::World::static_revision`].
pub struct StaticSet;

/// The derived channel of world poses: every id whose world matrix may have changed,
/// descendants included (its own or an ancestor's Transform landed a new value, it was
/// re-parented, placed, overwritten or removed). What physics re-poses a body from.
pub struct WorldPose;

/// The derived channel of `activeInHierarchy`: every id whose answer may have changed,
/// descendants included (an `active` flag flipped on it or above it, it or an ancestor
/// re-parented, it placed, overwritten or removed).
pub struct Activity;

/// The derived channel of layers: an id whose layer changed, or that was placed,
/// overwritten or removed.
pub struct Layer;

macro_rules! channels {
    ($column:literal: $($ty:ty => $name:literal,)*) => {
        $(
            impl sealed::Sealed for $ty {}
            impl Channel for $ty {
                const COLUMN: bool = $column;
                const NAME: &'static str = $name;
            }
        )*
    };
}

channels! { false:
    StaticSet => "static_set",
    WorldPose => "world_pose",
    Activity => "activity",
    Layer => "layer",
}

channels! { true:
    Core => "core",
    String => "name",
    TransformComponent => "transform",
    Vec<ScriptComponent> => "scripts",
    MaterialAsset => "pending_material",
    MeshComponent => "mesh",
    MaterialComponent => "material",
    AnimatorComponent => "animator",
    LightComponent => "light",
    ColliderComponent => "collider",
    RigidBodyComponent => "rigidbody",
    NavMeshAgentComponent => "nav_agent",
    NavMeshObstacleComponent => "nav_obstacle",
    OffMeshLinkComponent => "offmesh_link",
    NavMeshModifierVolumeComponent => "nav_modifier",
    NavMeshModifierComponent => "nav_mesh_modifier",
    AIPerceptionComponent => "ai_perception",
    CameraComponent => "camera",
    PostProcessingComponent => "post_processing",
    ParticleEmitterComponent => "particles",
    AudioSourceComponent => "audio",
    ReverbZoneComponent => "reverb_zone",
    CanvasComponent => "canvas",
    RectTransformComponent => "rect_transform",
    ImageComponent => "image",
    CanvasGroupComponent => "canvas_group",
    RectMaskComponent => "rect_mask",
    MaskComponent => "mask",
    BackdropFilterComponent => "backdrop_filter",
    TextComponent => "text",
    SelectableComponent => "selectable",
    LayoutGroupComponent => "layout_group",
    LayoutElementComponent => "layout_element",
    JointComponent => "joint",
    CharacterControllerComponent => "character_controller",
    LodGroupComponent => "lod_group",
    LightProbeProxyVolumeComponent => "light_probe_proxy_volume",
    TrailComponent => "trail",
    LineComponent => "line",
    ShapeComponent => "shape",
    PrefabLink => "prefab_link",
}
