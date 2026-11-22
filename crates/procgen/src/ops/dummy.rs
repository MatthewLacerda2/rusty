use crate::catalog::color::dummy::DummyOutput;
use crate::image_buf::Image;

pub fn keep(a: &Image, b: &Image, mask: Option<&Image>, output: DummyOutput) -> Image {
    let (keep, other) = match output {
        DummyOutput::A => (a, b),
        DummyOutput::B => (b, a),
    };
    let mut out = keep.clone();
    if let Some(mask) = mask {
        out.map_indexed(|i, p| match mask.pixels()[i][0] > 0.5 {
            true => p,
            false => other.pixels()[i],
        });
    }
    out
}
