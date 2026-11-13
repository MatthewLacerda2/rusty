//! A malformed file is refused whole, never a panic (#1887).

use super::super::super::super::NavigationGraph;
use super::super::file::decode;
use super::{level, saved};

/// Byte offset of the first span's floor in the baked layer.
fn first_span(bytes: &[u8]) -> usize {
    let starts = u32::from_le_bytes(bytes[44..48].try_into().unwrap()) as usize;
    48 + starts * 4 + 4
}

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
    let at = first_span(&bytes);
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
    // A negative floor depth would make `Surface::height` clamp backwards.
    assert!(bytes[at + 9] < 8, "the first span is a level floor");
    refused(&|b| b[at + 10..at + 14].copy_from_slice(&(-1.0f32).to_le_bytes()));
    refused(&|b| b[4] = 9);
    refused(&|b| b[0] = b'X');
    refused(&|b| b[16..20].copy_from_slice(&f32::NAN.to_le_bytes()));
}

/// Flipping any one byte decodes or is refused; it never panics.
#[test]
fn no_single_byte_flip_panics() {
    let (scene, _) = level();
    let (_, bytes) = saved(&scene);
    for i in (0..bytes.len()).step_by(3) {
        let mut b = bytes.clone();
        b[i] ^= 0xA5;
        let mut g = NavigationGraph::from_scene(&scene);
        let _ = g.restore(&scene, &b);
    }
}
