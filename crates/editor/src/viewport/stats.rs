//! The Game view's **Stats** overlay (#2205), Unity's Game-view Stats: the frame's
//! timings and counters over the last second, over the top-right of the image.
//!
//! It reads the table `Debug.Stats{ window = STATS_WINDOW }` returns, which the
//! shell hands in as a [`Readout`] ([`crate::EditorUi::stats`]) only while the
//! toggle is on, so the operator sees what an agent reads and a closed overlay
//! costs nothing. A build without the `dev` layer records no stats: the overlay
//! shows the window's own clock and says the rest needs one.

use rusty_core::frame_stats::{FrameStats, Series};

/// Drawn frames the overlay summarises: about one second at 60 Hz.
pub const STATS_WINDOW: usize = 60;

/// GPU pass kinds listed, heaviest first.
const PASSES: usize = 4;

/// The window's clock as the shell's frame loop reads it: FPS over the last whole
/// second, and the last frame's ms. What the overlay shows without a table.
#[derive(Clone, Copy, Debug, Default)]
pub struct Clock {
    pub fps: f32,
    pub frame_ms: f32,
}

impl Clock {
    /// A steady clock at `fps` frames a second (a headless capture's).
    pub fn at(fps: f32) -> Self {
        let frame_ms = 1000.0 / fps.max(f32::MIN_POSITIVE);
        Self { fps, frame_ms }
    }
}

impl From<(f32, f32)> for Clock {
    fn from((fps, frame_ms): (f32, f32)) -> Self {
        Self { fps, frame_ms }
    }
}

/// What the overlay shows this frame: the windowed frame stats (`None` without the
/// `dev` layer, which records them) and the window's clock.
#[derive(Clone, Debug, Default)]
pub struct Readout {
    pub table: Option<FrameStats>,
    pub clock: Clock,
}

impl Readout {
    /// Its lines ([`lines`]).
    pub fn lines(&self) -> Vec<String> {
        lines(self.table.as_ref(), self.clock)
    }
}

/// The overlay's lines, top to bottom: what [`paint`] draws, kept apart from egui
/// so a test can read them.
pub fn lines(stats: Option<&FrameStats>, clock: Clock) -> Vec<String> {
    let Some(stats) = stats else {
        return vec![
            format!("{:.0} FPS   {:.2} ms", clock.fps, clock.frame_ms),
            "Render, sim and GPU times need a dev build".to_string(),
        ];
    };
    let avg = |key: &str| stats.get(key).map(|s| s.avg);
    let ms = |v: Option<f64>| v.map_or("—".to_string(), |v| format!("{v:.2} ms"));
    let fps = avg("fps").unwrap_or(f64::from(clock.fps));
    let frame = avg("wall_ms").unwrap_or(f64::from(clock.frame_ms).max(0.0));
    let last = |key: &str| stats.get(key).map_or("—".to_string(), |s| count(s.last));
    let mut out = vec![
        format!("{fps:.0} FPS   frame {frame:.2} ms"),
        format!("Sim (per tick)   {}", ms(avg("frame_ms"))),
        format!("Render CPU   {}", ms(avg("renderer_ms"))),
    ];
    out.push(match avg("gpu_ms") {
        Some(gpu) => format!("GPU   {gpu:.2} ms"),
        None if stats.drawn_frames == 0 => "GPU   — (nothing drawn yet)".to_string(),
        None => "GPU   not measured: this adapter has no timestamps".to_string(),
    });
    out.push(format!(
        "Draw calls {}   Tris {}",
        last("draw_calls"),
        last("triangles")
    ));
    let mut passes: Vec<(&String, &Series)> = stats.gpu_passes.iter().collect();
    passes.sort_by(|a, b| b.1.avg.total_cmp(&a.1.avg));
    for (name, s) in passes.into_iter().take(PASSES) {
        out.push(format!("  {name}   {:.2} ms", s.avg));
    }
    out
}

/// A count as a whole number with thousands grouped: `1,234,567`.
fn count(v: f64) -> String {
    let digits = format!("{:.0}", v.max(0.0));
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Paint the overlay's `lines` in a dark box at the top-right of `image`.
pub fn paint(ui: &egui::Ui, image: egui::Rect, lines: &[String], t: crate::theme::Theme) {
    let font = egui::FontId::monospace(12.0);
    let painter = ui.painter_at(image);
    let galleys: Vec<_> = lines
        .iter()
        .map(|line| painter.layout_no_wrap(line.clone(), font.clone(), t.text_primary))
        .collect();
    let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let height: f32 = galleys.iter().map(|g| g.size().y).sum();
    let pad = t.space_sm;
    let size = egui::vec2(width, height) + egui::vec2(pad, pad) * 2.0;
    let min = egui::pos2(image.max.x - size.x - pad, image.min.y + pad);
    let rect = egui::Rect::from_min_size(min, size);
    painter.rect_filled(rect, 4.0, egui::Color32::from_black_alpha(190));
    let mut at = rect.min + egui::vec2(pad, pad);
    for galley in galleys {
        let height = galley.size().y;
        painter.galley(at, galley, t.text_primary);
        at.y += height;
    }
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod stats_tests;
