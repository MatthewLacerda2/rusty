//! crates/ui/src/text/lines.rs — measured items → lines: wrapping and overflow.

use super::measure::{Item, LineMetrics, Measurer};
use rusty_components::TextOverflow;

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

    /// The pen position before each item, in order.
    pub fn pens(&self) -> impl Iterator<Item = (f32, &Item)> {
        let mut pen = 0.0;
        self.items.iter().map(move |it| {
            let start = pen;
            pen += it.advance;
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
        for (i, it) in paragraph.iter().enumerate() {
            let right = end_pen(&line) + it.advance;
            let over = max_width.is_some_and(|w| right > w + EPS);
            if over && !it.is_space() && !it.joins && !line.is_empty() {
                let tail = match last_space.take() {
                    Some(b) => line.split_off(b + 1),
                    None => {
                        if let Some(last) = line.last_mut() {
                            last.cut();
                        }
                        Vec::new()
                    }
                };
                out.push(Line::new(std::mem::take(&mut line), empty));
                line = tail;
                soft = true;
            }
            let breakable = it.is_space() && !paragraph.get(i + 1).is_some_and(|n| n.joins);
            if breakable {
                if line.is_empty() && soft {
                    continue;
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
    items.iter().map(|it| it.advance).sum()
}

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
        let mut kept = items.clone();
        if let Some(last) = kept.last_mut() {
            last.cut();
        }
        let candidate = Line::new([kept.as_slice(), &dots].concat(), height);
        if candidate.width() <= width + EPS {
            *line = candidate;
            break;
        }
        if items.is_empty() {
            break;
        }
        while items.pop().is_some_and(|it| it.joins) {}
    }
    line.metrics = height.max(line.metrics);
}
