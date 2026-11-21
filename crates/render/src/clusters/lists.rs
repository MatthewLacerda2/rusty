use super::bin::Binned;
use super::CLUSTER_COUNT;

#[derive(Clone, Copy, Default)]
pub(super) struct Pair {
    pub cluster: u32,
    pub item: u32,
    pub score: f32,
}

pub(super) fn fill_lists(out: &mut Binned, pairs: &[Pair], cap: usize) {
    for p in pairs {
        out.ranges[p.cluster as usize][1] += 1;
    }
    if out.ranges.iter().any(|r| r[1] as usize > cap) {
        return cut_lists(out, pairs, cap);
    }
    let mut offset = 0;
    for range in &mut out.ranges {
        range[0] = offset;
        offset += range[1];
    }
    let mut cursor: Vec<u32> = out.ranges.iter().map(|r| r[0]).collect();
    out.indices = vec![0; pairs.len()];
    for p in pairs {
        let at = &mut cursor[p.cluster as usize];
        out.indices[*at as usize] = p.item;
        *at += 1;
    }
}

fn cut_lists(out: &mut Binned, pairs: &[Pair], cap: usize) {
    let mut cursor = Vec::with_capacity(CLUSTER_COUNT);
    let mut offset = 0;
    for range in &out.ranges {
        cursor.push(offset);
        offset += range[1] as usize;
    }
    let mut sorted = vec![Pair::default(); pairs.len()];
    for p in pairs {
        let at = &mut cursor[p.cluster as usize];
        sorted[*at] = *p;
        *at += 1;
    }
    out.indices = Vec::with_capacity(pairs.len());
    let mut start = 0;
    for range in &mut out.ranges {
        let list = &mut sorted[start..start + range[1] as usize];
        start += list.len();
        let keep = list.len().min(cap);
        if keep < list.len() {
            list.select_nth_unstable_by(keep, |a, b| {
                b.score.total_cmp(&a.score).then(a.item.cmp(&b.item))
            });
            list[..keep].sort_unstable_by_key(|p| p.item);
            out.cluster_dropped += (list.len() - keep) as u32;
        }
        *range = [out.indices.len() as u32, keep as u32];
        out.indices.extend(list[..keep].iter().map(|p| p.item));
    }
}
