use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, TryLockError};

use super::lightmap_dir;
use super::write::{clear_pages, remove_stems, stems, PREVIEW};
use rusty_lighting::lightmap::Pass;
use rusty_scene::lighting::lightmap_set::LightmapSet;
use rusty_scene::Scene;

#[derive(Default)]
pub(super) struct Shared {
    pub latest: Option<(Pass, LightmapSet)>,
    pub shown: BTreeSet<String>,
}

pub(super) type Latest = Arc<Mutex<Shared>>;

pub(super) struct Previews {
    pub(super) latest: Latest,
    pub(super) dir: PathBuf,
    pub(super) prefix: String,
}

static BAKES: AtomicU32 = AtomicU32::new(0);

impl Previews {
    pub fn new(scene_path: &str) -> Self {
        let n = BAKES.fetch_add(1, Ordering::Relaxed);
        Self {
            latest: Latest::default(),
            dir: lightmap_dir(scene_path),
            prefix: format!("{PREVIEW}_{n}"),
        }
    }

    pub fn show(&mut self, scene: &mut Scene) -> Option<Pass> {
        let mut shared = match self.latest.try_lock() {
            Ok(shared) => shared,
            Err(TryLockError::Poisoned(e)) => e.into_inner(),
            Err(TryLockError::WouldBlock) => return None,
        };
        let (pass, mut set) = shared.latest.take()?;
        set.retain(|id| scene.world.contains(id));
        let shown = &mut scene.lightmaps;
        let own = shown.shown_over.take();
        set.shown_over = Some(own.unwrap_or_else(|| Box::new((**shown).clone())));
        let keep = stems(&set);
        scene.lightmaps = set.into();
        let old = std::mem::replace(&mut shared.shown, keep);
        let gone = old.difference(&shared.shown).cloned().collect();
        if let Err(e) = remove_stems(&self.dir, &gone) {
            log::warn!("[LightmapBake] remove a preview page no longer shown: {e}");
        }
        Some(pass)
    }

    pub fn restore(&mut self, scene: &mut Scene) {
        if let Some(own) = scene.lightmaps.shown_over.take() {
            scene.lightmaps = (*own).into();
        }
        self.clear();
    }

    pub fn clear(&self) {
        let mut shared = lock(&self.latest);
        *shared = Shared::default();
        if let Err(e) = clear_pages(&self.dir, &self.prefix, &BTreeSet::new()) {
            log::warn!("[LightmapBake] remove the preview lightmaps: {e}");
        }
    }
}

pub(super) fn lock<V>(m: &Mutex<V>) -> MutexGuard<'_, V> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}
