//! A sub-object's mesh-collider entry in a model's sidecar: a bare kind builds from its own mesh,
//! an object (`{"kind": "TriMesh", "mesh": "Chair_col"}`) names another sub-object as the collision
//! mesh (Unity's `MeshCollider.sharedMesh`), imported but never drawn, its vertices read in the
//! render sub-object's local space.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// `TriMesh` is exact (static geometry); `Convex` is a hull (cheaper, usable on dynamic bodies).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeshColliderKind {
    TriMesh,
    Convex,
}

impl MeshColliderKind {
    pub fn is_convex(self) -> bool {
        matches!(self, MeshColliderKind::Convex)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Written", into = "Written")]
pub struct SubObjectCollider {
    pub kind: MeshColliderKind,
    pub mesh: Option<String>,
}

impl SubObjectCollider {
    pub fn own(kind: MeshColliderKind) -> Self {
        Self { kind, mesh: None }
    }

    pub fn for_owner(self, owner: &str) -> Self {
        Self {
            mesh: self.mesh.filter(|m| m != owner),
            ..self
        }
    }
}

pub(super) fn read_map<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, SubObjectCollider>, D::Error> {
    let map = BTreeMap::<String, SubObjectCollider>::deserialize(d)?;
    let owned = map.into_iter().map(|(k, c)| {
        let c = c.for_owner(&k);
        (k, c)
    });
    Ok(owned.collect())
}

/// Whether naming `mesh` as `owner`'s collision mesh closes a loop, which would leave a whole-model
/// spawn drawing none of them.
pub fn closes_loop(map: &BTreeMap<String, SubObjectCollider>, owner: &str, mesh: &str) -> bool {
    let mut at = mesh;
    for _ in 0..=map.len() {
        if at == owner {
            return true;
        }
        match map.get(at).and_then(|c| c.mesh.as_deref()) {
            Some(next) => at = next,
            None => return false,
        }
    }
    false
}

pub fn looped(map: &BTreeMap<String, SubObjectCollider>) -> Vec<String> {
    let looping = |(k, c): (&String, &SubObjectCollider)| {
        let mesh = c.mesh.as_deref()?;
        closes_loop(map, k, mesh).then(|| k.clone())
    };
    map.iter().filter_map(looping).collect()
}

pub(super) fn break_loops(map: &mut BTreeMap<String, SubObjectCollider>) -> Vec<String> {
    let looped = looped(map);
    for k in &looped {
        if let Some(c) = map.get_mut(k) {
            c.mesh = None;
        }
    }
    looped
}

static WARNED: std::sync::Mutex<std::collections::BTreeSet<std::path::PathBuf>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());

pub(super) fn warn_loop(source: &std::path::Path, looped: &[String]) {
    let mut warned = WARNED.lock().unwrap_or_else(|e| e.into_inner());
    if warned.insert(source.to_path_buf()) {
        log::warn!(
            "[Import] {}: the collision meshes of {} name each other; each builds from its own mesh",
            super::meta_path(source).display(),
            looped.join(", ")
        );
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Written {
    Kind(MeshColliderKind),
    Full {
        kind: MeshColliderKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mesh: Option<String>,
    },
}

impl From<Written> for SubObjectCollider {
    fn from(w: Written) -> Self {
        match w {
            Written::Kind(kind) => Self::own(kind),
            // An empty name is no name: the sub-object's own mesh.
            Written::Full { kind, mesh } => Self {
                kind,
                mesh: mesh.filter(|m| !m.is_empty()),
            },
        }
    }
}

impl From<SubObjectCollider> for Written {
    fn from(c: SubObjectCollider) -> Self {
        match c.mesh {
            None => Written::Kind(c.kind),
            mesh => Written::Full { kind: c.kind, mesh },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> BTreeMap<String, SubObjectCollider> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn both_forms_read_and_the_bare_one_writes_back_bare() {
        let map = parse(r#"{"A": "TriMesh", "B": {"kind": "Convex", "mesh": "B_col"}}"#);
        assert_eq!(map["A"], SubObjectCollider::own(MeshColliderKind::TriMesh));
        assert_eq!(map["B"].mesh.as_deref(), Some("B_col"));
        assert!(map["B"].kind.is_convex());
        let written = serde_json::to_string(&map).unwrap();
        assert_eq!(
            written,
            r#"{"A":"TriMesh","B":{"kind":"Convex","mesh":"B_col"}}"#
        );
    }

    #[test]
    fn an_object_without_a_mesh_is_its_own() {
        let map = parse(r#"{"A": {"kind": "TriMesh"}, "B": {"kind": "TriMesh", "mesh": ""}}"#);
        assert_eq!(map["A"].mesh, None);
        assert_eq!(map["B"].mesh, None, "an empty name is none");
    }

    #[test]
    fn a_mesh_naming_its_own_sub_object_is_its_own() {
        let json = r#"{"colliders": {"Chair": {"kind": "TriMesh", "mesh": "Chair"}}}"#;
        let settings: super::super::ImportSettings = serde_json::from_str(json).unwrap();
        let own = SubObjectCollider::own(MeshColliderKind::TriMesh);
        assert_eq!(settings.colliders["Chair"], own);
    }

    #[test]
    fn a_loop_of_collision_meshes_is_found_and_broken() {
        let mut map = parse(
            r#"{"A": {"kind": "TriMesh", "mesh": "B"}, "B": {"kind": "TriMesh", "mesh": "A"},
                "C": {"kind": "TriMesh", "mesh": "D"}, "D": "TriMesh"}"#,
        );
        assert!(closes_loop(&map, "D", "C"), "D naming C closes C's link");
        assert!(!closes_loop(&map, "C", "D"));
        assert_eq!(break_loops(&mut map), ["A", "B"]);
        assert_eq!(
            (map["A"].mesh.as_ref(), map["B"].mesh.as_ref()),
            (None, None)
        );
        assert_eq!(map["C"].mesh.as_deref(), Some("D"), "a chain is no loop");
    }

    #[test]
    fn a_bad_entry_is_an_error() {
        let bad = serde_json::from_str::<BTreeMap<String, SubObjectCollider>>(r#"{"A": "Box"}"#);
        assert!(bad.is_err());
    }
}
