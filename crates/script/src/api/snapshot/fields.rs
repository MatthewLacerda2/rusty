use serde_json::Value;

use rusty_components::registry::first_class;
use rusty_components::FirstClass;
use rusty_ecs::World;
use rusty_scene::Scene;

type Field = (&'static str, Option<Value>);

pub(super) trait Snapshot {
    fn snapshot(&self) -> Value;
}

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

// A table of pairs, not one `json!` literal: that many keys overrun its recursion limit.
macro_rules! snapshot_rows {
    ($($field:ident: $ty:ty => $kind:ident [$get_mut:ident $has:ident $($acc:ident)*];)*) => {
        pub(super) fn component_fields(scene: &Scene, id: u32) -> Vec<Field> {
            vec![$((stringify!($field), field_value!(scene, id, $field)),)*]
        }

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
