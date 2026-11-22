//! A glTF mesh with several primitives, an FBX `ByPolygon` material mapping or an
//! `.obj` split by `usemtl` is still one addressable sub-mesh, drawn in runs with
//! their own material: Unity's sub-meshes under one `Mesh`.

use std::ops::Range;

/// `material` indexes the parent `ImportedAsset::materials`; `None` when the source names none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterialRange {
    pub indices: Range<u32>,
    pub material: Option<usize>,
}

/// A run continuing the last with the same material extends it, so two primitives sharing a material stay one draw.
pub fn push_range(ranges: &mut Vec<MaterialRange>, indices: Range<u32>, material: Option<usize>) {
    if indices.is_empty() {
        return;
    }
    match ranges.last_mut() {
        Some(last) if last.material == material && last.indices.end == indices.start => {
            last.indices.end = indices.end;
        }
        _ => ranges.push(MaterialRange { indices, material }),
    }
}

pub fn whole(count: usize, material: Option<usize>) -> Vec<MaterialRange> {
    let mut ranges = Vec::new();
    push_range(&mut ranges, 0..count as u32, material);
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacent_runs_of_one_material_merge_and_others_stay_apart() {
        let mut ranges = Vec::new();
        push_range(&mut ranges, 0..3, Some(0));
        push_range(&mut ranges, 3..6, Some(0));
        push_range(&mut ranges, 6..6, Some(1));
        push_range(&mut ranges, 6..9, Some(1));
        push_range(&mut ranges, 9..12, None);
        let runs: Vec<_> = ranges
            .iter()
            .map(|r| (r.indices.clone(), r.material))
            .collect();
        assert_eq!(runs, vec![(0..6, Some(0)), (6..9, Some(1)), (9..12, None)]);
    }

    #[test]
    fn an_empty_list_has_no_runs() {
        assert!(whole(0, Some(0)).is_empty());
        assert_eq!(
            whole(6, None),
            vec![MaterialRange {
                indices: 0..6,
                material: None
            }]
        );
    }
}
