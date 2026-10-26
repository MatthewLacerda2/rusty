//! The test-only dummy effect (#1467): a scene pass that draws nothing, and its doc
//! section, which no shipped doc holds.

use super::{Owner, Pass, PassCx, PostEffect};
use crate::render::{RenderView, Renderer};

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "dummy",
    owner: Owner::Volume,
    option: None,
    module: None,
    composite: &[],
    grade: &[],
    pass: Some(Pass::Scene { order: 90, run }),
};

fn run(_: &Renderer, _: &mut RenderView, _: &PassCx) {}

/// Its section, as `docs/api/Graphics.md` would hold it.
pub(super) const DOC: &str = "<!-- effect: dummy -->
#### Dummy

Draws nothing. Tween paths: `Graphics.dummy.strength`.";
