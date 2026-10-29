//! Warns when a texture is used as two kinds (#1416): colour and data, say.
//!
//! A texture keeps one mip chain and one compressed format, built for the
//! [`TextureKind`] of its first use (#1322, #1574). A file later requested as
//! another kind is averaged (or compressed) wrongly for that use, and which use wins depends on load order. Unity keeps one setting per
//! texture and warns on a mismatch; this does the same, once per file. Both the
//! material texture cache and the decal atlas hold one.

use std::collections::{HashMap, HashSet};

use super::TextureKind;

/// What each loaded file was uploaded as, and which have been warned about.
#[derive(Default)]
pub(crate) struct KindWatch {
    uploaded: HashMap<String, TextureKind>,
    warned: HashSet<String>,
}

impl KindWatch {
    /// `path` was just uploaded as `kind`: its first use decides.
    pub(crate) fn uploaded(&mut self, path: &str, kind: TextureKind) {
        self.uploaded.insert(path.to_owned(), kind);
    }

    /// `path`, already uploaded, is requested as `kind` by `user`. Warns once per
    /// file when `kind` differs from the upload; returns whether it warned.
    pub(crate) fn requested(&mut self, path: &str, kind: TextureKind, user: &str) -> bool {
        let Some(&first) = self.uploaded.get(path) else {
            return false;
        };
        if first == kind || !self.warned.insert(path.to_owned()) {
            return false;
        }
        log::warn!(
            "[Render] {path} is used as {} by an earlier {user} load and as {} now; it keeps one \
             upload (mip chain and compression), built for the first use, so the other \
             reads it wrongly",
            first.name(),
            kind.name(),
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_then_data_request_warns_once() {
        let mut watch = KindWatch::default();
        watch.uploaded("a.png", TextureKind::Colour);
        assert!(watch.requested("a.png", TextureKind::Data, "material"));
        assert!(!watch.requested("a.png", TextureKind::Data, "material"));
    }

    #[test]
    fn matching_and_unknown_requests_never_warn() {
        let mut watch = KindWatch::default();
        watch.uploaded("a.png", TextureKind::Data);
        assert!(!watch.requested("a.png", TextureKind::Data, "decal"));
        assert!(!watch.requested("b.png", TextureKind::Colour, "decal"));
    }

    #[test]
    fn each_file_warns_on_its_own() {
        let mut watch = KindWatch::default();
        watch.uploaded("a.png", TextureKind::Colour);
        watch.uploaded("b.png", TextureKind::Colour);
        assert!(watch.requested("a.png", TextureKind::Data, "decal"));
        assert!(watch.requested("b.png", TextureKind::Data, "decal"));
    }
}
