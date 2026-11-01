//! src/components/entity/prefab_link.rs — an instance entity's link to its source
//! (a `.prefab`, or an imported model: Unity's model prefab, #1637), and its
//! overrides in Unity's shapes (#216, #1527, #1665).
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
//! - **removed components** (Unity's `m_RemovedComponents`), by name;
//! - on a model instance's root, **removed parts** (Unity's
//!   `m_RemovedGameObjects`), by node path (#1665).
//!
//! Every entity id the three hold is visited by [`visit_override_refs`](super::override_refs::visit_override_refs), the one walk
//! (`override_refs`): a remap on stamp, load and Stop, the local↔scene conversion of an apply
//! to source, a despawn's `forget`, and a bone kept by name.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::override_refs::{each_override_ref, visit_override_refs};
use super::refs::{escape_token, EntityRefs, RefVisit};

/// The last token of a list's size entry: `/scripts/#size` (Unity's `Array.size`).
/// No list index and no field name the engine serializes is `#size`.
pub const SIZE_TOKEN: &str = "#size";

/// The key prefix of a removed part ([`PrefabOverrides::removed_parts`]):
/// `-part/Kit/Wall`. No JSON pointer starts with `-`.
const PART_KEY: &str = "-part/";

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
    /// Path to the source: a `.prefab` asset, or a model file (#1637). The same
    /// string for every entity of one instance.
    pub source: String,
    /// The corresponding source entity's LOCAL id (0-based; root = 0). This is the
    /// stable correlation key against `PrefabData::entities`, order-independent.
    pub local_id: u32,
    /// The entity's node path inside its source when the source is a model
    /// (#1637): `Kit/Wall`, a duplicate sibling name taking `[n]`. A model's local
    /// ids follow its node order, which a re-import may change, so propagation
    /// re-derives `local_id` from this key first. `None` for a `.prefab` source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    /// This entity's overrides as last stored: its diff against [`Self::baseline`]
    /// in the shapes of [`PrefabOverrides`] (a removed part is recorded, not
    /// diffed). Propagation rebuilds the entity from the source, then re-applies them, so what the instance did not change
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

/// One instance entity's overrides, in Unity's shapes (see the module docs).
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
    /// Parts of a model instance the user removed (Unity's *Removed GameObject*
    /// override, `m_RemovedGameObjects`), by node path, held on the instance root
    /// only (#1665): propagation never stamps them back. Not a diff: the live set
    /// carries it over (`scene::prefab::live`).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub removed_parts: BTreeSet<String>,
}

impl PrefabOverrides {
    pub fn is_empty(&self) -> bool {
        self.modifications.is_empty()
            && self.added_components.is_empty()
            && self.removed_components.is_empty()
            && self.removed_parts.is_empty()
    }

    /// Every override's key, the name the API and the inspector use for it: a
    /// modification's pointer, `+/<component>` for an added component,
    /// `-/<component>` for a removed one and `-part/<node path>` for a removed part.
    pub fn keys(&self) -> Vec<String> {
        let mods = self.modifications.keys().cloned();
        let added = self.added_components.keys().map(|c| format!("+/{c}"));
        let removed = self.removed_components.iter().map(|c| format!("-/{c}"));
        let parts = self.removed_parts.iter().map(|p| format!("{PART_KEY}{p}"));
        mods.chain(added).chain(removed).chain(parts).collect()
    }

    /// The override at `key` ([`Self::keys`]) with the ones that go with it, if there
    /// is one: a list whose length changed goes whole ([`Self::list_of`]), as Unity
    /// applies and reverts a whole array.
    pub fn only(&self, key: &str) -> Option<Self> {
        let mut one = Self::default();
        if let Some(c) = key.strip_prefix("+/") {
            let value = self.added_components.get(c)?;
            one.added_components.insert(c.to_string(), value.clone());
        } else if let Some(c) = key.strip_prefix("-/") {
            self.removed_components.get(c)?;
            one.removed_components.insert(c.to_string());
        } else if let Some(p) = key.strip_prefix(PART_KEY) {
            self.removed_parts.get(p)?;
            one.removed_parts.insert(p.to_string());
        } else {
            self.modifications.get(key)?;
            one.modifications = self
                .modifications
                .iter()
                .filter(|(k, _)| self.together(key, k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
        }
        Some(one)
    }

    /// Drop the override at `key` ([`Self::keys`]) and the ones that go with it
    /// ([`Self::only`]); whether there was one.
    pub fn remove(&mut self, key: &str) -> bool {
        if let Some(c) = key.strip_prefix("+/") {
            self.added_components.remove(c).is_some()
        } else if let Some(c) = key.strip_prefix("-/") {
            self.removed_components.remove(c)
        } else if let Some(p) = key.strip_prefix(PART_KEY) {
            self.removed_parts.remove(p)
        } else if self.modifications.contains_key(key) {
            let gone: Vec<String> = self
                .modifications
                .keys()
                .filter(|k| self.together(key, k))
                .cloned()
                .collect();
            for k in gone {
                self.modifications.remove(&k);
            }
            true
        } else {
            false
        }
    }

    /// The list modification `key` belongs to when that list's length changed: the
    /// shallowest list with a size entry that is `key` or holds it (`/scripts` for
    /// `/scripts/#size`, `/scripts/1` and `/scripts/0/name`). Its size and every
    /// entry under it are one override to apply or revert.
    pub fn list_of<'k>(&self, key: &'k str) -> Option<&'k str> {
        let mut end = 0;
        while let Some(i) = key.get(end + 1..).and_then(|rest| rest.find('/')) {
            end += i + 1;
            let list = &key[..end];
            if self
                .modifications
                .contains_key(&format!("{list}/{SIZE_TOKEN}"))
            {
                return Some(list);
            }
        }
        None
    }

    /// Whether modification `other` goes with `key`: it is `key`, or both sit in the
    /// list of [`Self::list_of`]`(key)`.
    fn together(&self, key: &str, other: &str) -> bool {
        match self.list_of(key) {
            Some(list) => other.strip_prefix(list).is_some_and(|r| r.starts_with('/')),
            None => other == key,
        }
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
        each_override_ref(&self.overrides, &mut |p, id| {
            visit(&override_pointer(p), id)
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
