//! src/ui/layout/native.rs — texture native sizes for layout (#421, #1221).
//!
//! An `Image`'s preferred size is its texture's size in texels (one texel is one
//! reference unit, as the renderer draws it). The sim never reads a file for it:
//! [`TextureSizes`] is a table the platform fills — the shell and the headless
//! harness through `render::refresh_texture_sizes`, which reads each file's header
//! and follows the engine's own re-bakes (`core::fs::written`) — and the layout
//! reads it through the [`UiView`](crate::ui::UiView), beside the screen size.
//! Like the screen size it is a **sim input**: a test or a harness may set it.
//!
//! A path with no entry (not baked yet, unreadable, a render texture) measures
//! zero, as an Image with no texture does. A miss is never stored, so the size
//! appears the frame the file does.

use std::collections::BTreeMap;

use glam::Vec2;

/// Texture path → native size in texels, as the platform last read it (a sim
/// input; see the module docs).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextureSizes {
    sizes: BTreeMap<String, Vec2>,
    /// The platform's write-log generation the table is current as of. The sim
    /// never reads it; it lets the filler drain only what changed since.
    generation: u64,
}

impl TextureSizes {
    /// An empty table.
    pub const fn new() -> Self {
        Self {
            sizes: BTreeMap::new(),
            generation: 0,
        }
    }

    /// The size of the texture at `path`, when the platform knows it.
    pub fn get(&self, path: &str) -> Option<Vec2> {
        self.sizes.get(path).copied()
    }

    /// Record `path`'s size in texels.
    pub fn set(&mut self, path: impl Into<String>, size: Vec2) {
        self.sizes.insert(path.into(), size);
    }

    /// Keep only the entries whose path `keep` accepts.
    pub fn retain(&mut self, mut keep: impl FnMut(&str) -> bool) {
        self.sizes.retain(|path, _| keep(path));
    }

    /// The platform's write-log generation (see the field docs).
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Record the write-log generation the table now reflects.
    pub fn set_generation(&mut self, generation: u64) {
        self.generation = generation;
    }
}
