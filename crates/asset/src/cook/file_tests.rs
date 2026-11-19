//! A cooked file keeps its values bit for bit and refuses a malformed or stale one
//! whole (#2332).

use std::sync::Arc;

use super::file::{encode, parse, STALE};
use super::sub::Cooked;
use crate::clusters::{Cluster, Clusters, NO_CONE};

fn subs() -> Vec<Cooked> {
    let cluster = Cluster {
        indices: 0..3,
        min: [-0.0, 1.0, f32::MIN_POSITIVE],
        max: [1.0, 2.0, 3.0],
        apex: [0.5, 0.25, 0.125],
        axis: [0.0, 1.0, 0.0],
        cutoff: NO_CONE,
    };
    let split = Cooked {
        id: "Split".into(),
        parse: 0x0123_4567_89ab_cdef,
        source: Some(vec![0, 1, 2, 2]),
        positions: vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        tangents: vec![[1.0, -0.0, 0.0, -1.0]; 4],
        lightmap_uvs: vec![[0.1, 0.2], [0.3, 0.4], [0.5, 0.6], [0.7, 1.0e-30]],
        indices: vec![0, 1, 3],
        clusters: Some(Arc::new(Clusters {
            list: vec![cluster],
        })),
    };
    let plain = Cooked {
        id: "Plain".into(),
        parse: u64::MAX,
        source: None,
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, -0.0, 2.5]],
        tangents: vec![[0.0, 0.0, 1.0, 1.0]; 3],
        lightmap_uvs: vec![[0.0; 2]; 3],
        indices: vec![2, 1, 0],
        clusters: None,
    };
    let empty = Cooked {
        id: "Empty".into(),
        parse: 0,
        source: None,
        positions: Vec::new(),
        tangents: Vec::new(),
        lightmap_uvs: Vec::new(),
        indices: Vec::new(),
        clusters: None,
    };
    vec![split, plain, empty]
}

#[test]
fn a_cooked_file_reads_back_bit_for_bit() {
    let bytes = encode(7, &subs());
    let back = parse(&bytes, 7).unwrap();
    assert_eq!(back, subs());
    // `PartialEq` takes -0 for 0; the printed form does not.
    assert_eq!(format!("{back:?}"), format!("{:?}", subs()));
    assert_eq!(
        encode(7, &back),
        bytes,
        "the same values write the same file"
    );
}

#[test]
fn a_model_of_only_empty_sub_meshes_reads_back() {
    // A glTF of only lines or points, or an `.obj` object with no faces.
    let empty = subs()[2].clone();
    let both = [
        empty.clone(),
        Cooked {
            id: "Also".into(),
            ..empty
        },
    ];
    for subs in [&both[..1], &both[..]] {
        let bytes = encode(7, subs);
        assert_eq!(
            parse(&bytes, 7).as_deref(),
            Ok(subs),
            "{} empty",
            subs.len()
        );
    }
    assert_eq!(
        parse(&encode(7, &[]), 7),
        Ok(Vec::new()),
        "no sub-mesh at all"
    );
}

#[test]
fn it_is_a_glb_the_gltf_crate_reads() {
    // The empty sub-mesh aside: its count-0 accessors are the file's one
    // departure from strict glTF (the module docs).
    let bytes = encode(7, &subs()[..2]);
    let gltf = gltf::Gltf::from_slice(&bytes).expect("a glTF");
    let names: Vec<_> = gltf
        .meshes()
        .map(|m| m.name().unwrap().to_string())
        .collect();
    assert_eq!(names, ["Split", "Plain"]);
}

#[test]
fn another_stamp_or_cooker_version_is_stale() {
    let bytes = encode(7, &subs());
    let e = parse(&bytes, 8).unwrap_err();
    assert!(e.starts_with(STALE), "{e}");
    let version = format!("\"cooker\":{}", super::stamp::COOKER_VERSION);
    // The other version differs by one, so it prints in as many digits.
    let other = super::stamp::COOKER_VERSION ^ 1;
    let older = replace(&bytes, &version, &format!("\"cooker\":{other}"));
    let e = parse(&older, 7).unwrap_err();
    assert!(
        e.starts_with(STALE) && e.contains(&format!("version {other}")),
        "{e}"
    );
}

#[test]
fn a_malformed_file_is_refused_whole() {
    let bytes = encode(7, &subs());
    for cut in [0, 11, 20, bytes.len() / 2, bytes.len() - 1] {
        let e = parse(&bytes[..cut], 7).unwrap_err();
        assert!(!e.starts_with(STALE), "{cut}: {e}");
    }
    assert!(parse(b"not a glb at all", 7).is_err());
    // A count past its view: the accessor no longer fits its bytes.
    let bad = replace(&bytes, "\"count\":4", "\"count\":5");
    let e = parse(&bad, 7).unwrap_err();
    assert!(e.contains("not 5 elements"), "{e}");
}

/// `bytes` with the first `from` (as long as `to`) replaced by `to`.
fn replace(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    assert_eq!(from.len(), to.len());
    let at = (bytes.windows(from.len()))
        .position(|w| w == from.as_bytes())
        .unwrap_or_else(|| panic!("no {from}"));
    let mut out = bytes.to_vec();
    out[at..at + to.len()].copy_from_slice(to.as_bytes());
    out
}
