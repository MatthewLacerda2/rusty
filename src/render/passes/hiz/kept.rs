//! What a camera's cull keeps across frames (#1963): bind groups rebuilt only
//! when a resource they name was replaced, and the CPU tables it refills.

use glam::Vec3;

/// A value built from some resources, kept until one of them is replaced: the
/// key is the resources themselves (wgpu compares them by identity), so a grown
/// buffer or a resized texture rebuilds it and nothing else does.
pub(crate) struct Kept<K, V> {
    held: Option<(K, V)>,
}

impl<K, V> Default for Kept<K, V> {
    fn default() -> Self {
        Self { held: None }
    }
}

impl<K: PartialEq, V> Kept<K, V> {
    /// The value for `key`, built by `make` when the key changed since last time.
    pub(crate) fn get(&mut self, key: K, make: impl FnOnce() -> V) -> &V {
        if self.held.as_ref().is_none_or(|(k, _)| *k != key) {
            self.held = Some((key, make()));
        }
        &self.held.as_ref().expect("held above").1
    }

    /// The value last built, if any.
    pub(crate) fn value(&self) -> Option<&V> {
        self.held.as_ref().map(|(_, v)| v)
    }
}

/// The CPU tables a cull fills each frame, kept so a frame allocates nothing once
/// they reached the scene's size.
#[derive(Default)]
pub(crate) struct Tables {
    /// Each instance's object slot.
    pub(crate) slots: Vec<u32>,
    /// The tested bounds by object slot.
    pub(crate) by_slot: Vec<Option<(Vec3, Vec3)>>,
    /// Each instance's bounds, as `hiz.wgsl`'s `Bounds`.
    pub(crate) bounds: Vec<[f32; 8]>,
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
