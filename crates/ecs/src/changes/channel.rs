use rusty_components::registry::first_class;
use rusty_components::{MaterialAsset, PrefabLink, ScriptComponent, TransformComponent};

use super::super::bundle::Core;

mod sealed {
    pub trait Sealed {}
}

pub trait Channel: sealed::Sealed + Send + Sync + 'static {
    const COLUMN: bool;
    const NAME: &'static str;
    const SLOT: usize;
    const STATIC_GEOMETRY: bool = false;
}

pub struct StaticSet;

pub struct WorldPose;

pub struct Activity;

pub struct Layer;

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
