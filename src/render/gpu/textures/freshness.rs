//! src/render/gpu/textures/freshness.rs — keep the path-keyed texture cache in step
//! with the files on disk (#689), without a per-frame `stat` of every map.
//!
//! Two ways an upload goes stale, two cheap answers, both checked once per frame
//! before anything draws:
//!
//! - **The engine re-wrote the file** (`Texture.Bake`, `BakeSet`, `Material.Rebake`
//!   all go through one PNG writer, which notes the path in
//!   [`crate::procgen::written`]). The renderer drains the notes and drops the
//!   matching upload — colour and data views together, they are one `GpuTexture` —
//!   plus every material bind group, which still holds the old upload. Unity
//!   reimports on write; this is that.
//! - **The file was missing when first drawn.** A failed load is remembered as a
//!   *miss*, not cached as the default under its path, and misses are re-checked
//!   (`Path::exists`, one per missing path) every [`MISS_RETRY_FRAMES`] frames, so a
//!   map that appears later — baked, imported or copied in — is picked up.
//!
//! A file changed on disk by an outside tool at a path that already loaded is not
//! noticed: that would need the per-frame stat this module exists to avoid.

use std::collections::HashSet;
use std::path::Path;

use crate::render::Renderer;

/// How many frames apart missing texture paths are re-checked on disk.
pub(crate) const MISS_RETRY_FRAMES: u64 = 60;

/// What the renderer knows about the cache's freshness.
pub(crate) struct TextureFreshness {
    /// The newest write generation already applied to the cache.
    seen: u64,
    /// Paths whose load failed: drawn with the default until the file appears.
    pub(crate) misses: HashSet<String>,
    /// Frames since misses were last re-checked.
    frames: u64,
}

impl TextureFreshness {
    pub(crate) fn new() -> Self {
        Self {
            seen: crate::procgen::written::written_since(u64::MAX).0,
            misses: HashSet::new(),
            frames: 0,
        }
    }
}

impl Renderer {
    /// Drop every cached upload whose file was re-written or has appeared since the
    /// last frame. Called once per frame, before any texture is resolved.
    pub(crate) fn refresh_textures(&mut self) {
        let (seen, mut stale) = crate::procgen::written::written_since(self.texture_freshness.seen);
        self.texture_freshness.seen = seen;
        let freshness = &mut self.texture_freshness;
        freshness.frames += 1;
        if freshness.frames >= MISS_RETRY_FRAMES {
            freshness.frames = 0;
            stale.extend(
                freshness
                    .misses
                    .iter()
                    .filter(|p| Path::new(p).exists())
                    .cloned(),
            );
        }
        if !stale.is_empty() {
            self.invalidate_textures(&stale);
        }
    }

    /// Forget the uploads (and misses) of `paths`, so their next lookup reads the
    /// file again. A cached key naming the same file by another spelling (relative
    /// vs absolute) goes too.
    pub(crate) fn invalidate_textures(&mut self, paths: &[String]) {
        let written: Vec<_> = paths.iter().filter_map(|p| canonical(p)).collect();
        let is_stale = |key: &str| {
            paths.iter().any(|p| p == key) || canonical(key).is_some_and(|c| written.contains(&c))
        };
        let before = self.gpu_textures.len();
        self.gpu_textures.retain(|key, _| !is_stale(key));
        let misses_before = self.texture_freshness.misses.len();
        self.texture_freshness.misses.retain(|key| !is_stale(key));
        let dropped = self.gpu_textures.len() != before
            || self.texture_freshness.misses.len() != misses_before;
        if !dropped {
            return;
        }
        self.materials.forget_groups();
        if is_stale(&self.skybox_path) {
            // An empty path makes the next frame reload the scene's skybox.
            self.skybox_path.clear();
        }
    }
}

/// `path` resolved to an absolute, symlink-free form, if the file exists.
fn canonical(path: &str) -> Option<std::path::PathBuf> {
    std::fs::canonicalize(path).ok()
}
