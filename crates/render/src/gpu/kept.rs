pub(crate) struct Kept<K, V> {
    held: Option<(K, V)>,
}

impl<K, V> Default for Kept<K, V> {
    fn default() -> Self {
        Self { held: None }
    }
}

impl<K: PartialEq, V> Kept<K, V> {
    pub(crate) fn get(&mut self, key: K, make: impl FnOnce() -> V) -> &V {
        if self.held.as_ref().is_none_or(|(k, _)| *k != key) {
            self.held = Some((key, make()));
        }
        &self.held.as_ref().expect("held above").1
    }

    pub(crate) fn key(&self) -> Option<&K> {
        self.held.as_ref().map(|(k, _)| k)
    }

    pub(crate) fn value(&self) -> Option<&V> {
        self.held.as_ref().map(|(_, v)| v)
    }
}

#[cfg(test)]
mod tests {
    use super::Kept;

    #[test]
    fn a_kept_value_is_rebuilt_only_when_its_key_changes() {
        let mut kept = Kept::default();
        let mut built = 0;
        for key in [1, 1, 2, 2, 1] {
            kept.get(key, || {
                built += 1;
                key * 10
            });
        }
        assert_eq!(built, 3, "1, then 2, then 1 again");
        assert_eq!(kept.value(), Some(&10));
    }
}
