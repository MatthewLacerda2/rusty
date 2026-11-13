//! [`WireCache`]: each collider's [`ShapeWire`], built once and kept until what
//! physics builds the shape from changes.
//!
//! **What moves a wire.** The inputs `shape::build::collider_inputs` reads: the
//! Collider (shape, `active`, `is_trigger`, its kept shared mesh and its collision
//! mesh, #1960), the entity's Mesh, a
//! CharacterController, the world scale (within a relative 1e-5, so a turning
//! parent's float noise is no change) and whether its body is dynamic (a concave
//! mesh is decomposed on one, #1886). The three columns are read off the World's
//! change log (`ecs::changes`), as physics' own sync reads them
//! (`physics::watch`); the scale and the body's class (it may be a parent's) are
//! compared with what the wire was built for.
//! A log that cannot say (a scene load, an Undo, Play's restore, a reader that
//! fell behind, another scene) forgets every wire. A mesh physics refuses is kept
//! as refused, so its warning is logged once per change, not once per frame.

use std::collections::BTreeMap;

use glam::Vec3;

use super::{on_dynamic_body, shape_wire, ShapeWire};
use rusty_components::{CharacterControllerComponent, ColliderComponent, MeshComponent};
use rusty_ecs::{Changes, Cursor, World};
use rusty_scene::{Scene, SceneId};

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
    /// Per entity: the scale and the body fit (dynamic or not) it was built for,
    /// and the wire (`None`: no shape).
    wires: BTreeMap<u32, ((Vec3, bool), Option<ShapeWire>)>,
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

    /// `id`'s wire, built now if it is not kept, or its scale or its body's fit
    /// moved. Call [`refresh`](Self::refresh) first, once per frame.
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

    /// Wires built since the cache was made.
    pub fn builds(&self) -> u64 {
        self.builds
    }
}

/// The most a world scale may move, relative to its size, and still be the one a
/// wire was built for. A child of a turning parent reads a lossy scale that drifts
/// by an ulp or two from tick to tick, though nothing changed: compared bit for bit,
/// that rebuilt its wire (and re-ran a decomposition) every frame.
const SCALE_TOLERANCE: f32 = 1e-5;

/// Whether `a` and `b` are the same scale, within [`SCALE_TOLERANCE`].
fn same_scale(a: Vec3, b: Vec3) -> bool {
    if a.to_array().map(f32::to_bits) == b.to_array().map(f32::to_bits) {
        return true; // a NaN scale too: refused once, not every frame
    }
    let size = a.abs().max(b.abs()).max(Vec3::splat(f32::MIN_POSITIVE));
    ((a - b).abs()).cmple(size * SCALE_TOLERANCE).all()
}
