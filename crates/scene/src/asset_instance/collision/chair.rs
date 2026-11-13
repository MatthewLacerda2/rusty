//! The collision-mesh model (#1960) the collision tests import: a bumpy `Chair` and
//! its low-poly `Chair_col`. A test hook: compiled in every build, since the
//! scripting tests that import it are another crate's.

use std::path::Path;

use rusty_asset::{sidecar, MeshColliderKind, SubObjectCollider};

/// Write a bumpy `Chair` of `n`×`n` quads (2n² triangles, 0.1 high) and a
/// two-triangle `Chair_col` as the `.obj` at `path`, its sidecar's `Chair`
/// collider built from `mesh`.
pub fn write(path: &Path, n: u32, mesh: Option<&str>) -> Result<(), String> {
    let mut obj = String::from("o Chair\n");
    for z in 0..=n {
        for x in 0..=n {
            let y = if (x + z) % 2 == 1 { 0.1 } else { 0.0 };
            obj += &format!("v {} {y} {}\n", x as f32 / n as f32, z as f32 / n as f32);
        }
    }
    let at = |x: u32, z: u32| z * (n + 1) + x + 1;
    for z in 0..n {
        for x in 0..n {
            let (a, b, c, d) = (at(x, z), at(x + 1, z), at(x + 1, z + 1), at(x, z + 1));
            obj += &format!("f {a} {b} {c}\nf {a} {c} {d}\n");
        }
    }
    let base = (n + 1) * (n + 1);
    obj += "o Chair_col\nv 0 0 0\nv 1 0 0\nv 1 0.5 1\nv 0 0.5 1\n";
    obj += &format!("f {} {} {}\n", base + 1, base + 2, base + 3);
    obj += &format!("f {} {} {}\n", base + 1, base + 3, base + 4);
    rusty_core::fs::written::write(path, obj.as_bytes()).map_err(|e| e.to_string())?;
    let mut settings = sidecar::ImportSettings::default();
    let entry = SubObjectCollider {
        kind: MeshColliderKind::TriMesh,
        mesh: mesh.map(str::to_string),
    };
    settings.colliders.insert("Chair".into(), entry);
    sidecar::save(path, &settings).map_err(|e| e.to_string())
}
