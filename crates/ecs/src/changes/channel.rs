//! Sealed: only real column types and the derived markers are channels, so `ScriptComponent`
//! (whose column is `Vec<ScriptComponent>`) does not compile.

use rusty_components::registry::first_class;
use rusty_components::{MaterialAsset, PrefabLink, ScriptComponent, TransformComponent};

use super::super::bundle::Core;

mod sealed {
    pub trait Sealed {}
}

pub trait Channel: sealed::Sealed + Send + Sync + 'static {
    /// `false`: derived, stamped only where the code that knows its fact says so.
    const COLUMN: bool;
    const NAME: &'static str;
    const SLOT: usize;
    /// A write on a static entity moves `World::static_geometry`'s revision (the Collider's).
    const STATIC_GEOMETRY: bool = false;
}

/// Static entities whose membership of the active static set may have moved (`static_revision`).
pub struct StaticSet;

/// Ids whose world matrix may have changed, descendants included: what physics re-poses from.
pub struct WorldPose;

/// Ids whose `activeInHierarchy` may have changed, descendants included.
pub struct Activity;

pub struct Layer;

/// Ids whose own `children` list changed (both parents of a re-parent): UI sibling order.
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

macro_rules! fixed_channels {
    ($($column:literal $ty:ty => $name:ident;)*) => {
        #[expect(non_camel_case_types, reason = "each variant is its channel's `NAME`")]
        enum FixedSlot { $($name,)* Count }
        $(channels! { $column: $ty => stringify!($name), FixedSlot::$name as usize; })*
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

macro_rules! static_geometry {
    (collider) => {
        true
    };
    ($field:ident) => {
        false
    };
}

macro_rules! registry_channels {
    ($($field:ident: $ty:ty => $kind:ident [$($acc:ident)*];)*) => {
        enum RegistrySlot { $($field,)* }
        channels! { true: $($ty => stringify!($field), FIXED_SLOTS + RegistrySlot::$field as usize,
            geometry: static_geometry!($field);)* }
    };
}

first_class!(registry_channels);
