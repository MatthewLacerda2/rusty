//! src/ui/layout/native.rs — texture native sizes for layout (#421, #1221).
//!
//! An `Image`'s preferred size is its texture's size in texels (one texel is one
//! reference unit, as the renderer draws it). The sim never reads a file for it:
//! [`TextureSizes`] is a table the platform fills before every tick — the window
//! shell and the headless harness through `render::TextureSizeReader`, which reads
//! each file's header and follows the engine's own re-bakes (`core::fs::written`)
//! — and every layout reads it through the [`UiView`](crate::ui::UiView), beside
//! the screen size. The renderer draws with the same table, so what is drawn is
//! what is clicked. Like the screen size it is a **sim input**.
//!
//! **A size appears at the next tick** (Unity's content-size fitting, applied
//! late): an Image created, or a texture baked, during a tick measures what the
//! table held when the tick began. A path with no entry (not baked yet,
//! unreadable, a render texture) measures zero, as an Image with no texture does;
//! a miss is never stored as a size.

use std::collections::BTreeMap;

use glam::Vec2;

/// Texture path → native size in texels, as the platform last read it (a sim
/// input; see the module docs). Shared, not copied: the resource, every
/// [`UiView`](crate::ui::UiView) and the renderer hold one `Rc` of it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextureSizes {
    sizes: BTreeMap<String, Vec2>,
}

impl TextureSizes {
    /// The size of the texture at `path`, when the platform knows it.
    pub fn get(&self, path: &str) -> Option<Vec2> {
        self.sizes.get(path).copied()
    }

    /// Record `path`'s size in texels.
    pub fn set(&mut self, path: impl Into<String>, size: Vec2) {
        self.sizes.insert(path.into(), size);
    }

    /// Forget `path`'s size.
    pub fn remove(&mut self, path: &str) {
        self.sizes.remove(path);
    }

    /// Every path the table holds a size for.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.sizes.keys().map(String::as_str)
    }
}
