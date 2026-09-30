//! src/asset/lod.rs — the `_LOD<n>` naming convention (#472).
//!
//! Unity's and Blender's convention for authored levels of detail: objects named
//! `Crate_LOD0`, `Crate_LOD1`, `Crate_LOD2` are one prop, `Crate`, at decreasing
//! detail. A sub-object belongs to a set when its glTF **node** name (the Blender
//! object name) or, failing that, its sub-object id (the mesh name) ends in
//! `_LOD<n>` (case-insensitive). The scene layer turns each set into one entity
//! carrying an `LODGroup` (`scene::lod_instance`). Pure: names in, sets out.

use super::ImportedAsset;

/// One prop's levels: the shared base name, and each level's sub-object ids, finest
/// first. Level numbers need not be contiguous — `_LOD0` and `_LOD2` make two levels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LodSet {
    pub base: String,
    pub levels: Vec<Vec<String>>,
}

/// Split `name` into its base and level when it ends in `_LOD<n>`.
pub fn lod_suffix(name: &str) -> Option<(&str, u32)> {
    let at = name.rfind('_')?;
    let (base, tail) = (&name[..at], &name[at + 1..]);
    let tag = tail.get(..3).filter(|t| t.eq_ignore_ascii_case("lod"));
    let digits = tag.and(tail.get(3..))?;
    let numeric = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
    match numeric && !base.is_empty() {
        true => Some((base, digits.parse().ok()?)),
        false => None,
    }
}

/// Every LOD set in `asset`, in the file order of each set's first sub-object.
pub fn lod_sets(asset: &ImportedAsset) -> Vec<LodSet> {
    let mut found: Vec<(String, Vec<(u32, String)>)> = Vec::new();
    for sub in &asset.sub_meshes {
        let named = sub.node_name.as_deref().and_then(lod_suffix);
        let Some((base, level)) = named.or_else(|| lod_suffix(&sub.id)) else {
            continue;
        };
        match found.iter_mut().find(|(b, _)| b == base) {
            Some((_, members)) => members.push((level, sub.id.clone())),
            None => found.push((base.to_string(), vec![(level, sub.id.clone())])),
        }
    }
    found
        .into_iter()
        .map(|(base, mut members)| {
            members.sort_by_key(|(level, _)| *level);
            let mut levels: Vec<(u32, Vec<String>)> = Vec::new();
            for (level, id) in members {
                match levels.last_mut() {
                    Some((l, ids)) if *l == level => ids.push(id),
                    _ => levels.push((level, vec![id])),
                }
            }
            let levels = levels.into_iter().map(|(_, ids)| ids).collect();
            LodSet { base, levels }
        })
        .collect()
}

/// The set named `base` in `asset`, if any.
pub fn lod_set(asset: &ImportedAsset, base: &str) -> Option<LodSet> {
    lod_sets(asset).into_iter().find(|s| s.base == base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::SubMesh;

    fn sub(id: &str, node: Option<&str>) -> SubMesh {
        SubMesh {
            id: id.to_string(),
            vertices: Vec::new(),
            indices: Vec::new(),
            material: None,
            skin: None,
            clips: Vec::new(),
            node_name: node.map(str::to_string),
        }
    }

    #[test]
    fn suffix_parses_base_and_level() {
        assert_eq!(lod_suffix("Crate_LOD0"), Some(("Crate", 0)));
        assert_eq!(lod_suffix("big_crate_lod12"), Some(("big_crate", 12)));
        assert_eq!(lod_suffix("Crate_LOD"), None);
        assert_eq!(lod_suffix("Crate_LODx"), None);
        assert_eq!(lod_suffix("_LOD1"), None);
        assert_eq!(lod_suffix("CrateLOD1"), None);
        assert_eq!(lod_suffix("LOD"), None);
    }

    #[test]
    fn sets_group_by_base_order_levels_and_prefer_node_names() {
        let asset = ImportedAsset {
            sub_meshes: vec![
                sub("Cube.002", Some("Crate_LOD2")),
                sub("Barrel", None),
                sub("Cube.000", Some("Crate_LOD0")),
                sub("Lamp_LOD1", None),
                sub("Lamp_LOD0", Some("Lamp")),
            ],
            materials: Vec::new(),
        };
        let sets = lod_sets(&asset);
        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0].base, "Crate");
        assert_eq!(sets[0].levels, vec![vec!["Cube.000"], vec!["Cube.002"]]);
        assert_eq!(sets[1].levels, vec![vec!["Lamp_LOD0"], vec!["Lamp_LOD1"]]);
        assert!(lod_set(&asset, "Barrel").is_none());
    }
}
