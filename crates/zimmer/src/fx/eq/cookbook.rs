//! The cookbook's numerator and denominator for each band kind, before normalising by `a0`.

use crate::patch::EqKind;

pub(super) fn coefficients(kind: EqKind, cos: f32, alpha: f32, a: f32) -> ([f32; 3], [f32; 3]) {
    match kind {
        EqKind::HighPass => (
            [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        ),
        EqKind::LowPass => (
            [(1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        ),
        EqKind::Peak => (
            [1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a],
            [1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a],
        ),
        EqKind::LowShelf => {
            let shared = 2.0 * a.sqrt() * alpha;
            (
                [
                    a * ((a + 1.0) - (a - 1.0) * cos + shared),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                    a * ((a + 1.0) - (a - 1.0) * cos - shared),
                ],
                [
                    (a + 1.0) + (a - 1.0) * cos + shared,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                    (a + 1.0) + (a - 1.0) * cos - shared,
                ],
            )
        }
        EqKind::HighShelf => {
            let shared = 2.0 * a.sqrt() * alpha;
            (
                [
                    a * ((a + 1.0) + (a - 1.0) * cos + shared),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                    a * ((a + 1.0) + (a - 1.0) * cos - shared),
                ],
                [
                    (a + 1.0) - (a - 1.0) * cos + shared,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos),
                    (a + 1.0) - (a - 1.0) * cos - shared,
                ],
            )
        }
    }
}
