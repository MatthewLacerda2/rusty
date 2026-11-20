//! The `<scene>.occlusion` file (#1714): a bake comes back byte for byte, and a
//! truncated, out-of-range or self-contradicting file is refused whole.

use crate::occlusion::bake_occlusion;
use crate::occlusion::test_level::corridors;
use rusty_scene::lighting::occlusion::{decode, encode};

fn bytes() -> Vec<u8> {
    encode(&bake_occlusion(&corridors(), 2).0.expect("occluders"))
}

/// Where the objects start: the magic, the stamp (version, never culled, floor
/// source, baked cells, reach hash, bounds), the count.
const OBJECTS_AT: usize = 56;
/// Where the stamp's floor source, baked cells and bounds are.
const FLOOR_AT: usize = 12;
const BAKED_AT: usize = 16;
const REACH_AT: usize = 20;
const BOUNDS_AT: usize = 28;

fn object_count(bytes: &[u8]) -> usize {
    u32::from_le_bytes(bytes[52..56].try_into().unwrap()) as usize
}

#[test]
fn a_bake_reads_back_as_it_was_written() {
    let data = bake_occlusion(&corridors(), 2).0.unwrap();
    assert_eq!(data.stamp.version, crate::occlusion::OCCLUSION_VERSION);
    assert_eq!(decode(&encode(&data)), Ok(data));
}

#[test]
fn a_never_culled_count_past_the_objects_is_refused() {
    let mut bytes = bytes();
    let past = object_count(&bytes) as u32 + 1;
    bytes[8..12].copy_from_slice(&past.to_le_bytes());
    assert!(decode(&bytes).unwrap_err().contains("never culled"));
}

#[test]
fn a_stamp_out_of_range_is_refused() {
    let bytes = bytes();
    let mut floor = bytes.clone();
    floor[FLOOR_AT..FLOOR_AT + 4].copy_from_slice(&2u32.to_le_bytes());
    assert_eq!(decode(&floor), Err("floor source out of range".into()));
    let mut baked = bytes.clone();
    baked[BAKED_AT..BAKED_AT + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    let err = decode(&baked).unwrap_err();
    assert!(err.contains("more cells baked"), "{err}");
    let mut nan = bytes;
    nan[BOUNDS_AT + 4..BOUNDS_AT + 8].copy_from_slice(&f32::NAN.to_le_bytes());
    assert_eq!(decode(&nan), Err("bounds not finite".into()));
}

#[test]
fn a_stamp_sampling_every_cell_with_a_reach_or_fewer_cells_is_refused() {
    let bytes = bytes();
    let mut reach = bytes.clone();
    reach[REACH_AT..REACH_AT + 8].copy_from_slice(&7u64.to_le_bytes());
    let mut fewer = bytes.clone();
    let baked = u32::from_le_bytes(bytes[BAKED_AT..BAKED_AT + 4].try_into().unwrap());
    fewer[BAKED_AT..BAKED_AT + 4].copy_from_slice(&(baked - 1).to_le_bytes());
    for refused in [reach, fewer] {
        let err = decode(&refused).unwrap_err();
        assert!(err.contains("sampling every cell"), "{err}");
    }
}

#[test]
fn a_truncated_or_padded_file_is_refused() {
    let bytes = bytes();
    for cut in [0, 3, 4, 9, bytes.len() / 2, bytes.len() - 1] {
        assert!(decode(&bytes[..cut]).is_err(), "cut at {cut}");
    }
    let mut padded = bytes.clone();
    padded.push(0);
    assert!(decode(&padded).is_err());
}

#[test]
fn a_nan_cell_or_a_row_past_the_rows_is_refused() {
    let bytes = bytes();
    let cell_at = OBJECTS_AT + object_count(&bytes) * 13 + 12;
    let mut nan = bytes.clone();
    nan[cell_at..cell_at + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(decode(&nan).is_err());
    let mut row = bytes.clone();
    let last = row.len() - 4;
    row[last..].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(decode(&row), Err("rows disagree with the grid".into()));
    let mut flag = bytes;
    flag[OBJECTS_AT + 12] = 2; // the first object's occluder flag
    assert!(decode(&flag).is_err());
}

#[test]
fn dims_whose_product_overflows_are_refused_not_a_panic() {
    let mut bytes = bytes();
    let dims_at = OBJECTS_AT + object_count(&bytes) * 13 + 16;
    for k in 0..3 {
        let at = dims_at + k * 4;
        bytes[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    }
    assert_eq!(decode(&bytes), Err("grid out of range".into()));
}

#[test]
fn another_versions_file_is_refused_by_its_version_first() {
    let version = rusty_scene::lighting::occlusion::OCCLUSION_VERSION;
    let mut older = bytes();
    older[4..8].copy_from_slice(&(version - 1).to_le_bytes());
    older[FLOOR_AT..FLOOR_AT + 4].copy_from_slice(&9u32.to_le_bytes()); // unread
    let err = decode(&older).unwrap_err();
    let want = format!(
        "an older occlusion bake (version {}, this build writes {version})",
        version - 1
    );
    assert!(err.starts_with(&want), "{err}");
    assert!(
        err.contains("re-baked (Generate Lighting or Bake Occlusion)"),
        "{err}"
    );
    let mut newer = bytes();
    newer[4..8].copy_from_slice(&(version + 1).to_le_bytes());
    assert!(decode(&newer)
        .unwrap_err()
        .starts_with("a newer occlusion bake"));
    assert_eq!(
        rusty_scene::lighting::occlusion::version_of(&older),
        Some(version - 1)
    );
}
