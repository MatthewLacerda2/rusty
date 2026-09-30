//! src/ui/text/rich.rs — the rich-text tag subset → styled characters (#419).
//!
//! `<color=#rrggbb>` / `<color=#rrggbbaa>`, `<b>`, `<i>` and `<size=n>`, each closed
//! by its `</…>` tag, nesting freely (a close pops the innermost open one). A tag
//! outside the subset, a malformed one, or a close with nothing open is kept as
//! literal text — nothing the author typed silently disappears.

use glam::Vec4;

/// The style of one character.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    /// Display-space fill, straight alpha.
    pub color: Vec4,
    pub bold: bool,
    pub italic: bool,
    /// A `<size>` override in reference units (before auto-size scaling).
    pub size: Option<f32>,
}

/// Split `text` into characters with their style. `rich` off keeps tags literal.
pub fn parse(text: &str, rich: bool, base_color: Vec4) -> Vec<(char, Style)> {
    let base = Style {
        color: base_color,
        bold: false,
        italic: false,
        size: None,
    };
    if !rich {
        return text.chars().map(|c| (c, base)).collect();
    }
    let mut state = State::default();
    let mut out = Vec::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if c == '<' {
            if let Some(end) = rest.find('>') {
                if state.apply(&rest[1..end], base.color) {
                    rest = &rest[end + 1..];
                    continue;
                }
            }
        }
        out.push((c, state.style(base)));
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// The open tags.
#[derive(Default)]
struct State {
    colors: Vec<Vec4>,
    sizes: Vec<f32>,
    bold: u32,
    italic: u32,
}

impl State {
    fn style(&self, base: Style) -> Style {
        Style {
            color: self.colors.last().copied().unwrap_or(base.color),
            bold: self.bold > 0,
            italic: self.italic > 0,
            size: self.sizes.last().copied(),
        }
    }

    /// Apply the tag body between `<` and `>`; `false` when it is not a known,
    /// well-formed tag (it then stays literal).
    fn apply(&mut self, tag: &str, base_color: Vec4) -> bool {
        let lower = tag.to_ascii_lowercase();
        match lower.as_str() {
            "b" => self.bold += 1,
            "i" => self.italic += 1,
            "/b" => return pop_count(&mut self.bold),
            "/i" => return pop_count(&mut self.italic),
            "/color" => return self.colors.pop().is_some(),
            "/size" => return self.sizes.pop().is_some(),
            _ => {
                if let Some(v) = lower.strip_prefix("color=") {
                    let Some(c) = parse_hex(v.trim_matches('"'), base_color.w) else {
                        return false;
                    };
                    self.colors.push(c);
                } else if let Some(v) = lower.strip_prefix("size=") {
                    match v.trim_matches('"').parse::<f32>() {
                        Ok(n) if n.is_finite() && n > 0.0 => self.sizes.push(n),
                        _ => return false,
                    }
                } else {
                    return false;
                }
            }
        }
        true
    }
}

fn pop_count(n: &mut u32) -> bool {
    let open = *n > 0;
    *n = n.saturating_sub(1);
    open
}

/// `#rrggbb` (keeping `alpha`) or `#rrggbbaa` → display-space RGBA in `[0, 1]`.
pub fn parse_hex(hex: &str, alpha: f32) -> Option<Vec4> {
    let digits = hex.strip_prefix('#')?;
    if !matches!(digits.len(), 6 | 8) || !digits.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    let a = if digits.len() == 8 {
        f32::from(byte(6)?) / 255.0
    } else {
        alpha
    };
    Some(Vec4::new(
        f32::from(byte(0)?) / 255.0,
        f32::from(byte(2)?) / 255.0,
        f32::from(byte(4)?) / 255.0,
        a,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(chars: &[(char, Style)]) -> String {
        chars.iter().map(|(c, _)| *c).collect()
    }

    #[test]
    fn tags_style_their_runs_and_vanish() {
        let s = parse(
            "a<b>b<i>c</i></b><color=#ff0000>d</color><size=50>e</size>",
            true,
            Vec4::ONE,
        );
        assert_eq!(text(&s), "abcde");
        assert!(!s[0].1.bold && s[1].1.bold && s[2].1.bold && s[2].1.italic);
        assert!(!s[3].1.bold, "</b> closed");
        assert_eq!(s[3].1.color, Vec4::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(s[4].1.color, Vec4::ONE, "</color> restores");
        assert_eq!(s[4].1.size, Some(50.0));
    }

    #[test]
    fn unknown_malformed_and_unmatched_tags_stay_literal() {
        let s = parse("<u>x</u> <color=red>y</b> 1 < 2", true, Vec4::ONE);
        assert_eq!(text(&s), "<u>x</u> <color=red>y</b> 1 < 2");
        let off = parse("<b>z</b>", false, Vec4::ONE);
        assert_eq!(text(&off), "<b>z</b>", "rich text off");
    }

    #[test]
    fn hex_colours_keep_or_set_alpha() {
        assert_eq!(
            parse_hex("#00ff00", 0.5),
            Some(Vec4::new(0.0, 1.0, 0.0, 0.5))
        );
        assert_eq!(
            parse_hex("#0000ff80", 1.0).map(|c| c.w),
            Some(128.0 / 255.0)
        );
        assert_eq!(parse_hex("00ff00", 1.0), None);
        assert_eq!(parse_hex("#zzzzzz", 1.0), None);
    }
}
