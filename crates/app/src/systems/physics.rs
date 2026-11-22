use crate::resources::Resources;
use crate::world::World;

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

pub(crate) fn follow_bones(world: &mut World, res: &mut Resources) {
    if let Some(physics) = res.physics.borrow_mut().as_mut() {
        physics.follow_bones(&world.scene.borrow());
    }
}

pub(crate) fn pose_ragdolls(world: &mut World, res: &mut Resources) {
    let mut scene = world.scene.borrow_mut();
    scene.advance_reactions(res.frame_dt);
    if let Some(physics) = res.physics.borrow().as_ref() {
        physics.pose_bones(&mut scene);
    }
    scene.end_reactions();
}
