//! Test support for the filters (#1743): run one op on one image, and roll an image
//! by whole pixels, so a test can show an op wraps (rolling its input rolls its
//! output) and holds its look across resolutions.

use crate::procgen::image_buf::{Image, Rgba};
use crate::procgen::ops::eval_node;
use crate::procgen::recipe::OpKind;
use crate::procgen::source::Sources;

/// `op` run on `img` alone, at its size, seed 0.
pub fn run(op: &OpKind, img: &Image) -> Image {
    eval_node(op, &[img], img.size(), 0, &Sources::new())
}

/// `img` shifted `dx` right and `dy` down, wrapping.
pub fn rolled(img: &Image, dx: i64, dy: i64) -> Image {
    let mut out = Image::new(img.size());
    out.for_each_row(|y, row| {
        for (x, p) in row.iter_mut().enumerate() {
            *p = img.get_wrapped(x as i64 - dx, y as i64 - dy);
        }
    });
    out
}

/// A busy, tiling height at `n²`: a sine across times a cosine down, two whole
/// periods across and three down, in red (green and blue are functions of it).
pub fn relief(n: u32) -> Image {
    Image::fill_uv(n, |u, v| {
        let tau = std::f32::consts::TAU;
        let h = 0.5 + 0.25 * (tau * 2.0 * u).sin() * (tau * 3.0 * v).cos();
        [h, h * h, 1.0 - h, 1.0]
    })
}

/// Assert `op` wraps: its output on `img` rolled is its output on `img`, rolled.
pub fn assert_wraps(op: &OpKind, img: &Image) {
    let (a, b) = (run(op, &rolled(img, 5, 3)), rolled(&run(op, img), 5, 3));
    assert_close(&a, &b, 1e-4, op.tag());
}

/// Every channel of `a` within `tol` of `b`.
pub fn assert_close(a: &Image, b: &Image, tol: f32, what: &str) {
    for (i, (p, q)) in a.pixels().iter().zip(b.pixels()).enumerate() {
        let off = (0..4).any(|c| (p[c] - q[c]).abs() > tol);
        assert!(!off, "{what}: pixel {i}: {p:?} vs {q:?}");
    }
}

/// The mean of channel `c`.
pub fn mean(img: &Image, c: usize) -> f32 {
    img.pixels().iter().map(|p: &Rgba| p[c]).sum::<f32>() / img.pixels().len() as f32
}
