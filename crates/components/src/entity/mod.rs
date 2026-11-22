//! A GameObject's document shape, which its hecs columns assemble into for a save, a prefab
//! diff and the inspector; not live storage.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::registry::first_class;
use super::{MaterialAsset, ScriptComponent, TransformComponent};

mod override_refs;
mod prefab_link;
mod refs;

pub use override_refs::visit_override_refs;
pub use prefab_link::{PrefabLink, PrefabOverrides, PART_KEY, SIZE_TOKEN};
pub use refs::{escape_token, EntityRefs, RefVisit};

// Field order is the key order a saved scene writes.
macro_rules! entity {
    ($($field:ident: $ty:ty => $kind:ident [$($acc:ident)*];)*) => {
        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct Entity {
            pub id: u32,
            pub name: String,
            pub active: bool,
            pub is_static: bool,
            #[serde(default)]
            pub layer: u8,
            pub transform: TransformComponent,
            /// A default material the Add menu staged until the scene's library is reachable;
            /// never saved.
            #[serde(skip)]
            pub pending_material: Option<MaterialAsset>,
            #[serde(default)]
            pub scripts: Vec<ScriptComponent>,
            $(
                #[doc = concat!("The `", stringify!($kind), "` component (`registry`).")]
                #[serde(default)]
                pub $field: Option<$ty>,
            )*
            #[serde(default)]
            pub prefab_link: Option<PrefabLink>,
            pub parent_id: Option<u32>,
            /// Document-only: the parent's bone it hangs from, then that bone's ancestors, nearest
            /// first, re-bound by name on load; empty on a live entity.
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub parent_bone: Vec<String>,
            /// Document-only: each id leaf naming a bone, kept by bone names as `parent_bone` is.
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
                    pending_material: None,
                    scripts: Vec::new(),
                    $($field: None,)*
                    prefab_link: None,
                    parent_id: None,
                    parent_bone: Vec::new(),
                    bone_refs: BTreeMap::new(),
                    children: Vec::new(),
                }
            }
        }
    };
}
first_class!(entity);
