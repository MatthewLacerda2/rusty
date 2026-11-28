//! The floating origin's tick-tail system: past the threshold, the World shifts by whole
//! steps so its focus is near zero again. Decided from sim state alone, so a replay shifts
//! on the same tick.

use glam::{IVec3, Vec3};

use crate::resources::Resources;
use crate::world::World;
use rusty_scene::Scene;

/// After the scene loads: a scene loaded this tick shifts before anything steps in it.
pub(crate) fn shift_origin(world: &mut World, res: &mut Resources) {
    let steps = {
        let scene = world.scene.borrow();
        let at = focus(&scene, res);
        let settings = res.application.borrow().origin_settings();
        scene.loaded.origin.plan(at, &settings)
    };
    if let Some(steps) = steps {
        shift(world, res, steps);
    }
}

/// Back to authored space before a `"Single"` load swaps the World, so its survivors land
/// where the new scene's coordinates expect them.
pub(crate) fn to_authored(world: &mut World, res: &mut Resources) {
    let steps = world.scene.borrow().loaded.origin.steps();
    if steps != IVec3::ZERO {
        shift(world, res, -steps);
    }
}

fn shift(world: &mut World, res: &mut Resources, steps: IVec3) {
    let by = {
        let mut scene = world.scene.borrow_mut();
        let (mut nav, mut physics) = (res.nav.borrow_mut(), res.physics.borrow_mut());
        let tracked = nav.tracks(&scene);
        if tracked {
            nav.sync_changes(&mut scene);
        }
        if let Some(physics) = physics.as_mut() {
            physics.prepare_shift(&mut scene);
        }
        let by = scene.shift_origin(steps);
        if tracked {
            nav.shift_origin(&mut scene, by);
        }
        if let Some(physics) = physics.as_mut() {
            physics.shift_origin(&mut scene, by);
        }
        by
    };
    res.camera.borrow_mut().position += by;
    res.audio.borrow_mut().shift_origin(by);
    res.script_manager.dispatch_origin_shift(by);
}

/// The chosen entity, else the main camera, else the camera resource Play draws from.
fn focus(scene: &Scene, res: &Resources) -> Vec3 {
    let chosen = scene.loaded.origin.focus;
    let chosen = chosen.filter(|&id| scene.world.contains(id));
    match chosen.or_else(|| rusty_scene::camera::main_camera(scene)) {
        Some(id) => scene.world.world_position(id),
        None => res.camera.borrow().position,
    }
}
