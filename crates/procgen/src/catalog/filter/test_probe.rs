use crate::image_buf::{Image, Rgba};
use crate::ops::eval_node;
use crate::recipe::OpKind;
use crate::source::Sources;

pub fn run(op: &OpKind, img: &Image) -> Image {
    eval_node(op, &[img], img.size(), 0, &Sources::new())
}

pub fn rolled(img: &Image, dx: i64, dy: i64) -> Image {
    let mut out = Image::new(img.size());
    out.for_each_row(|y, row| {
        for (x, p) in row.iter_mut().enumerate() {
            *p = img.get_wrapped(x as i64 - dx, y as i64 - dy);
        }
    });
    out
}

pub fn relief(n: u32) -> Image {
    Image::fill_uv(n, |u, v| {
        let tau = std::f32::consts::TAU;
        let h = 0.5 + 0.25 * (tau * 2.0 * u).sin() * (tau * 3.0 * v).cos();
        [h, h * h, 1.0 - h, 1.0]
    })
}

pub fn assert_wraps(op: &OpKind, img: &Image) {
    let (a, b) = (run(op, &rolled(img, 5, 3)), rolled(&run(op, img), 5, 3));
    assert_close(&a, &b, 1e-4, op.tag());
}

pub fn assert_close(a: &Image, b: &Image, tol: f32, what: &str) {
    for (i, (p, q)) in a.pixels().iter().zip(b.pixels()).enumerate() {
        let off = (0..4).any(|c| (p[c] - q[c]).abs() > tol);
        assert!(!off, "{what}: pixel {i}: {p:?} vs {q:?}");
    }
}

pub fn mean(img: &Image, c: usize) -> f32 {
    img.pixels().iter().map(|p: &Rgba| p[c]).sum::<f32>() / img.pixels().len() as f32
}

pub fn ridged(n: u32) -> Image {
    let op: OpKind = serde_json::from_str(
        r#"{ "op": "noise", "kind": "ridged", "scale": 8, "octaves": 2, "seed": 3 }"#,
    )
    .expect("noise parses");
    eval_node(&op, &[], n.into(), 0, &Sources::new())
}
