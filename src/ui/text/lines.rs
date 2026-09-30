//! src/ui/text/lines.rs — measured items → lines: wrapping and overflow (#419).
//!
//! **Wrapping** is greedy: a line breaks after the last whitespace that keeps it
//! within the width; a word wider than the whole width breaks between characters.
//! Whitespace that ends a wrapped line is not counted in its width, and a soft
//! break drops the whitespace it lands on. `\n` always breaks.
//!
//! **Overflow** keeps the lines that fit the rect's height entirely (`Truncate`,
//! `Ellipsis`); an unwrapped line wider than the rect loses the characters past its
//! edge, and `Ellipsis` ends the last kept line — or any cut line — with `…`.

use super::measure::{Item, LineMetrics, Measurer};
use crate::components::TextOverflow;

/// Slack for float noise when comparing a measure against the rect.
pub const EPS: f32 = 1e-3;

/// One laid-out line.
#[derive(Clone, Debug, Default)]
pub struct Line {
    pub items: Vec<Item>,
    pub metrics: LineMetrics,
}

impl Line {
    fn new(items: Vec<Item>, empty: LineMetrics) -> Self {
        let metrics = items
            .iter()
            .map(|i| i.metrics)
            .reduce(LineMetrics::max)
            .unwrap_or(empty);
        Self { items, metrics }
    }

    /// The pen position after each item, in order (kerning within the line only).
    pub fn pens(&self) -> impl Iterator<Item = (f32, &Item)> {
        let mut pen = 0.0;
        self.items.iter().enumerate().map(move |(j, it)| {
            let start = pen + if j > 0 { it.kern } else { 0.0 };
            pen = start + it.advance;
            (start, it)
        })
    }

    /// The full advance, trailing spaces included — where a caret after the line sits.
    pub fn advance(&self) -> f32 {
        self.pens().last().map_or(0.0, |(x, it)| x + it.advance)
    }

    /// The visible width: up to the end of the last non-whitespace item.
    pub fn width(&self) -> f32 {
        self.pens()
            .filter(|(_, it)| !it.is_space())
            .map(|(x, it)| x + it.advance)
            .last()
            .unwrap_or(0.0)
    }
}

/// Break `items` into lines, wrapping at `max_width` when given.
pub fn break_lines(items: &[Item], max_width: Option<f32>, empty: LineMetrics) -> Vec<Line> {
    let mut out = Vec::new();
    for paragraph in items.split(|it| it.ch == '\n') {
        let mut line: Vec<Item> = Vec::new();
        let mut last_space: Option<usize> = None;
        let mut soft = false;
        for it in paragraph {
            let right = end_pen(&line) + if line.is_empty() { 0.0 } else { it.kern } + it.advance;
            let over = max_width.is_some_and(|w| right > w + EPS);
            if over && !it.is_space() && !line.is_empty() {
                let tail = match last_space.take() {
                    Some(b) => line.split_off(b + 1),
                    None => Vec::new(),
                };
                out.push(Line::new(std::mem::take(&mut line), empty));
                line = tail;
                soft = true;
            }
            if it.is_space() {
                if line.is_empty() && soft {
                    continue; // a soft break swallows the space it lands on
                }
                last_space = Some(line.len());
            }
            line.push(*it);
        }
        out.push(Line::new(line, empty));
    }
    out
}

/// The pen position after the last of `items` (trailing whitespace included).
fn end_pen(items: &[Item]) -> f32 {
    items
        .iter()
        .enumerate()
        .map(|(j, it)| if j > 0 { it.kern } else { 0.0 } + it.advance)
        .sum()
}

/// The block height of `lines` at `spacing`: the first ascent, each line-to-line
/// pitch (descent + gap + next ascent, scaled by `spacing`) and the last descent.
pub fn block_height(lines: &[Line], spacing: f32) -> f32 {
    let (Some(first), Some(last)) = (lines.first(), lines.last()) else {
        return 0.0;
    };
    let pitches: f32 = lines.windows(2).map(|w| pitch(&w[0], &w[1], spacing)).sum();
    first.metrics.ascent + pitches + last.metrics.descent
}

/// The baseline-to-baseline distance from `a` to the line after it, `b`.
pub fn pitch(a: &Line, b: &Line, spacing: f32) -> f32 {
    (a.metrics.descent + a.metrics.gap + b.metrics.ascent) * spacing
}

/// Apply `overflow` to `lines` inside a `width` × `height` rect. Returns whether
/// anything was cut.
pub fn clip(
    lines: &mut Vec<Line>,
    overflow: TextOverflow,
    (width, height): (f32, f32),
    spacing: f32,
    measurer: &Measurer,
) -> bool {
    if overflow == TextOverflow::Overflow {
        return false;
    }
    let before = lines.len();
    let kept = (0..lines.len())
        .take_while(|&i| block_height(&lines[..=i], spacing) <= height + EPS)
        .count();
    lines.truncate(kept);
    let cut_vertically = kept < before;
    let ellipsis = overflow == TextOverflow::Ellipsis;
    let mut cut = cut_vertically;
    let last = lines.len().saturating_sub(1);
    for (i, line) in lines.iter_mut().enumerate() {
        let too_wide = line.width() > width + EPS;
        let end_mark = ellipsis && (too_wide || (cut_vertically && i == last));
        if too_wide || end_mark {
            cut |= too_wide;
            shorten(line, width, end_mark, measurer);
        }
    }
    cut
}

/// Drop items from the end of `line` until it (plus an ellipsis, when `mark`) fits
/// `width`, then append the ellipsis. A line nothing fits in keeps its height.
fn shorten(line: &mut Line, width: f32, mark: bool, measurer: &Measurer) {
    let Some(style) = line.items.iter().rev().find(|i| !i.is_space()).copied() else {
        return;
    };
    let dots = if mark {
        measurer.ellipsis(&style)
    } else {
        Vec::new()
    };
    let height = line.metrics;
    let mut items = std::mem::take(&mut line.items);
    loop {
        if items.last().is_some_and(Item::is_space) {
            items.pop();
            continue;
        }
        let candidate = Line::new([items.as_slice(), &dots].concat(), height);
        if candidate.width() <= width + EPS {
            *line = candidate;
            break;
        }
        if items.pop().is_none() {
            break;
        }
    }
    line.metrics = height.max(line.metrics);
}
