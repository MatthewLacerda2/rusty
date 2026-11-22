use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::generators;
use crate::recipe::{choice, op, req, ImageEncoding, Lit, OpInfo, OpParam, SOURCE};

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
