use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::override_refs::{each_override_ref, visit_override_refs};
use super::refs::{escape_token, EntityRefs, RefVisit};

pub const SIZE_TOKEN: &str = "#size";

pub const PART_KEY: &str = "-part/";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PrefabLink {
    pub source: String,
    pub local_id: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    #[serde(default, skip_serializing_if = "PrefabOverrides::is_empty")]
    pub overrides: PrefabOverrides,
    #[serde(skip)]
    pub baseline: Option<std::sync::Arc<Value>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PrefabOverrides {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub modifications: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub added_components: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub removed_components: BTreeSet<String>,
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

    pub fn keys(&self) -> Vec<String> {
        let mods = self.modifications.keys().cloned();
        let added = self.added_components.keys().map(|c| format!("+/{c}"));
        let removed = self.removed_components.iter().map(|c| format!("-/{c}"));
        let parts = self.removed_parts.iter().map(|p| format!("{PART_KEY}{p}"));
        mods.chain(added).chain(removed).chain(parts).collect()
    }

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

    fn together(&self, key: &str, other: &str) -> bool {
        match self.list_of(key) {
            Some(list) => other.strip_prefix(list).is_some_and(|r| r.starts_with('/')),
            None => other == key,
        }
    }
}

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

    #[doc(hidden)]
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
