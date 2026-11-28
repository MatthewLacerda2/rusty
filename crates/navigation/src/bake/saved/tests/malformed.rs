use super::super::super::super::NavigationGraph;
use super::super::file::decode;
use super::super::first_baked_span;
use super::{level, saved};

#[test]
fn every_truncation_is_refused() {
    let (_, bytes) = saved(&level().0);
    assert!(decode(&bytes).is_ok());
    for len in (0..bytes.len()).step_by(7) {
        assert!(decode(&bytes[..len]).is_err(), "truncated at {len}");
    }
    let mut long = bytes.clone();
    long.push(0);
    assert_eq!(decode(&long).unwrap_err(), "trailing bytes");
}

#[test]
fn a_nan_a_bad_area_or_a_wrong_format_is_refused() {
    let (scene, _) = level();
    let (_, bytes) = saved(&scene);
    let at = first_baked_span(&bytes).expect("a span");
    let refused = |patch: &dyn Fn(&mut Vec<u8>)| {
        let mut b = bytes.clone();
        patch(&mut b);
        let mut g = NavigationGraph::from_scene(&scene);
        let before = g.spans.len();
        assert!(g.restore(&scene, &b).is_err());
        assert_eq!(g.spans.len(), before, "nothing was installed");
    };
    refused(&|b| b[at..at + 4].copy_from_slice(&f32::NAN.to_le_bytes()));
    refused(&|b| b[at + 8] = 200);
    refused(&|b| b[at + 9] = 9);
    assert!(bytes[at + 9] < 8, "the first span is a level floor");
    refused(&|b| b[at + 10..at + 14].copy_from_slice(&(-1.0f32).to_le_bytes()));
    refused(&|b| b[4] = 9);
    refused(&|b| b[0] = b'X');
    refused(&|b| b[16..20].copy_from_slice(&f32::NAN.to_le_bytes()));
}

#[test]
fn no_single_byte_flip_panics() {
    let (scene, _) = level();
    let (_, bytes) = saved(&scene);
    let mut g = NavigationGraph::from_scene(&scene);
    for i in 0..bytes.len() {
        let mut b = bytes.clone();
        b[i] ^= 0xA5;
        let _ = g.restore(&scene, &b);
    }
}

#[test]
fn a_link_end_shifted_off_the_lattice_is_refused_not_a_panic() {
    let (scene, _) = level();
    let (g, _) = saved(&scene);
    let mut link = g
        .offmesh_links()
        .next()
        .expect("the level has links")
        .clone();
    link.to.gx = i32::MAX;
    assert!(super::super::shifted(&link, (1, 0)).is_none());
    assert!(super::super::shifted(&link, (-1, 0)).is_some());
}
