//! The chain's shape (#1700): how many mips a start size gets, and their sizes.

use super::{mip_count, mip_sizes, MAX_MIPS};

#[test]
fn a_view_gets_urps_count_capped_at_the_most() {
    // 1080p at half, a third and a quarter: each quality level starts smaller.
    assert_eq!(mip_count((960, 540)), MAX_MIPS);
    assert_eq!(mip_count((640, 360)), 8);
    assert_eq!(mip_count((480, 270)), 7);
    assert_eq!(mip_count((3840, 2160)), MAX_MIPS);
    assert_eq!(mip_count((64, 16)), 5);
}

#[test]
fn a_tiny_view_has_one_mip_and_never_a_zero_side() {
    for start in [(1, 1), (2, 1), (3, 3), (1, 700)] {
        let sizes = mip_sizes(start);
        assert!(!sizes.is_empty(), "{start:?}");
        assert_eq!(sizes[0], start);
        assert!(sizes.iter().all(|&(w, h)| w >= 1 && h >= 1), "{start:?}");
    }
    assert_eq!(mip_sizes((1, 1)), vec![(1, 1)]);
    assert_eq!(*mip_sizes((1, 700)).last().unwrap(), (1, 5));
}

#[test]
fn each_mip_is_half_the_one_above_rounded_down() {
    let sizes = mip_sizes((641, 359));
    for pair in sizes.windows(2) {
        assert_eq!(pair[1], (pair[0].0 / 2, pair[0].1 / 2));
    }
    assert_eq!(sizes.len(), 8);
}
