//! The play schedule's physics systems (`app::play::register` orders them): the
//! step and its events, a rapier panic logged (#1797), and the bone-carried
//! colliders and ragdoll bones brought to this tick's pose.

use crate::resources::Resources;
use crate::world::World;

/// Step the rapier world and dispatch its trigger and collision events to scripts;
/// a step rapier panicked in (its world rebuilt, #1797) logs one console error.
pub(crate) fn step_physics(world: &mut World, res: &mut Resources) {
    let dt = res.frame_dt;
    let mut events = {
        let mut s = world.scene.borrow_mut();
        match res.physics.borrow_mut().as_mut() {
            Some(physics) => physics.step(&mut s, dt),
            None => rusty_physics::PhysicsEvents::default(),
        }
    };
    if let Some(rebuilt) = events.rebuilt.take() {
        log_rebuild(res, rebuilt);
    }
    res.script_manager.dispatch_physics_events(events);
}

/// Log a step rapier panicked in: the first of a run names the play frame, and
/// the ones after it carry no frame, so a cause that panics every step collapses
/// into one counted line instead of filling the console (#1797).
fn log_rebuild(res: &Resources, rebuilt: rusty_physics::Rebuilt) {
    let rebuilt_from_scene = "the physics world was rebuilt from the scene";
    let message = rebuilt.message;
    let mut console = res.console.borrow_mut();
    if rebuilt.in_a_row <= 1 {
        let frame = res.play_frame();
        console.error(format!(
            "Physics: rapier panicked on frame {frame} ({message}); {rebuilt_from_scene}"
        ));
    } else {
        let line = format!("Physics: rapier panicked again ({message}); {rebuilt_from_scene}");
        console.repeated(line, rusty_script::scripting::LogLevel::Error, 1);
    }
}

/// Move every collider hanging from a bone (a hitbox) to the pose the Animator
/// and `LateUpdate` left it in, so the next tick's casts hit what was rendered
/// (#464, see `physics::follow`).
pub(crate) fn follow_bones(world: &mut World, res: &mut Resources) {
    if let Some(physics) = res.physics.borrow_mut().as_mut() {
        physics.follow_bones(&world.scene.borrow());
    }
}

/// Override the Animator on every bone a dynamic body carries — a ragdoll's —
/// with the pose this tick's step solved, blended toward the animation by its
/// weight (#466, #1895, see `physics::joints::ragdoll`). The hit reactions age by
/// this tick's `dt` first, and the bones they let go of return to the Animator
/// after, so a reaction's last tick is drawn at weight 0.
pub(crate) fn pose_ragdolls(world: &mut World, res: &mut Resources) {
    let mut scene = world.scene.borrow_mut();
    scene.advance_reactions(res.frame_dt);
    if let Some(physics) = res.physics.borrow().as_ref() {
        physics.pose_bones(&mut scene);
    }
    scene.end_reactions();
}
