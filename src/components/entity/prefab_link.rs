//! src/components/entity/prefab_link.rs — an instance entity's link to its `.prefab`,
//! and its overrides in Unity's three shapes (#216, #1527).
//!
//! An instance's divergence from its source is held as Unity holds it:
//! - **property modifications**, one per leaf, by JSON pointer into the entity
//!   document (`/light/intensity`). A list's elements are addressed by index, and a
//!   list whose length changed has one **size entry**, `<list>/#size` (Unity's
//!   `Array.size`): shrinking is a size entry alone, growing is a size entry plus one
//!   entry per new element holding its whole value, and the elements both lists
//!   hold keep following the source leaf by leaf. A value the leaf diff can't split
//!   (an enum that switched variant, a map key the source lacks) is one entry holding
//!   it whole; a key the instance dropped is `null`;
//! - **added components** (Unity's `m_AddedComponents`), each held whole;
//! - **removed components** (Unity's `m_RemovedComponents`), by name.
//!
//! Every entity id the three hold is visited by [`visit_override_refs`], the one walk
//! (`refs`): a remap on stamp, load and Stop, the local↔scene conversion of an apply
//! to source, a despawn's `forget`, and a bone kept by name.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::refs::{escape_token, EntityRefs, RefVisit};
use super::Entity;

/// The last token of a list's size entry: `/scripts/#size` (Unity's `Array.size`).
/// No list index and no field name the engine serializes is `#size`.
pub const SIZE_TOKEN: &str = "#size";

/// The live link from an instance entity back to the `.prefab` it was stamped from
/// (#216). Set on EVERY entity of a linked instance — not just the root — so that
/// reordering the source's entities can never scramble which source entity an
/// instance entity corresponds to: the correlation is the explicit `local_id` (the
/// source's own 0-based local id), independent of order. The root anchors the
/// `source` path; non-root entities carry the same path so each can be matched on
/// its own. Absent (`None`) on a plain entity or a v1 *unpacked* copy, which has no
/// link back to the asset.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PrefabLink {
    /// Path to the source `.prefab` asset (the same string for every entity of one
    /// instance).
    pub source: String,
    /// The corresponding source entity's LOCAL id (0-based; root = 0). This is the
    /// stable correlation key against `PrefabData::entities`, order-independent.
    pub local_id: u32,
    /// This entity's overrides as last stored: its diff against [`Self::baseline`]
    /// in the three shapes of [`PrefabOverrides`]. Propagation rebuilds the entity
    /// from the source, then re-applies them, so what the instance did not change
    /// tracks the source. The current set is `scene::prefab::live::live_overrides`;
    /// it is stored here before a propagation, a save and a Play snapshot.
    #[serde(default, skip_serializing_if = "PrefabOverrides::is_empty")]
    pub overrides: PrefabOverrides,
    /// The source as this entity last took it (#1527): its masked document as the
    /// stamp or the last propagation built it, before any override. An edit is an
    /// override the moment it is made because the live overrides are the entity's
    /// diff against this, not a list someone records. Runtime only: a saved scene or
    /// a Play snapshot carries `overrides` and no baseline, and its load or restore
    /// propagates a fresh one (none while the source is missing).
    #[serde(skip)]
    pub baseline: Option<std::sync::Arc<Value>>,
}

/// One instance entity's overrides, in Unity's three shapes (see the module docs).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PrefabOverrides {
    /// Property modifications: JSON pointer → value, a list's length at
    /// `<list>/#size`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub modifications: BTreeMap<String, Value>,
    /// Components the instance has and the source lacks, by entity field name, each
    /// held whole.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub added_components: BTreeMap<String, Value>,
    /// Source components the instance removed, by entity field name.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub removed_components: BTreeSet<String>,
}

impl PrefabOverrides {
    pub fn is_empty(&self) -> bool {
        self.modifications.is_empty()
            && self.added_components.is_empty()
            && self.removed_components.is_empty()
    }

    /// Every override's key, the name the API and the inspector use for it: a
    /// modification's pointer, `+/<component>` for an added component and
    /// `-/<component>` for a removed one.
    pub fn keys(&self) -> Vec<String> {
        let mods = self.modifications.keys().cloned();
        let added = self.added_components.keys().map(|c| format!("+/{c}"));
        let removed = self.removed_components.iter().map(|c| format!("-/{c}"));
        mods.chain(added).chain(removed).collect()
    }

    /// The override at `key` ([`Self::keys`]) alone, if there is one.
    pub fn only(&self, key: &str) -> Option<Self> {
        let mut one = Self::default();
        if let Some(c) = key.strip_prefix("+/") {
            let value = self.added_components.get(c)?;
            one.added_components.insert(c.to_string(), value.clone());
        } else if let Some(c) = key.strip_prefix("-/") {
            self.removed_components.get(c)?;
            one.removed_components.insert(c.to_string());
        } else {
            let value = self.modifications.get(key)?;
            one.modifications.insert(key.to_string(), value.clone());
        }
        Some(one)
    }

    /// Drop the override at `key` ([`Self::keys`]); whether there was one.
    pub fn remove(&mut self, key: &str) -> bool {
        if let Some(c) = key.strip_prefix("+/") {
            self.added_components.remove(c).is_some()
        } else if let Some(c) = key.strip_prefix("-/") {
            self.removed_components.remove(c)
        } else {
            self.modifications.remove(key).is_some()
        }
    }
}

/// Visit every entity id the overrides hold, each with its pointer in the entity
/// document (`/joint/connected_body`): an id leaf of a modification, one inside a
/// modification's whole value, and one inside an added component, which goes
/// through that component's own [`EntityRefs`] walk, so a list of ids drops a
/// dropped one as the component itself does. A dropped id elsewhere becomes `null`,
/// the override of "no reference".
pub fn visit_override_refs(overrides: &mut PrefabOverrides, visit: &mut RefVisit<'_>) {
    for (path, value) in &mut overrides.modifications {
        visit_value_refs(path, value, visit);
    }
    for (name, value) in &mut overrides.added_components {
        if !visit_component_refs(name, value, visit) {
            visit_value_refs(&format!("/{}", escape_token(name)), value, visit);
        }
    }
}

/// Walk `value`, the document's value at `pointer`, visiting every leaf whose pointer
/// is an id field ([`Entity::is_ref_pointer`]).
fn visit_value_refs(pointer: &str, value: &mut Value, visit: &mut RefVisit<'_>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                visit_value_refs(&format!("{pointer}/{}", escape_token(k)), v, visit);
            }
        }
        Value::Array(items) => {
            for (i, v) in items.iter_mut().enumerate() {
                visit_value_refs(&format!("{pointer}/{i}"), v, visit);
            }
        }
        leaf if Entity::is_ref_pointer(pointer) => {
            let mut slot = match &*leaf {
                Value::Null => None,
                v => match v.as_u64().and_then(|n| u32::try_from(n).ok()) {
                    Some(id) => Some(id),
                    None => return,
                },
            };
            visit(pointer, &mut slot);
            *leaf = slot.map_or(Value::Null, Value::from);
        }
        _ => {}
    }
}

/// Run added component `name`'s own [`EntityRefs`] walk over `value`, through an
/// entity carrying it alone; `false` (and `value` untouched) when it doesn't read
/// back as that component.
fn visit_component_refs(name: &str, value: &mut Value, visit: &mut RefVisit<'_>) -> bool {
    let Ok(Value::Object(mut doc)) = serde_json::to_value(Entity::new(0, String::new())) else {
        return false;
    };
    doc.insert(name.to_string(), value.clone());
    let Ok(mut holder) = serde_json::from_value::<Entity>(Value::Object(doc)) else {
        return false;
    };
    holder.visit_refs(visit);
    match serde_json::to_value(&holder) {
        Ok(Value::Object(mut back)) => match back.remove(name) {
            Some(walked) => {
                *value = walked;
                true
            }
            None => false,
        },
        _ => false,
    }
}

/// The pointer of an override's id under the link: `/overrides/<escaped pointer>`.
fn override_pointer(pointer: &str) -> String {
    format!("/overrides/{}", escape_token(pointer))
}

impl EntityRefs for PrefabLink {
    fn visit_refs(&mut self, visit: &mut RefVisit<'_>) {
        visit_override_refs(&mut self.overrides, &mut |p, r| {
            visit(&override_pointer(p), r)
        });
    }

    fn each_ref(&self, visit: &mut dyn FnMut(&str, u32)) {
        let mut overrides = self.overrides.clone();
        visit_override_refs(&mut overrides, &mut |p, r| {
            if let Some(id) = *r {
                visit(&override_pointer(p), id);
            }
        });
    }

    #[cfg(test)]
    fn every_ref() -> Self {
        let mut overrides = PrefabOverrides::default();
        overrides
            .modifications
            .insert("/joint/connected_body".into(), Value::Null);
        overrides.added_components.insert(
            "joint".into(),
            serde_json::json!({ "connected_body": null }),
        );
        Self {
            overrides,
            ..Self::default()
        }
    }
}
