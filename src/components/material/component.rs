//! src/components/material/component.rs — the entity's reference to its library
//! materials, the first-class `Material` component (the asset it names is
//! [`super::MaterialAsset`]).

use serde::{Deserialize, Serialize};

/// The entity's references to library materials — the first-class component. Holds
/// only library keys: the material data is a shared asset in `Scene.materials`, and
/// this only points at it.
///
/// One key per sub-mesh of the entity's mesh, in the mesh's range order (Unity's
/// `Renderer.sharedMaterials`, #1285): a mesh imported with two materials draws its
/// first index range with `materials[0]` and its second with `materials[1]`. A
/// one-range mesh has a one-entry list. As in Unity, a range past the end of the list
/// is not drawn, and a material past the last range draws that range again over it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialComponent {
    pub materials: Vec<String>,
}

impl MaterialComponent {
    /// A one-material reference to library key `key`.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            materials: vec![key.into()],
        }
    }

    /// The reference to entity `id`'s own library material, keyed
    /// [`entity_material_key`] — what every "give this entity a material" path attaches.
    pub fn for_entity(id: u32) -> Self {
        Self::new(entity_material_key(id))
    }

    /// The first material's key (Unity's `Renderer.sharedMaterial`), or `""` for an
    /// empty list, which no library entry has.
    pub fn first(&self) -> &str {
        self.materials.first().map_or("", String::as_str)
    }
}

/// The library key of entity `id`'s own material (`entity_{id}_material`): the one
/// spelling every surface that creates a per-entity material uses (#997).
pub fn entity_material_key(id: u32) -> String {
    format!("entity_{id}_material")
}

impl crate::components::FirstClass for MaterialComponent {
    const NAME: &'static str = "Material";
}
