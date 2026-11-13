//! crates/asset/src/import_cache/hulls/tests.rs — a hull sidecar keeps its hulls bit
//! for bit, refuses a malformed file whole, and follows its model's meshes (#1940).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use super::file::{encode, parse};
use super::{load, sidecar_of};
use rusty_core::hulls::{self, Hull, Key, Pieces, DECOMPOSER};

fn tet(x: f32) -> Hull {
    let points = vec![
        [x, -0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.5],
    ];
    let triangles = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
    Hull { points, triangles }
}

fn entries() -> BTreeMap<Key, Pieces> {
    let mut e = BTreeMap::new();
    e.insert((7, 4, 12), Arc::new(vec![tet(0.1), tet(f32::MIN_POSITIVE)]));
    e.insert((9, 3, 3), Pieces::default()); // no usable decomposition
    e
}

fn bits(e: &BTreeMap<Key, Pieces>) -> Vec<(Key, Vec<u32>, Vec<[u32; 3]>)> {
    let hull = |k: &Key, h: &Hull| {
        let p = h.points.iter().flatten().map(|c| c.to_bits()).collect();
        (*k, p, h.triangles.clone())
    };
    (e.iter())
        .flat_map(|(k, p)| p.iter().map(move |h| hull(k, h)))
        .collect()
}

#[test]
fn a_sidecar_reads_back_the_same_bits() {
    let read = parse(&encode(&entries())).expect("parses");
    assert_eq!(bits(&read), bits(&entries()));
    assert_eq!(
        read.keys().collect::<Vec<_>>(),
        entries().keys().collect::<Vec<_>>()
    );
    assert!(read[&(9, 3, 3)].is_empty(), "an empty entry stays empty");
}

#[test]
fn a_malformed_sidecar_is_refused_whole() {
    let good = encode(&entries());
    for cut in 0..good.len() {
        assert!(
            parse(&good[..cut]).is_err(),
            "cut at {cut} of {}",
            good.len()
        );
    }
    let mut longer = good.clone();
    longer.push(0);
    assert!(parse(&longer).is_err(), "trailing bytes");
    assert!(parse(b"GLTF\0\0\0\0").is_err(), "another file");
    // Past the header (magic, fingerprint, entry count), the first entry's 28
    // bytes (its hull count last) and the first hull's point count.
    let header = 4 + 4 + DECOMPOSER.len() + 4;
    let point = header + 28 + 4;
    let mut nan = good.clone();
    nan[point..point + 4].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
    assert!(parse(&nan).unwrap_err().contains("finite"));
    let mut index = good.clone();
    let tri = point + 48 + 4;
    index[tri..tri + 4].copy_from_slice(&4u32.to_le_bytes());
    assert!(parse(&index).unwrap_err().contains("index 4"));
    let mut count = good.clone();
    count[point - 4..point].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(parse(&count).unwrap_err().contains("count"));
    let mut other = good;
    other[8] ^= 1; // the fingerprint's first byte
    assert!(parse(&other).unwrap_err().contains("another decomposer"));
}

/// A model of its own in a fresh folder: the quad `.obj` raised to `y`, so no
/// other test shares its mesh (the stores are process-wide), and its quad's key.
fn quad_model(y: f32) -> (PathBuf, Key) {
    let dir = rusty_test_support::temp::dir();
    let path = dir.join("quad.obj");
    let obj = rusty_test_support::models::TRIANGLE_OBJ.replace(" 0.0 -1.0", &format!(" {y} -1.0"));
    std::fs::write(&path, obj).unwrap();
    let import = crate::imported(&path).expect("imports");
    let sub = &import.sub_meshes[0];
    let sidecar = sidecar_of(&path).unwrap();
    std::fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(&sidecar);
    let positions = sub.vertices.iter().map(|v| v.position);
    (path, hulls::key(positions, &sub.indices))
}

#[cfg(any(feature = "editor", feature = "dev"))]
#[test]
fn a_fresh_decomposition_is_written_beside_its_model_and_stale_entries_go() {
    let (path, quad) = quad_model(19.401);
    load(&path, &crate::imported(&path).unwrap());
    let sidecar = sidecar_of(&path).unwrap();
    let stale: Key = (1, 2, 3);
    let mut old = BTreeMap::new();
    old.insert(stale, Pieces::default());
    std::fs::write(&sidecar, encode(&old)).unwrap();
    hulls::keep_fresh(quad, Arc::new(vec![tet(0.25)]));
    super::save_pending();
    // Whichever call took the entry wrote it before this one returned.
    let written = std::fs::read(&sidecar).unwrap();
    let read = parse(&written).unwrap();
    assert_eq!(read.keys().copied().collect::<Vec<_>>(), vec![quad]);
    assert_eq!(read[&quad][0], tet(0.25));
    super::save_pending();
    assert_eq!(std::fs::read(&sidecar).unwrap(), written, "nothing new");
}

#[test]
fn an_import_keeps_the_sidecar_entries_of_its_own_meshes_only() {
    let (path, quad) = quad_model(19.402);
    let other: Key = (0x1940, 5, 5);
    let mut file = BTreeMap::new();
    file.insert(quad, Arc::new(vec![tet(0.5)]));
    file.insert(other, Arc::new(vec![tet(0.75)]));
    std::fs::write(sidecar_of(&path).unwrap(), encode(&file)).unwrap();
    load(&path, &crate::imported(&path).unwrap());
    assert_eq!(hulls::get(quad).unwrap()[0], tet(0.5));
    assert!(
        hulls::get(other).is_none(),
        "a mesh the model lacks is not kept"
    );
    // A file that will not parse keeps nothing and does not fail the import.
    std::fs::write(sidecar_of(&path).unwrap(), b"RHUL\x01").unwrap();
    load(&path, &crate::imported(&path).unwrap());
}
