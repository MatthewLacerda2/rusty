//! crates/asset/src/sidecar/collider.rs — one sub-object's mesh-collider entry in a model's
//! sidecar (#77): its shape, and the sub-object whose geometry builds it (#1960).
//!
//! The entry is written as a bare kind when the collider builds from the sub-object's
//! own mesh (`"Chair": "TriMesh"`), and as an object when another sub-object of the
//! same file is its collision mesh (`"Chair": { "kind": "TriMesh", "mesh":
//! "Chair_col" }`), Unity's `MeshCollider.sharedMesh` pointed at a low-poly copy.
//! The collision sub-object is imported for its geometry and never drawn: a whole
//! model spawns no entity for it (`scene::model_instance`). Its vertices are read in
//! the render sub-object's local space, so it shares that object's origin, as a
//! Blender duplicate with a Decimate modifier does.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// How a sub-object's baked mesh collider is shaped (#77). `TriMesh` is an exact
/// triangle mesh (static geometry); `Convex` is a convex hull (cheaper, usable on
/// dynamic bodies). A sub-object absent from the sidecar map gets no collider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeshColliderKind {
    TriMesh,
    Convex,
}

impl MeshColliderKind {
    /// `true` when this kind bakes a convex hull rather than a triangle mesh.
    pub fn is_convex(self) -> bool {
        matches!(self, MeshColliderKind::Convex)
    }
}

/// A sub-object's collider entry: its kind, and the sub-object of the same file
/// whose geometry builds it (`None`: its own).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Written", into = "Written")]
pub struct SubObjectCollider {
    pub kind: MeshColliderKind,
    pub mesh: Option<String>,
}

impl SubObjectCollider {
    /// A collider built from the sub-object's own mesh.
    pub fn own(kind: MeshColliderKind) -> Self {
        Self { kind, mesh: None }
    }

    /// This entry as the sub-object `owner` reads it: a `mesh` naming `owner` itself
    /// means its own mesh, so it never names a collision sub-object that is hidden.
    pub fn for_owner(self, owner: &str) -> Self {
        Self {
            mesh: self.mesh.filter(|m| m != owner),
            ..self
        }
    }
}

/// Read a sidecar's `colliders` map, each entry [`SubObjectCollider::for_owner`] its
/// key: `"Chair": {"kind": "TriMesh", "mesh": "Chair"}` is `"Chair": "TriMesh"`.
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

/// Whether naming `mesh` as `owner`'s collision mesh closes a loop: following each
/// entry's `mesh` from `mesh` leads back to `owner` (`"A": {mesh: "B"}, "B": {mesh:
/// "A"}`). Every sub-object of a loop names another as its collision mesh, so a
/// whole-model spawn would draw none of them.
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

/// The sub-objects of `map` whose collision mesh closes a loop ([`closes_loop`]).
pub fn looped(map: &BTreeMap<String, SubObjectCollider>) -> Vec<String> {
    let looping = |(k, c): (&String, &SubObjectCollider)| {
        let mesh = c.mesh.as_deref()?;
        closes_loop(map, k, mesh).then(|| k.clone())
    };
    map.iter().filter_map(looping).collect()
}

/// Read every entry of a loop as its own mesh, so the loop hides nothing; returns
/// the sub-objects it changed. What a hand-edited sidecar's loop reads as
/// (`sidecar::load`, warned once).
pub(super) fn break_loops(map: &mut BTreeMap<String, SubObjectCollider>) -> Vec<String> {
    let looped = looped(map);
    for k in &looped {
        if let Some(c) = map.get_mut(k) {
            c.mesh = None;
        }
    }
    looped
}

/// Files whose collision-mesh loop has been warned of.
static WARNED: std::sync::Mutex<std::collections::BTreeSet<std::path::PathBuf>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());

/// Say once per file that its sidecar's collision meshes loop through `looped`.
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

/// The two ways an entry is written: the bare kind, or the kind with its mesh.
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
