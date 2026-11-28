//! A stamp selects, a value decides: a changed `NavMeshModifier` counts wherever it is (it changes
//! how its static children bake), but only if it now differs from the one kept at the last read.

use super::super::NavigationGraph;
use super::sync::Rebake;
use rusty_components::NavMeshModifierComponent;
use rusty_core::collections::Map;
use rusty_ecs::{Changes, Cursor, StaticGeometry, StaticSet, World};
use rusty_scene::Scene;

pub(crate) struct StaticWatch {
    geometry: StaticGeometry,
    statics: Cursor<StaticSet>,
    modifiers: Modifiers,
}

struct Modifiers {
    cursor: Cursor<NavMeshModifierComponent>,
    kept: Map<u32, NavMeshModifierComponent>,
}

impl Modifiers {
    fn open(w: &World) -> Self {
        let mut m = Self {
            cursor: w.change_cursor(),
            kept: Map::default(),
        };
        m.rebuild(w);
        m
    }

    fn rebuild(&mut self, w: &World) {
        self.kept.clear();
        for id in w.ids_with_nav_mesh_modifier() {
            if let Some(m) = w.nav_mesh_modifier(id) {
                self.kept.insert(id, m.clone());
            }
        }
    }

    fn still(&mut self, w: &World) -> bool {
        let ids = match w.changes(&mut self.cursor) {
            Changes::Rebuild => {
                self.rebuild(w);
                return false;
            }
            Changes::Ids(ids) => ids,
        };
        let mut still = true;
        for id in ids {
            let now = w.nav_mesh_modifier(id).map(|m| m.clone());
            if now.as_ref() == self.kept.get(&id) {
                continue;
            }
            still = false;
            match now {
                Some(m) => self.kept.insert(id, m),
                None => self.kept.remove(&id),
            };
        }
        still
    }
}

impl StaticWatch {
    fn open(w: &mut World) -> Self {
        Self {
            geometry: w.static_geometry(),
            statics: w.change_cursor(),
            modifiers: Modifiers::open(w),
        }
    }

    pub(crate) fn still(&mut self, w: &mut World) -> bool {
        let membership = w.changes(&mut self.statics).is_empty();
        let modifiers = self.modifiers.still(w);
        let geometry = self.geometry.changed(w);
        membership && modifiers && !geometry
    }
}

impl NavigationGraph {
    pub fn sync_changes(&mut self, scene: &mut Scene) -> Rebake {
        let still = match self.static_watch.as_mut() {
            Some(watch) => watch.still(&mut scene.world),
            None => {
                self.static_watch = Some(StaticWatch::open(&mut scene.world));
                false
            }
        };
        let watch = self.static_watch.take();
        let rebake = self.sync_inner(scene, still);
        // A bake that re-shaped the grid made a new graph; the watch carries over.
        self.static_watch = watch;
        rebake
    }

    pub fn restore_or_bake(&mut self, scene: &Scene) -> Start {
        let Some(bytes) = scene.navmesh.bytes.clone() else {
            self.bake(scene);
            return Start::Baked;
        };
        match self.restore(scene, &bytes) {
            Ok(_) => Start::Restored,
            Err(why) => {
                log::warn!("[Navigation] the saved navmesh was refused ({why}): baking it");
                self.bake(scene);
                Start::Refused(why)
            }
        }
    }

    pub fn start_tracked(&mut self, scene: &mut Scene) -> Start {
        self.start_tracked_with(scene, None)
    }

    pub fn start_tracked_with(&mut self, scene: &mut Scene, baked: Option<&[u8]>) -> Start {
        let watch = StaticWatch::open(&mut scene.world);
        let start = match baked.filter(|_| scene.navmesh.bytes.is_none()) {
            Some(bytes) if self.restore(scene, bytes).is_ok() => Start::Restored,
            _ => self.restore_or_bake(scene),
        };
        self.static_watch = Some(watch);
        self.tracked_scene = Some((scene.id(), start.clone()));
        start
    }

    pub fn baked_bytes(scene: &Scene) -> Vec<u8> {
        let b = crate::NavBounds::EMPTY_SCENE;
        let spacing = rusty_scene::nav_settings::DEFAULT_GRID_SPACING;
        Self::new(b.min_x, b.max_x, b.min_z, b.max_z, spacing).bake_to_bytes(scene)
    }

    pub fn tracks(&self, scene: &Scene) -> bool {
        self.tracked_scene
            .as_ref()
            .is_some_and(|(id, _)| *id == scene.id())
    }

    pub fn last_start(&self) -> Option<Start> {
        self.tracked_scene.as_ref().map(|(_, start)| start.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Start {
    Restored,
    Baked,
    Refused(String),
}
