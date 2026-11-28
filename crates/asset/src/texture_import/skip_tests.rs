use image::{Rgba, RgbaImage};

use super::*;

fn blocks(png: &Path, skip: u32) -> Compressed {
    let settings = crate::sidecar::texture(png);
    let loaded = load_skipping(png, &settings, TextureKind::Colour, true, skip).unwrap();
    match loaded.texels {
        Texels::Blocks(blocks) => blocks,
        Texels::Image(_) => panic!("the refresh made a compressed copy"),
    }
}

#[test]
fn a_skipping_read_starts_at_a_lower_level_with_the_same_blocks() {
    let dir = rusty_test_support::temp::dir();
    let png = dir.join("tile.png");
    let image = RgbaImage::from_fn(64, 32, |x, y| Rgba([(x * 4) as u8, (y * 8) as u8, 9, 255]));
    image.save(&png).unwrap();
    refresh(&png, "").unwrap();
    let whole = blocks(&png, 0);
    assert_eq!((whole.width, whole.height, whole.levels.len()), (64, 32, 7));
    let two = blocks(&png, 2);
    assert_eq!((two.width, two.height), (16, 8));
    assert_eq!(
        two.levels[..],
        whole.levels[2..],
        "the lower levels, unchanged"
    );
    let most = blocks(&png, 99);
    assert_eq!(
        (most.width, most.height, most.levels.len()),
        (8, 4, 4),
        "a skip stops at the last level of whole 4×4 blocks"
    );
}

#[test]
fn max_skip_keeps_whole_blocks_and_one_level() {
    assert_eq!(max_skip(64, 32, 7), 3, "4 high is the last whole height");
    assert_eq!(max_skip(1024, 1024, 11), 8, "down to 4×4");
    assert_eq!(max_skip(1024, 1024, 3), 2, "never past the chain");
    assert_eq!(max_skip(4, 4, 3), 0, "a one-block texture keeps its top");
    assert_eq!(max_skip(12, 8, 4), 0, "6 wide is no whole block");
}

#[test]
fn the_variance_table_moves_up_with_the_first_level() {
    let table: variance::VarianceTable = std::array::from_fn(|i| i as u8 * 10);
    let shifted = encode::shift_variance(table, 3);
    assert_eq!(
        shifted[0], 30,
        "index 0 is the new level 1: the old level 4"
    );
    assert_eq!(shifted[15], 150, "past the end, the last level repeats");
}
