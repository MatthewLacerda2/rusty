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
pub trait Channel: sealed::Sealed + Send + Sync + 'static {
    /// A component column: stamped by every write to it and by every whole-entity
    /// write (placed, overwritten, removed) of an entity that carries or carried it.
    /// A derived channel (`false`) is stamped only where the code that knows its
    /// fact says so.
    const COLUMN: bool;
    /// Its name in [`super::World::change_stats`] and `Debug.Stats` (`changes_<name>`).
    const NAME: &'static str;
    /// Its own place in the World's table of logs, fixed at compile time and unique:
    /// an enum's discriminant, the fixed channels' then the registry's after them
    /// (`fixed_channels!`), so a write finds its column's log by index.
    const SLOT: usize;
    /// A column whose write on a static entity moves
    /// [`super::World::static_geometry`]'s revision (the Collider's).
    const STATIC_GEOMETRY: bool = false;
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

/// The derived channel of child order: an id whose `children` list changed (a child
/// added or removed, so a re-parent stamps both parents, and a re-parent under the
/// same parent, which moves the child to the end, stamps that parent), or that was
/// placed, overwritten or removed. What the UI layout's sibling order is read from.
pub struct Children;

macro_rules! channels {
    ($column:literal: $($ty:ty => $name:expr, $slot:expr $(, geometry: $geometry:expr)?;)*) => {
        $(
            impl sealed::Sealed for $ty {}
            impl Channel for $ty {
                const COLUMN: bool = $column;
                const NAME: &'static str = $name;
                const SLOT: usize = $slot;
                $(const STATIC_GEOMETRY: bool = $geometry;)?
            }
        )*
    };
}

/// The channels outside the registry, each listed once: its slot is its
/// `FixedSlot` discriminant, and `FIXED_SLOTS` (where the registry's start) is
/// their count, so a new one cannot take another's slot or a registry column's.
macro_rules! fixed_channels {
    ($($column:literal $ty:ty => $name:ident;)*) => {
        #[expect(non_camel_case_types, reason = "each variant is its channel's `NAME`")]
        enum FixedSlot { $($name,)* Count }
        $(channels! { $column: $ty => stringify!($name), FixedSlot::$name as usize; })*
        /// The slots before the registry's columns.
        const FIXED_SLOTS: usize = FixedSlot::Count as usize;
    };
}

fixed_channels! {
    false StaticSet => static_set;
    false WorldPose => world_pose;
    false Activity => activity;
    false Layer => layer;
    false Children => children;
    true Core => core;
    true String => name;
    true TransformComponent => transform;
    true Vec<ScriptComponent> => scripts;
    true MaterialAsset => pending_material;
    true PrefabLink => prefab_link;
}

/// The registry columns that are static geometry, by `Entity` field: a new one is
/// one more arm.
macro_rules! static_geometry {
    (collider) => {
        true
    };
    ($field:ident) => {
        false
    };
}

/// Each registry row's column is a channel named after its `Entity` field, its slot
/// after the fixed ones in the registry's order.
macro_rules! registry_channels {
    ($($field:ident: $ty:ty => $kind:ident [$($acc:ident)*];)*) => {
        enum RegistrySlot { $($field,)* }
        channels! { true: $($ty => stringify!($field), FIXED_SLOTS + RegistrySlot::$field as usize,
            geometry: static_geometry!($field);)* }
    };
}

first_class!(registry_channels);
