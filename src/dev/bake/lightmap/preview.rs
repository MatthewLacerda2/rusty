//! A progressive bake's preview on the live scene (#1970, Unity's Progressive
//! Lightmapper): after each pass the worker encodes the lightmaps as they stand
//! ([`Previews::encode`]), and the editor's next frame writes them beside the scene
//! as `lightmap_preview_*` pages and points the scene at them ([`Previews::show`]),
//! the same path a finished bake's pages take to the renderer.
//!
//! The scene's own lightmaps are kept while a preview shows, inside its set
//! (`LightmapSet::shown_over`), so a save meanwhile writes them and never a
//! preview's pages. A cancel or a failure puts them back and removes the preview
//! pages ([`Previews::restore`]); a finished bake's write replaces both, its clear
//! of the folder taking the preview pages with it.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use super::job::{labelled, Taken};
use super::write::{clear_pages, write_set, PREVIEW};
use super::{encode_pages, lightmap_dir, EncodedPages};
use crate::scene::lighting::lightmap::{BakeProgress, Lightmap, LightmapSet, Pass};
use crate::scene::Scene;

/// The newest pass's encoded pages, handed from the worker to the UI thread; a
/// pass the UI has not shown yet is replaced by the next.
pub(super) type Latest = Arc<Mutex<Option<(Pass, EncodedPages)>>>;

/// One bake's previews: the newest from the worker, and the scene's own lightmaps
/// once one is shown.
pub(super) struct Previews {
    latest: Latest,
    dir: PathBuf,
    kept: Option<LightmapSet>,
}

impl Previews {
    /// Previews written beside `scene_path`.
    pub fn new(scene_path: &str) -> Self {
        Self {
            latest: Latest::default(),
            dir: lightmap_dir(scene_path),
            kept: None,
        }
    }

    /// Where the worker puts each pass's pages.
    pub fn latest(&self) -> Latest {
        Arc::clone(&self.latest)
    }

    /// On the worker: `pass`'s lightmaps `maps` encoded into pages of at most
    /// `page` texels and handed to `latest` with the lights and channels `taken`.
    /// A cancel or a failed encode hands nothing (the next pass tries again); the
    /// page counters are cleared after, so the bar keeps showing the trace.
    pub fn encode(
        latest: &Latest,
        (pass, maps): (Pass, &[Lightmap]),
        page: u32,
        taken: &Taken,
        progress: &BakeProgress,
    ) {
        if let Ok(Some(pages)) = encode_pages(maps, page, progress) {
            *latest.lock().unwrap_or_else(PoisonError::into_inner) =
                Some((pass, labelled(pages, taken)));
        }
        progress.start_pages(0);
    }

    /// On the UI thread: show the newest pass on `scene`, if one came in since the
    /// last call, and say which. The scene's own lightmaps are kept the first time.
    pub fn show(&mut self, scene: &mut Scene) -> Result<Option<Pass>, String> {
        let newest = self
            .latest
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        let Some((pass, pages)) = newest else {
            return Ok(None);
        };
        let kept = self
            .kept
            .get_or_insert_with(|| scene.lightmaps.saved().clone());
        clear_pages(&self.dir, PREVIEW)?;
        let mut set = write_set(&self.dir, PREVIEW, &pages)?;
        set.shown_over = Some(Box::new(kept.clone()));
        scene.lightmaps = set.into();
        Ok(Some(pass))
    }

    /// Put the scene's own lightmaps back, if a preview was shown, and remove the
    /// preview pages.
    pub fn restore(&mut self, scene: &mut Scene) {
        if let Some(kept) = self.kept.take() {
            scene.lightmaps = kept.into();
        }
        self.clear();
    }

    /// Remove the preview pages; the folder may hold none.
    pub fn clear(&self) {
        if let Err(e) = clear_pages(&self.dir, PREVIEW) {
            log::warn!("[LightmapBake] remove the preview lightmaps: {e}");
        }
    }
}
