//! crates/ecs/src/changes/channel.rs — what may be read through a cursor: a sealed [`Channel`]
//! trait, implemented for every real column type and for the derived markers, so a
//! type that is neither (`ScriptComponent`, whose column is `Vec<ScriptComponent>`)
//! does not compile, and a new marker is a column only by saying so.

use rusty_components::registry::first_class;
use rusty_components::{MaterialAsset, PrefabLink, ScriptComponent, TransformComponent};

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
    ($column:literal: $($ty:ty => $name:expr,)*) => {
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
    PrefabLink => "prefab_link",
}

/// Each registry row's column is a channel named after its `Entity` field.
macro_rules! registry_channels {
    ($($field:ident: $ty:ty => $kind:ident [$($acc:ident)*];)*) => {
        channels! { true: $($ty => stringify!($field),)* }
    };
}

first_class!(registry_channels);
