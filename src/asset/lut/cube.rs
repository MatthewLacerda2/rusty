//! Adobe's Cube LUT format (Cube LUT Specification 1.0): keyword lines, then the
//! table, one `r g b` line per texel, red fastest. `#` starts a comment line, blank
//! lines are skipped, and a keyword the spec does not name (a vendor's own) is
//! ignored, as Resolve and Photoshop ignore each other's. A leading UTF-8 byte-order
//! mark (Windows tools write one) is skipped.

use super::Lut;

/// What went wrong, and on which 1-based line.
pub(super) type Error = (usize, String);

/// The table `text` holds.
pub(super) fn parse(text: &str) -> Result<Lut, Error> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut header = Header {
        size: None,
        domain: [[0.0; 3], [1.0; 3]],
        domain_lines: [0; 2],
    };
    let mut texels = Vec::new();
    let mut last = 0;
    for (i, raw) in text.lines().enumerate() {
        let (n, line) = (i + 1, raw.trim());
        last = n;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with(|c: char| c.is_ascii_alphabetic()) {
            if !texels.is_empty() {
                return Err((n, format!("`{line}` after the table starts")));
            }
            header.keyword(n, line)?;
            continue;
        }
        let Some((_, side)) = header.size else {
            return Err((n, "a table entry before LUT_3D_SIZE".into()));
        };
        if texels.len() == (side as usize).pow(3) {
            return Err((n, format!("more than the {side}³ entries")));
        }
        let [r, g, b] = numbers(n, line.split_whitespace())?;
        texels.push([r, g, b, 1.0]);
    }
    header.finish(texels, last.max(1))
}

/// What the keyword lines said: the size (with its line) and the domain (with the
/// lines that set its min and max; `0` for the default).
struct Header {
    size: Option<(usize, u32)>,
    domain: [[f32; 3]; 2],
    domain_lines: [usize; 2],
}

impl Header {
    /// Read keyword line `n`.
    fn keyword(&mut self, n: usize, line: &str) -> Result<(), Error> {
        let mut words = line.split_whitespace();
        match words.next().unwrap_or_default() {
            "LUT_3D_SIZE" => self.size = Some((n, side(n, words)?)),
            "LUT_1D_SIZE" => return Err((n, "a 1D LUT; only 3D LUTs are read".into())),
            "DOMAIN_MIN" => (self.domain[0], self.domain_lines[0]) = (numbers(n, words)?, n),
            "DOMAIN_MAX" => (self.domain[1], self.domain_lines[1]) = (numbers(n, words)?, n),
            "LUT_3D_INPUT_RANGE" => {
                let [lo, hi]: [f32; 2] = numbers(n, words)?;
                (self.domain, self.domain_lines) = ([[lo; 3], [hi; 3]], [n; 2]);
            }
            _ => {}
        }
        Ok(())
    }

    /// The table, once every line is read (`last` the file's last line).
    fn finish(self, texels: Vec<[f32; 4]>, last: usize) -> Result<Lut, Error> {
        let Some((at, side)) = self.size else {
            return Err((last, "no LUT_3D_SIZE".into()));
        };
        let (needs, found) = ((side as usize).pow(3), texels.len());
        if found != needs {
            let why = format!("LUT_3D_SIZE {side} needs {needs} entries, found {found}");
            return Err((at, why));
        }
        let domain = self.domain;
        if (0..3).any(|c| domain[1][c] <= domain[0][c]) {
            // At the line that flipped it: the max's when one was given, else the min's.
            let [min, max] = self.domain_lines;
            let line = if max > 0 { max } else { min };
            return Err((line, "DOMAIN_MAX must be above DOMAIN_MIN".into()));
        }
        Ok(Lut {
            size: side,
            domain,
            texels,
        })
    }
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
