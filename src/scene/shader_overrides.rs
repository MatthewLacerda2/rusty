//! src/scene/shader_overrides.rs — per-entity runtime shader-param overrides (#670):
//! Unity's `MaterialPropertyBlock`.
//!
//! A runtime param (#399) normally lives on the material asset, so every entity
//! sharing the material draws with one value. An override is a value for **one
//! entity**: it wins over its material's value for that entity alone, and the
//! material stays shared and unchanged — one grunt flashes, the other nine do not.
//!
//! Overrides are **runtime-only**, like Unity's property blocks: never serialized,
//! dropped when the entity is destroyed and when a scene document is applied (a
//! load, or Stop restoring the edit scene). Values are keyed by the param's
//! canonical name (`ParamLayout::name`), so they merge over the material's
//! `shader_params` name for name.

use std::collections::BTreeMap;

/// One entity's (or one material's) runtime param values, by canonical name.
pub type ParamValues = BTreeMap<String, Vec<f32>>;

/// Every entity's overrides, by entity id. Ordered maps: the sim is deterministic.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShaderOverrides(BTreeMap<u32, ParamValues>);

impl ShaderOverrides {
    /// Entity `id`'s overrides, if it has any.
    pub fn of(&self, id: u32) -> Option<&ParamValues> {
        self.0.get(&id)
    }

    /// Whether entity `id` has any override.
    pub fn has(&self, id: u32) -> bool {
        self.0.contains_key(&id)
    }

    /// Override param `name` (canonical) on entity `id` with `value`.
    pub fn set(&mut self, id: u32, name: String, value: Vec<f32>) {
        self.0.entry(id).or_default().insert(name, value);
    }

    /// Drop entity `id`'s override of `name` (canonical), or every override it has
    /// when `name` is `None`. An entity left with none has no entry at all.
    pub fn clear(&mut self, id: u32, name: Option<&str>) {
        let Some(name) = name else {
            self.0.remove(&id);
            return;
        };
        if let Some(values) = self.0.get_mut(&id) {
            values.remove(name);
            if values.is_empty() {
                self.0.remove(&id);
            }
        }
    }

    /// Drop every override (a scene document replaced the World).
    pub fn clear_all(&mut self) {
        self.0.clear();
    }

    /// How many entities carry an override.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clearing_the_last_override_drops_the_entity() {
        let mut o = ShaderOverrides::default();
        o.set(7, "hit_flash.amount".into(), vec![1.0]);
        o.set(7, "hit_flash.color".into(), vec![1.0, 0.0, 0.0]);
        o.clear(7, Some("hit_flash.amount"));
        assert!(o.has(7), "one override left");
        o.clear(7, Some("hit_flash.color"));
        assert!(!o.has(7) && o.is_empty());
        o.set(7, "a.b".into(), vec![0.5]);
        o.set(8, "a.b".into(), vec![0.5]);
        o.clear(7, None);
        assert_eq!((o.has(7), o.len()), (false, 1), "all of 7's, none of 8's");
    }
}
