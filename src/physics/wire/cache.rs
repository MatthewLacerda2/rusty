//! [`WireCache`]: each collider's [`ShapeWire`], built once and kept until what
//! physics builds the shape from changes.
//!
//! **What moves a wire.** The inputs `shape::build::collider_inputs` reads: the
//! Collider (shape, `active`, `is_trigger`, its shared mesh), the entity's Mesh, a
//! CharacterController, and the world scale. The three columns are read off the
//! World's change log (`ecs::changes`), as physics' own sync reads them
//! (`physics::watch`); the scale is compared with the one the wire was built at.
//! A log that cannot say (a scene load, an Undo, Play's restore, a reader that
//! fell behind, another scene) forgets every wire. A mesh physics refuses is kept
//! as refused, so its warning is logged once per change, not once per frame.

use std::collections::BTreeMap;

use glam::Vec3;

use super::{shape_wire, ShapeWire};
use crate::components::{CharacterControllerComponent, ColliderComponent, MeshComponent};
use crate::ecs::{Changes, Cursor, World};
use crate::scene::{Scene, SceneId};

/// The change-log cursors a [`WireCache`] reads.
struct Cursors {
    colliders: Cursor<ColliderComponent>,
    meshes: Cursor<MeshComponent>,
    characters: Cursor<CharacterControllerComponent>,
}

impl Cursors {
    fn open(world: &World) -> Self {
        Self {
            colliders: world.change_cursor(),
            meshes: world.change_cursor(),
            characters: world.change_cursor(),
        }
    }

    /// The ids whose wire may have changed since the last read; `None` for all.
    fn read(&mut self, world: &World) -> Option<Vec<u32>> {
        let mut ids = Vec::new();
        let reads = [
            world.changes(&mut self.colliders),
            world.changes(&mut self.meshes),
            world.changes(&mut self.characters),
        ];
        for changes in reads {
            match changes {
                Changes::Rebuild => return None,
                Changes::Ids(more) => ids.extend(more),
            }
        }
        Some(ids)
    }
}

/// Each entity's wire, kept across frames (see the module docs).
#[derive(Default)]
pub struct WireCache {
    scene: Option<SceneId>,
    cursors: Option<Cursors>,
    /// Per entity: the scale it was built at, and the wire (`None`: no shape).
    wires: BTreeMap<u32, (Vec3, Option<ShapeWire>)>,
    /// Wires built since the cache was made: what a test counts a rebuild by.
    builds: u64,
}

impl WireCache {
    /// Forget the wires of whatever changed in `scene` since the last call.
    pub fn refresh(&mut self, scene: &Scene) {
        if self.scene != Some(scene.id()) {
            self.scene = Some(scene.id());
            self.cursors = None;
        }
        let changed = match self.cursors.as_mut() {
            Some(cursors) => cursors.read(&scene.world),
            None => {
                self.cursors = Some(Cursors::open(&scene.world));
                None
            }
        };
        match changed {
            Some(ids) => ids.iter().for_each(|id| drop(self.wires.remove(id))),
            None => self.wires.clear(),
        }
    }

    /// `id`'s wire, built now if it is not kept or its scale moved. Call
    /// [`refresh`](Self::refresh) first, once per frame.
    pub fn wire(&mut self, scene: &Scene, id: u32) -> Option<&ShapeWire> {
        let scale = scene.world.world_pose(id)?.scale;
        let kept = self.wires.get(&id).is_some_and(|(at, _)| *at == scale);
        if !kept {
            self.builds += 1;
            self.wires.insert(id, (scale, shape_wire(scene, id)));
        }
        self.wires.get(&id)?.1.as_ref()
    }

    /// Wires built since the cache was made.
    pub fn builds(&self) -> u64 {
        self.builds
    }
}
