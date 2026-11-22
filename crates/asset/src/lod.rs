//! Unity's and Blender's `_LOD<n>` convention. Matches the glTF node name, else
//! the sub-object id (case-insensitive).

use super::ImportedAsset;

/// Levels finest first; numbers need not be contiguous.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LodSet {
    pub base: String,
    pub levels: Vec<Vec<String>>,
}

fn lod_suffix(name: &str) -> Option<(&str, u32)> {
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

/// Base, level and Blender's duplicate suffix (`Crate_LOD0.001` → `Crate`, 0,
/// `.001`), so each Alt+D copy of a set is a set of its own. Nodes sharing a
/// parent, base and suffix are one LODGroup, as Unity groups by name.
pub fn node_lod(name: &str) -> Option<(&str, u32, &str)> {
    let dup = name.rfind('.').filter(|&at| {
        let digits = &name[at + 1..];
        !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
    });
    let (stem, suffix) = name.split_at(dup.unwrap_or(name.len()));
    let (base, level) = lod_suffix(stem)?;
    Some((base, level, suffix))
}

/// In the file order of each set's first sub-object.
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
        .map(|(base, members)| LodSet {
            base,
            levels: by_level(members),
        })
        .collect()
}

/// One list per level, finest first; members of one level keep their order.
pub fn by_level<T>(mut members: Vec<(u32, T)>) -> Vec<Vec<T>> {
    members.sort_by_key(|(level, _)| *level);
    let mut levels: Vec<(u32, Vec<T>)> = Vec::new();
    for (level, member) in members {
        match levels.last_mut() {
            Some((l, these)) if *l == level => these.push(member),
            _ => levels.push((level, vec![member])),
        }
    }
    levels.into_iter().map(|(_, these)| these).collect()
}

pub fn lod_set(asset: &ImportedAsset, base: &str) -> Option<LodSet> {
    lod_sets(asset).into_iter().find(|s| s.base == base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SubMesh;

    fn sub(id: &str, node: Option<&str>) -> SubMesh {
        SubMesh {
            id: id.to_string(),
            vertices: Default::default(),
            indices: Default::default(),
            ranges: Vec::new(),
            skin: None,
            clips: Vec::new(),
            node_name: node.map(str::to_string),
            clusters: None,
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
    fn a_node_name_keeps_its_duplicate_suffix_apart() {
        assert_eq!(node_lod("Crate_LOD0"), Some(("Crate", 0, "")));
        assert_eq!(node_lod("Crate_LOD1.001"), Some(("Crate", 1, ".001")));
        assert_eq!(node_lod("Crate.001"), None);
        assert_eq!(node_lod("Crate_LOD1."), None);
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
            nodes: Vec::new(),
        };
        let sets = lod_sets(&asset);
        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0].base, "Crate");
        assert_eq!(sets[0].levels, vec![vec!["Cube.000"], vec!["Cube.002"]]);
        assert_eq!(sets[1].levels, vec![vec!["Lamp_LOD0"], vec!["Lamp_LOD1"]]);
        assert!(lod_set(&asset, "Barrel").is_none());
    }
}
