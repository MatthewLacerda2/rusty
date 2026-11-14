//! Group 0 kept per procedural sky slot (#1620): two skies drawn in one frame (the
//! Inspector preview's default sky beside a scene's own) bind different procedural
//! cubes, and a group-0 rebuild per switch would be two bind-group creations a frame.
//! The group built for the slot not bound is kept, so a switch swaps the two.
//!
//! Generic over the group so its rule is tested without an adapter; the renderer
//! holds one over `wgpu::BindGroup`.

/// The group-0 bind group last built for the procedural slot not bound now.
pub(crate) struct SpareGroup<G> {
    spare: Option<(usize, G)>,
    /// The procedural slot the bound group shows (`None`: a panorama, or unknown).
    bound: Option<usize>,
    /// The bound procedural slot changed since group 0 was last built.
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
    /// Replace `group` with one showing procedural `slot` (`None`: a panorama shows).
    /// When only the sky slot changed (`only_sky`), the spare built for `slot` is
    /// reused, and the replaced group kept as the spare for the slot it showed; any
    /// other change (a buffer, a panorama, a probe array) drops the spare, since it
    /// names what was replaced. `build` makes a fresh group.
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

    /// Groups are numbered by build; `build` hands out the next one.
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
