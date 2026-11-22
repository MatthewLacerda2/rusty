use super::Lightmaps;

impl Lightmaps {
    pub(crate) fn release(&mut self, kept: impl Fn(&[String]) -> bool) -> usize {
        let before = self.cache.len();
        let bound = &self.bound;
        self.cache
            .retain(|list, _| kept(list) || bound.iter().any(|b| b == list));
        before - self.cache.len()
    }

    #[cfg(test)]
    pub(crate) fn cached_lists(&self) -> Vec<Vec<String>> {
        let mut lists: Vec<Vec<String>> = self.cache.keys().cloned().collect();
        lists.sort();
        lists
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Entry, Lightmaps};

    fn list(name: &str) -> Vec<String> {
        vec![format!("{name}.png")]
    }

    #[test]
    fn gpu_a_release_keeps_the_kept_and_the_bound_lists_alone() {
        let Some(r) = crate::test_gpu::headless_or_skip(4, 4) else {
            return;
        };
        let mut maps = Lightmaps::new(&r.device, true);
        for name in ["kept", "bound", "left"] {
            maps.cache.insert(list(name), Entry::Ready(None));
        }
        maps.bound[0] = list("bound");
        assert_eq!(maps.release(|l| l == list("kept")), 1, "the list left goes");
        let mut cached: Vec<&Vec<String>> = maps.cache.keys().collect();
        cached.sort();
        assert_eq!(cached, [&list("bound"), &list("kept")]);
    }
}
