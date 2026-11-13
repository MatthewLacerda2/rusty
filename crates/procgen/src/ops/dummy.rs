//! The test-only `dummy` op's math (#1751): kept outside its op file to show how
//! math in `ops/` names an enum the op declares in its own file, by its catalog path
//! (`catalog::<family>::<op>::<Enum>`).

use crate::catalog::color::dummy::DummyOutput;
use crate::image_buf::Image;

/// `a` or `b` (per `output`) where `mask`'s red is above 0.5 (everywhere, with no
/// mask), and the other input elsewhere.
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
