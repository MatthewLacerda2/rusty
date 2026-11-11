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

use super::registry::first_class;
use super::{MaterialAsset, ScriptComponent, TransformComponent};

mod override_refs;
mod prefab_link;
mod refs;

pub use override_refs::visit_override_refs;
pub use prefab_link::{PrefabLink, PrefabOverrides, PART_KEY, SIZE_TOKEN};
pub(crate) use refs::with_ref_components;
pub use refs::{escape_token, EntityRefs, RefVisit};

/// Declares `Entity` with one `Option<…>` field per registry row, between the
/// engine-only fields (`registry`'s module doc): the order a saved scene writes its
/// keys in.
macro_rules! entity {
    ($($field:ident: $ty:ty => $kind:ident [$($acc:ident)*];)*) => {
        /// One GameObject as a document: the engine-only fields, the mandatory
        /// Transform, and each first-class component (`None` without it).
        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct Entity {
            pub id: u32,
            pub name: String,
            pub active: bool,
            pub is_static: bool,
            /// Index into the project's shared Layers registry (Unity's per-object
            /// layer). Defaults to 0 (`"Default"`).
            #[serde(default)]
            pub layer: u8,
            pub transform: TransformComponent,
            /// Transient staging slot: a default library material the editor's Add
            /// menu attached at the World level (which holds no library), to be
            /// inserted into the scene's material library once one is reachable
            /// (the inspector, or `apply_world_data` when a snapshot is restored).
            /// Never serialized; always `None` on a loaded entity.
            #[serde(skip)]
            pub pending_material: Option<MaterialAsset>,
            /// An entity can carry MANY scripts, each its own MonoBehaviour-equivalent
            /// Lua lifecycle table (#83).
            #[serde(default)]
            pub scripts: Vec<ScriptComponent>,
            $(
                #[doc = concat!("The `", stringify!($kind), "` component (`registry`).")]
                #[serde(default)]
                pub $field: Option<$ty>,
            )*
            /// Live link back to the source `.prefab` for a *linked* prefab instance
            /// (#216). `None` on a plain entity or a v1 unpacked copy. Carried on every
            /// entity of an instance.
            #[serde(default)]
            pub prefab_link: Option<PrefabLink>,
            pub parent_id: Option<u32>,
            /// Document-only (#453): the bone of the parent's skeleton this entity
            /// hangs from, then that bone's ancestor bones, nearest first. Bones are
            /// rebuilt from the model on load, so a child of `hand_r` is saved under
            /// the skinned entity with `parent_bone: ["hand_r", "forearm_r", …]` and
            /// re-parented by name once the skeleton is back; when the model no longer
            /// has `hand_r`, the nearest ancestor it still has takes it (#1252).
            /// Always empty on a live entity.
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub parent_bone: Vec<String>,
            /// Document-only (#1264): each component id leaf (by its pointer, see
            /// `refs`) that names a bone, saved as that bone's name, then its ancestor
            /// bones', nearest first, the way `parent_bone` keeps a bone child. The
            /// leaf itself names the skinned entity, and is re-bound to the bone by
            /// name once the skeleton is rebuilt (`scene::skeleton::bone_refs`).
            /// Always empty on a live entity.
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
