//! src/api/snapshot/fields.rs — every first-class component's snapshot field.
//!
//! Generated from the registry (`components::registry`): each row's value, keyed by
//! its `Entity` field, is its [`Snapshot`], read off the live entity. A table of
//! `(key, value)` pairs merged into the entity object, not keys of one `json!`
//! literal: that many keys overrun the macro's recursion limit.

use serde_json::Value;

use crate::components::registry::first_class;
use crate::components::FirstClass;
use crate::ecs::World;
use crate::scene::Scene;

type Field = (&'static str, Option<Value>);

/// A first-class component's authoring fields as the snapshot reports them.
/// Implemented beside each value builder (`components.rs`, `ui.rs`, …) with
/// [`snapshot!`]. The Material is the exception: its row reports the library asset
/// it points at, not the reference.
pub(super) trait Snapshot {
    fn snapshot(&self) -> Value;
}

/// Implement [`Snapshot`] for each `Type => value builder`.
macro_rules! snapshot {
    ($($ty:ty => $value:expr,)*) => {
        $(impl $crate::api::snapshot::fields::Snapshot for $ty {
            fn snapshot(&self) -> serde_json::Value {
                ($value)(self)
            }
        })*
    };
}
pub(super) use snapshot;

/// One row's snapshot value on entity `id`.
macro_rules! field_value {
    ($scene:ident, $id:ident, material) => {
        $scene
            .material_asset_of($id)
            .map(super::components::material_value)
    };
    ($scene:ident, $id:ident, $field:ident) => {
        $scene.world.$field($id).map(|c| Snapshot::snapshot(&*c))
    };
}

macro_rules! snapshot_rows {
    ($($field:ident: $ty:ty => $kind:ident [$get_mut:ident $has:ident $($acc:ident)*];)*) => {
        /// Each first-class component's authoring fields, keyed by its snapshot name
        /// (`None` when the entity lacks it, written as `null`).
        pub(super) fn component_fields(scene: &Scene, id: u32) -> Vec<Field> {
            vec![$((stringify!($field), field_value!(scene, id, $field)),)*]
        }

        /// The names of the optional first-class components entity `id` carries,
        /// by their inventory names, in the registry's order.
        pub(super) fn carried(world: &World, id: u32) -> Vec<&'static str> {
            let mut names = Vec::new();
            $(if world.$has(id) {
                names.push(<$ty as FirstClass>::INVENTORY);
            })*
            names
        }
    };
}

first_class!(snapshot_rows);
