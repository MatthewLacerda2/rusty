#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::lightmap) struct Step {
    pub seen: u32,
    pub rest: u32,
}

impl Step {
    pub fn of(&self, seen: bool) -> u32 {
        if seen {
            self.seen
        } else {
            self.rest
        }
    }
}

pub(in crate::lightmap) fn steps(samples: u32, per_pass: u32, split: bool) -> Vec<Step> {
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
