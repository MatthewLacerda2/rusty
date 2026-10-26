//! Adobe's Cube LUT format (Cube LUT Specification 1.0): keyword lines, then the
//! table, one `r g b` line per texel, red fastest. `#` starts a comment line, blank
//! lines are skipped, and a keyword the spec does not name (a vendor's own) is
//! ignored, as Resolve and Photoshop ignore each other's.

use super::Lut;

/// What went wrong, and on which 1-based line.
pub(super) type Error = (usize, String);

/// The table `text` holds.
pub(super) fn parse(text: &str) -> Result<Lut, Error> {
    let mut size: Option<(usize, u32)> = None;
    let mut domain = [[0.0; 3], [1.0; 3]];
    let mut texels = Vec::new();
    let mut last = 0;
    for (i, raw) in text.lines().enumerate() {
        let (n, line) = (i + 1, raw.trim());
        last = n;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.split_whitespace();
        let first = words.next().unwrap_or_default();
        if first.starts_with(|c: char| c.is_ascii_alphabetic()) {
            if !texels.is_empty() {
                return Err((n, format!("`{first}` after the table starts")));
            }
            match first {
                "LUT_3D_SIZE" => size = Some((n, side(n, words)?)),
                "LUT_1D_SIZE" => return Err((n, "a 1D LUT; only 3D LUTs are read".into())),
                "DOMAIN_MIN" => domain[0] = numbers(n, words)?,
                "DOMAIN_MAX" => domain[1] = numbers(n, words)?,
                "LUT_3D_INPUT_RANGE" => {
                    let [lo, hi]: [f32; 2] = numbers(n, words)?;
                    domain = [[lo; 3], [hi; 3]];
                }
                _ => {}
            }
            continue;
        }
        let Some((_, side)) = size else {
            return Err((n, "a table entry before LUT_3D_SIZE".into()));
        };
        if texels.len() == (side as usize).pow(3) {
            return Err((n, format!("more than the {}³ entries", side)));
        }
        let [r, g, b] = numbers(n, line.split_whitespace())?;
        texels.push([r, g, b, 1.0]);
    }
    let Some((at, side)) = size else {
        return Err((last.max(1), "no LUT_3D_SIZE".into()));
    };
    let needs = (side as usize).pow(3);
    if texels.len() != needs {
        let found = texels.len();
        return Err((
            at,
            format!("LUT_3D_SIZE {side} needs {needs} entries, found {found}"),
        ));
    }
    if (0..3).any(|c| domain[1][c] <= domain[0][c]) {
        return Err((at, "DOMAIN_MAX must be above DOMAIN_MIN".into()));
    }
    Ok(Lut {
        size: side,
        domain,
        texels,
    })
}

/// `LUT_3D_SIZE`'s value, within what the importer reads.
fn side<'a>(n: usize, mut words: impl Iterator<Item = &'a str>) -> Result<u32, Error> {
    let side = words
        .next()
        .and_then(|w| w.parse::<u32>().ok())
        .ok_or((n, "LUT_3D_SIZE needs a whole number".to_string()))?;
    if !Lut::fits(side) {
        let (lo, hi) = (Lut::MIN_SIZE, Lut::MAX_SIZE);
        return Err((n, format!("LUT_3D_SIZE {side} is outside {lo}–{hi}")));
    }
    Ok(side)
}

/// Exactly `N` finite numbers.
fn numbers<'a, const N: usize>(
    n: usize,
    words: impl Iterator<Item = &'a str>,
) -> Result<[f32; N], Error> {
    let values: Vec<f32> = words
        .map(|w| w.parse::<f32>().ok().filter(|v| v.is_finite()))
        .collect::<Option<_>>()
        .ok_or((n, format!("expected {N} numbers")))?;
    values
        .try_into()
        .map_err(|v: Vec<f32>| (n, format!("expected {N} numbers, found {}", v.len())))
}
