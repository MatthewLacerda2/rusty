//! A wire is built again when what physics builds its shape from moves: the Collider, the
//! entity's Mesh or CharacterController, the world scale (within a relative 1e-5, so a turning
//! parent's float noise is no change) and whether its body is dynamic (a concave mesh is
//! decomposed on one). A log that cannot say (a load, an Undo, another scene) forgets every wire.
//! A mesh physics refuses is kept as refused, so its warning is logged once per change.

use std::collections::BTreeMap;

use glam::Vec3;

use super::{on_dynamic_body, shape_wire, ShapeWire};
use rusty_components::{CharacterControllerComponent, ColliderComponent, MeshComponent};
use rusty_ecs::{Changes, Cursor, World};
use rusty_scene::{Scene, SceneId};

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

#[derive(Default)]
pub struct WireCache {
    scene: Option<SceneId>,
    cursors: Option<Cursors>,
    wires: BTreeMap<u32, ((Vec3, bool), Option<ShapeWire>)>,
    builds: u64,
}

impl WireCache {
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

    pub fn wire(&mut self, scene: &Scene, id: u32) -> Option<&ShapeWire> {
        let scale = scene.world.world_pose(id)?.scale;
        let built_for = (scale, on_dynamic_body(scene, id));
        let kept = self
            .wires
            .get(&id)
            .is_some_and(|((at, fit), _)| *fit == built_for.1 && same_scale(*at, built_for.0));
        if !kept {
            self.builds += 1;
            self.wires.insert(id, (built_for, shape_wire(scene, id)));
        }
        self.wires.get(&id)?.1.as_ref()
    }

    /// Test hook for this crate's cache tests: wires built since the cache was made.
    pub fn builds(&self) -> u64 {
        self.builds
    }
}

const SCALE_TOLERANCE: f32 = 1e-5;

fn same_scale(a: Vec3, b: Vec3) -> bool {
    if a.to_array().map(f32::to_bits) == b.to_array().map(f32::to_bits) {
        return true; // a NaN scale too
    }
    let size = a.abs().max(b.abs()).max(Vec3::splat(f32::MIN_POSITIVE));
    ((a - b).abs()).cmple(size * SCALE_TOLERANCE).all()
}
