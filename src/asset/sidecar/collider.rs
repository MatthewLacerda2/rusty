//! src/asset/sidecar/collider.rs — one sub-object's mesh-collider entry in a model's
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
    use std::collections::BTreeMap;

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
    fn a_bad_entry_is_an_error() {
        let bad = serde_json::from_str::<BTreeMap<String, SubObjectCollider>>(r#"{"A": "Box"}"#);
        assert!(bad.is_err());
    }
}
