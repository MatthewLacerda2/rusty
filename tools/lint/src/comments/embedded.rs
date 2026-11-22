use crate::code::{walk, Open, Piece};

/// A comment opener a file the string is written to would read: `#`, `//` or `--`.
const OPENERS: &[&str] = &["# ", "// ", "//! ", "/// ", "-- "];

/// The comment lines Rust string literals carry for another file (a table's `#` header, a
/// Lua chunk's `--` lines), by 0-based source line, each with the line its literal opens on,
/// where an inline allow above the literal reaches it.
pub(super) fn lines(source: &str) -> Vec<(usize, usize, String)> {
    let mut open = Open::None;
    let mut opened = 0;
    let mut found = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let mut continued = matches!(open, Open::Escaped | Open::Raw(_));
        walk(line, &mut open, |piece| {
            let Piece::Str { body, .. } = piece else {
                return;
            };
            if !std::mem::take(&mut continued) {
                opened = i;
            }
            for text in line[body].split("\\n") {
                let text = text.trim_start();
                if OPENERS.iter().any(|o| text.starts_with(o)) {
                    found.push((i, opened, text.to_string()));
                }
            }
        });
    }
    found
}
