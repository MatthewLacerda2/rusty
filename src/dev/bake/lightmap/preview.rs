//! A progressive bake's preview on the live scene (#1970, Unity's Progressive
//! Lightmapper). After each pass the bake's worker hands the lightmaps as they
//! stand to an encoder thread of its own (`encoder::Encoder`), so the trace goes on at
//! once; the encoder writes them beside the scene as `lightmap_preview_*` pages,
//! cheaply (uncompressed PNGs of one level, which the renderer loads as it loads
//! any `.png` page), and the editor's next frame points the scene at them
//! ([`Previews::show`]). A pass not yet shown when the next one is in is skipped,
//! and the writer removes its pages; while the writer is busy, the bake builds
//! no lightmaps for a preview at all (`Encoder::busy`).
//!
//! The scene's own lightmaps are hidden under the preview, in its set
//! (`LightmapSet::shown_over`, their one owner), so a save meanwhile writes them,
//! never a preview's pages, and an edit (a despawn, a clear) reaches them. A
//! cancel or a failure puts them back and removes the preview pages
//! ([`Previews::restore`]); a finished bake's write replaces both, its clear of the
//! folder taking the preview pages with it.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, TryLockError};

use super::lightmap_dir;
use super::write::{clear_pages, remove_stems, stems, PREVIEW};
use crate::scene::lighting::lightmap::{LightmapSet, Pass};
use crate::scene::Scene;

/// What the writer and the UI share, under one lock held only to swap or remove
/// pages, never while one is written: the newest pass written and not yet shown,
/// and the page stems the scene shows.
#[derive(Default)]
pub(super) struct Shared {
    pub latest: Option<(Pass, LightmapSet)>,
    pub shown: BTreeSet<String>,
}

pub(super) type Latest = Arc<Mutex<Shared>>;

/// One bake's previews, as the UI thread sees them: the newest pass written.
pub(super) struct Previews {
    pub(super) latest: Latest,
    pub(super) dir: PathBuf,
}

impl Previews {
    /// Previews written beside `scene_path`.
    pub fn new(scene_path: &str) -> Self {
        Self {
            latest: Latest::default(),
            dir: lightmap_dir(scene_path),
        }
    }

    /// Show the newest pass on `scene`, if one came in since the last call, and
    /// say which. The scene's own lightmaps are hidden under it the first time,
    /// and the pages of the pass shown before are removed; one that cannot be is
    /// said, and the pass is shown all the same. A frame that finds the writer
    /// swapping a pass in waits for the next.
    pub fn show(&mut self, scene: &mut Scene) -> Option<Pass> {
        let mut shared = match self.latest.try_lock() {
            Ok(shared) => shared,
            Err(TryLockError::Poisoned(e)) => e.into_inner(),
            Err(TryLockError::WouldBlock) => return None,
        };
        let (pass, mut set) = shared.latest.take()?;
        let shown = &mut scene.lightmaps;
        let own = shown.shown_over.take();
        set.shown_over = Some(own.unwrap_or_else(|| Box::new((**shown).clone())));
        let keep = stems(&set);
        scene.lightmaps = set.into();
        // Only the pages shown before: the writer may be writing the next ones.
        let old = std::mem::replace(&mut shared.shown, keep);
        let gone = old.difference(&shared.shown).cloned().collect();
        if let Err(e) = remove_stems(&self.dir, &gone) {
            log::warn!("[LightmapBake] remove a preview page no longer shown: {e}");
        }
        Some(pass)
    }

    /// Put the scene's own lightmaps back, if a preview is shown, and remove the
    /// preview pages.
    pub fn restore(&mut self, scene: &mut Scene) {
        if let Some(own) = scene.lightmaps.shown_over.take() {
            scene.lightmaps = (*own).into();
        }
        self.clear();
    }

    /// Remove the preview pages; the folder may hold none. Once the bake is
    /// cancelled, a pass the writer is still writing is removed by the writer.
    pub fn clear(&self) {
        let mut shared = lock(&self.latest);
        *shared = Shared::default();
        if let Err(e) = clear_pages(&self.dir, PREVIEW, &BTreeSet::new()) {
            log::warn!("[LightmapBake] remove the preview lightmaps: {e}");
        }
    }
}

pub(super) fn lock<V>(m: &Mutex<V>) -> MutexGuard<'_, V> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}
