//! Per-entity runtime shader params, Unity's `MaterialPropertyBlock`: never saved, dropped on
//! destroy and when a document is applied. Keyed by canonical name, merged over the material's.

use std::collections::BTreeMap;

pub type ParamValues = BTreeMap<String, Vec<f32>>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShaderOverrides(BTreeMap<u32, ParamValues>);

impl ShaderOverrides {
    pub fn of(&self, id: u32) -> Option<&ParamValues> {
        self.0.get(&id)
    }

    pub fn entities(&self) -> impl Iterator<Item = u32> + '_ {
        self.0.keys().copied()
    }

    pub fn has(&self, id: u32) -> bool {
        self.0.contains_key(&id)
    }

    pub fn set(&mut self, id: u32, name: String, value: Vec<f32>) {
        self.0.entry(id).or_default().insert(name, value);
    }

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

    pub fn clear_all(&mut self) {
        self.0.clear();
    }

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
