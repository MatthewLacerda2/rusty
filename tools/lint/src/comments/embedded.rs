use crate::code::{walk, Open, Piece};

/// A comment opener a file the string is written to would read: `#`, `//` or `--`.
const OPENERS: &[&str] = &["# ", "// ", "//! ", "/// ", "-- "];

/// The comment lines Rust string literals carry for another file (a table's `#` header, a
/// Lua chunk's `--` lines), by 0-based source line.
pub(super) fn lines(source: &str) -> Vec<(usize, String)> {
    let mut open = Open::None;
    let mut found = Vec::new();
    for (i, line) in source.lines().enumerate() {
        walk(line, &mut open, |piece| {
            let Piece::Str { body, .. } = piece else {
                return;
            };
            for text in line[body].split("\\n") {
                let text = text.trim_start();
                if OPENERS.iter().any(|o| text.starts_with(o)) {
                    found.push((i, text.to_string()));
                }
            }
        });
    }
    found
}
