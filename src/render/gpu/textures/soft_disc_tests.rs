//! The default particle sprite's texels (#2052): white, opaque at the centre,
//! falling monotonically to transparent at the rim and in the corners.

use super::{soft_disc, SOFT_DISC_SIDE};

#[test]
fn soft_disc_is_white_and_fades_from_centre_to_rim() {
    let side = SOFT_DISC_SIDE;
    let disc = soft_disc(side);
    assert!(disc.pixels().all(|p| p.0[..3] == [255; 3]), "white");
    let half = side / 2;
    assert!(
        disc.get_pixel(half, half).0[3] >= 250,
        "opaque at the centre"
    );
    assert_eq!(disc.get_pixel(0, 0).0[3], 0, "a corner is outside the disc");
    assert_eq!(disc.get_pixel(0, half).0[3], 0, "the rim is transparent");
    let row: Vec<u8> = (half..side).map(|x| disc.get_pixel(x, half).0[3]).collect();
    assert!(
        row.windows(2).all(|w| w[0] >= w[1]),
        "falls outward: {row:?}"
    );
    assert!(
        row.iter().any(|&a| a > 0 && a < 255),
        "a soft edge, not a cut"
    );
}

#[test]
fn soft_disc_is_symmetric() {
    let side = SOFT_DISC_SIDE;
    let disc = soft_disc(side);
    for (x, y, p) in disc.enumerate_pixels() {
        assert_eq!(p, disc.get_pixel(side - 1 - x, y), "mirrored in x");
        assert_eq!(p, disc.get_pixel(y, x), "mirrored on the diagonal");
    }
}
