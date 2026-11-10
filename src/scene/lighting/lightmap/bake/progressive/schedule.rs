//! The order a progressive bake traces its passes in (#1970). Without a view, or
//! when the view sees all of the scene or none of it, every step is one pass over
//! every texel. With Prioritize View, the whole scene gets its first pass, so it
//! shows at once; then the texels the camera sees get all their other passes, so
//! the view converges first (Unity's Prioritize View); then the rest get theirs.
//! A texel's paths are the same either way, in the same order, so only when each
//! is traced moves, never the bytes.

/// One step of a progressive bake: how many more paths each group's texels get.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::scene::lighting::lightmap) struct Step {
    /// For the texels the view sees.
    pub seen: u32,
    /// For the rest.
    pub rest: u32,
}

impl Step {
    /// The paths a texel of the group `seen` gets in this step.
    pub fn of(&self, seen: bool) -> u32 {
        if seen {
            self.seen
        } else {
            self.rest
        }
    }
}

/// `samples` paths per texel in passes of `per_pass` (the last taking the rest),
/// as steps: the view's passes after the first run ahead of the rest's when
/// `split` (the view sees some of the scene, not all of it).
pub(in crate::scene::lighting::lightmap) fn steps(
    samples: u32,
    per_pass: u32,
    split: bool,
) -> Vec<Step> {
    let per_pass = per_pass.max(1);
    let mut counts = Vec::new();
    let mut done = 0;
    while done < samples.max(1) {
        let count = per_pass.min(samples.max(1) - done);
        counts.push(count);
        done += count;
    }
    let both = |&c: &u32| Step { seen: c, rest: c };
    if !split || counts.len() < 2 {
        return counts.iter().map(both).collect();
    }
    let later = &counts[1..];
    let view = later.iter().map(|&c| Step { seen: c, rest: 0 });
    let rest = later.iter().map(|&c| Step { seen: 0, rest: c });
    std::iter::once(both(&counts[0]))
        .chain(view)
        .chain(rest)
        .collect()
}
