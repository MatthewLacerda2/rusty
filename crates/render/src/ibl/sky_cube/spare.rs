/// Keeps group 0 for the unbound procedural slot, so alternating two skies swaps, not rebuilds.
pub(crate) struct SpareGroup<G> {
    spare: Option<(usize, G)>,
    bound: Option<usize>,
    pub switched: bool,
}

impl<G> Default for SpareGroup<G> {
    fn default() -> Self {
        Self {
            spare: None,
            bound: None,
            switched: false,
        }
    }
}

impl<G> SpareGroup<G> {
    pub(crate) fn replace(
        &mut self,
        group: &mut G,
        (slot, only_sky): (Option<usize>, bool),
        build: impl FnOnce() -> G,
    ) {
        self.switched = false;
        let reuse = match self.spare.take() {
            Some((held, g)) if only_sky && Some(held) == slot => Some(g),
            _ => None,
        };
        let old = std::mem::replace(group, reuse.unwrap_or_else(build));
        self.spare = if only_sky {
            self.bound.map(|s| (s, old))
        } else {
            None
        };
        self.bound = slot;
    }
}

#[cfg(test)]
mod tests {
    use super::SpareGroup;

    fn replace(
        cache: &mut SpareGroup<u32>,
        group: &mut u32,
        next: &mut u32,
        to: (Option<usize>, bool),
    ) {
        cache.replace(group, to, || {
            *next += 1;
            *next
        });
    }

    #[test]
    fn alternating_slots_build_each_group_once() {
        let (mut cache, mut group, mut next) = (SpareGroup::default(), 0, 0);
        replace(&mut cache, &mut group, &mut next, (Some(0), false));
        assert_eq!(group, 1);
        replace(&mut cache, &mut group, &mut next, (Some(1), true));
        assert_eq!(group, 2, "the other slot's first group is built");
        for _ in 0..3 {
            replace(&mut cache, &mut group, &mut next, (Some(0), true));
            assert_eq!(group, 1, "slot 0's group is swapped back in");
            replace(&mut cache, &mut group, &mut next, (Some(1), true));
            assert_eq!(group, 2);
        }
        assert_eq!(next, 2, "two builds in all");
    }

    #[test]
    fn any_other_change_drops_the_spare() {
        let (mut cache, mut group, mut next) = (SpareGroup::default(), 0, 0);
        replace(&mut cache, &mut group, &mut next, (Some(0), false));
        replace(&mut cache, &mut group, &mut next, (Some(1), true));
        replace(&mut cache, &mut group, &mut next, (Some(1), false));
        assert_eq!(group, 3, "a buffer changed: rebuilt");
        replace(&mut cache, &mut group, &mut next, (Some(0), true));
        assert_eq!(group, 4, "slot 0's old group named the old buffer");
        replace(&mut cache, &mut group, &mut next, (None, false));
        replace(&mut cache, &mut group, &mut next, (Some(1), true));
        assert_eq!(group, 6, "a panorama's group is never a slot's spare");
    }
}
