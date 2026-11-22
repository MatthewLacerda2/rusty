use serde::{Deserialize, Serialize};

use super::path::{mask, FillRule};
use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::generators::clamp_or;
use crate::ops::outline::text::{glyphs, Layout};
use crate::recipe::{choice, op, opt, req, Lit, OpInfo, OpParam, SOURCE};

// The longest text in bytes, the most lines and the largest em in tiles: the figures
// `docs/api/Texture.md`'s `text` bullet states.
const MAX_BYTES: usize = 4096;
const MAX_LINES: usize = 256;
const MAX_SIZE: f32 = 4.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Written")]
pub struct Text {
    pub text: String,
    pub font: String,
    pub size: f32,
    pub align: Align,
    pub line_spacing: f32,
    pub tracking: f32,
}

#[derive(Deserialize)]
struct Written {
    text: String,
    #[serde(default)]
    font: String,
    #[serde(default = "size")]
    size: f32,
    #[serde(default)]
    align: Align,
    #[serde(default = "line_spacing")]
    line_spacing: f32,
    #[serde(default)]
    tracking: f32,
}

impl TryFrom<Written> for Text {
    type Error = String;

    fn try_from(w: Written) -> Result<Self, String> {
        if w.text.len() > MAX_BYTES {
            return Err(format!(
                "`text` is {} bytes, past the {MAX_BYTES} a text takes",
                w.text.len()
            ));
        }
        if w.text.split('\n').count() > MAX_LINES {
            return Err(format!("`text` has past {MAX_LINES} lines"));
        }
        Ok(Self {
            text: w.text,
            font: w.font,
            size: w.size,
            align: w.align,
            line_spacing: w.line_spacing,
            tracking: w.tracking,
        })
    }
}

impl Row for Text {
    const INFO: OpInfo = op(
        "text",
        "generator",
        SOURCE,
        &[
            req("text", "text"),
            opt("font", "path", Lit::Text("")),
            opt("size", "number", Lit::Number(0.12)),
            OpParam {
                default: Some(Lit::Text("center")),
                ..choice("align", &["left", "center", "right"])
            },
            opt("line_spacing", "number", Lit::Number(1.2)),
            opt("tracking", "number", Lit::Number(0.0)),
        ],
    );
}

impl Op for Text {
    fn run(&self, cx: &Cx) -> Image {
        let font = match self.font.as_str() {
            "" => rusty_font::default_font(),
            path => cx
                .sources
                .font(path)
                .expect("the runner checks every font source"),
        };
        let layout = Layout {
            size: clamp_or(self.size, size(), 0.0, MAX_SIZE),
            tracking: clamp_or(self.tracking, 0.0, -MAX_SIZE, MAX_SIZE),
            line_spacing: clamp_or(self.line_spacing, line_spacing(), 0.0, 16.0),
            align: self.align,
        };
        if layout.size <= 0.0 {
            return Image::filled(cx.size, [0.0, 0.0, 0.0, 1.0]);
        }
        mask(&glyphs(&font, &self.text, layout), cx, FillRule::Nonzero)
    }
}

fn size() -> f32 {
    0.12
}

fn line_spacing() -> f32 {
    1.2
}
