//! The `<scene>.occlusion` file (#1714): a bake comes back byte for byte, and a
//! truncated, out-of-range or self-contradicting file is refused whole.

use super::test_level::corridors;
use super::{bake_occlusion, decode, encode};

fn bytes() -> Vec<u8> {
    encode(&bake_occlusion(&corridors(), 2).0.expect("occluders"))
}

#[test]
fn a_bake_reads_back_as_it_was_written() {
    let data = bake_occlusion(&corridors(), 2).0.unwrap();
    assert_eq!(decode(&encode(&data)), Ok(data));
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
    let objects = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let cell_at = 8 + objects * 13 + 12;
    let mut nan = bytes.clone();
    nan[cell_at..cell_at + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(decode(&nan).is_err());
    let mut row = bytes.clone();
    let last = row.len() - 4;
    row[last..].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(decode(&row), Err("rows disagree with the grid".into()));
    let mut flag = bytes;
    flag[20] = 2; // the first object's occluder flag
    assert!(decode(&flag).is_err());
}
