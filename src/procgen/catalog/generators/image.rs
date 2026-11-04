//! The `image` op: an image file as a source (#1491).

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::generators;
use crate::procgen::recipe::{choice, op, req, ImageEncoding, Lit, OpInfo, OpParam, SOURCE};

/// An image file as a source (#1491), Blender's *Image Texture* node. `path` is
/// project-relative; the caller reads and decodes the file
/// ([`crate::procgen::Sources`]), since procgen reads no files. It is resampled
/// bilinearly, wrapped, to the recipe's resolution; its tiling is the file's own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageTexture {
    pub path: String,
    #[serde(default)]
    pub encoding: ImageEncoding,
}

impl Row for ImageTexture {
    const INFO: OpInfo = op(
        "image",
        "generator",
        SOURCE,
        &[
            req("path", "path"),
            OpParam {
                default: Some(Lit::Text("srgb")),
                ..choice("encoding", &["srgb", "linear"])
            },
        ],
    );
}

impl Op for ImageTexture {
    fn run(&self, cx: &Cx) -> Image {
        let source = cx
            .sources
            .get(&self.path)
            .expect("the runner checks every image source");
        generators::image(source, self.encoding, cx.size)
    }
}
