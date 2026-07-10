//! Particle collision response (issue #13): a particle fired into a static
//! collider must either die on contact or bounce back off it. Exercised through the
//! real play loop so the rapier world is built exactly as in-game.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::components::particle::{CollisionResponse, EmitMode, ParticleEmitterComponent};
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::scene::{ColliderComponent, ColliderShape, Scene};
use rusty::scripting::ConsoleLogs;

/// Build a world: a static wall at x=+3 and an emitter at the origin firing one
/// burst of particles straight at the wall along +X.
fn scene_with_wall(response: CollisionResponse) -> (Scene, u32) {
    let mut scene = Scene::new();

    let wall = scene.add_entity("Wall".to_string());
    scene.world.set_static(wall, true);
    scene.world.transform_mut(wall).unwrap().position = Vec3::new(3.0, 0.0, 0.0);
    scene.world.set_collider(
        wall,
        Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box {
                size: Vec3::new(1.0, 4.0, 4.0),
            },
            is_trigger: false,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        }),
    );
    scene.update_entity_collider(wall);

    let emitter = scene.add_entity("Emitter".to_string());
    scene.world.transform_mut(emitter).unwrap().position = Vec3::ZERO;
    scene.world.set_particles(
        emitter,
        Some(ParticleEmitterComponent {
            emit_mode: EmitMode::Burst,
            burst_count: 16,
            looping: false,
            collision: response,
            rate: 0.0,
            lifetime: 5.0,
            speed: 8.0,
            direction: Vec3::X,
            spread: 0.0,
            gravity: Vec3::ZERO,
            bounciness: 0.8,
            seed: 3,
            ..Default::default()
        }),
    );
    (scene, emitter)
}

fn build_world(scene: Scene) -> Rc<RefCell<GameWorld>> {
    let scene = Rc::new(RefCell::new(scene));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -20.0, 20.0, -20.0, 20.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    Rc::new(RefCell::new(GameWorld::new(scene, input, nav, console)))
}

/// Run `frames` ticks and return the emitter's live particles' max X reached and
/// whether any are still moving in +X (positive vx).
fn run(scene: Scene, emitter: u32, frames: u32) -> (usize, f32, bool) {
    let world = build_world(scene);
    {
        let mut w = world.borrow_mut();
        w.set_playing(true);
        for _ in 0..frames {
            w.tick(1.0 / 60.0);
        }
    }
    let w = world.borrow();
    let scene = w.scene().borrow();
    let particles = scene.world.particles(emitter).unwrap();
    let ps = &particles.runtime.particles;
    let max_x = ps.iter().map(|p| p.position.x).fold(f32::MIN, f32::max);
    let any_forward = ps.iter().any(|p| p.velocity.x > 0.1);
    (ps.len(), max_x, any_forward)
}

#[test]
fn particles_die_on_collider_contact() {
    // Without collision the burst flies past x=3; with Die it should be culled at
    // (or just past spawn before reaching) the wall, leaving far fewer particles.
    let (free_scene, free_em) = scene_with_wall(CollisionResponse::None);
    let (count_free, max_x_free, _) = run(free_scene, free_em, 60);
    assert!(
        count_free > 0 && max_x_free > 3.0,
        "free particles fly past the wall"
    );

    let (die_scene, die_em) = scene_with_wall(CollisionResponse::Die);
    let (count_die, max_x_die, _) = run(die_scene, die_em, 60);
    assert!(
        count_die < count_free,
        "Die response should despawn particles that hit the wall (die={count_die}, free={count_free})"
    );
    assert!(
        max_x_die <= 3.1,
        "no surviving particle should have passed through the wall (max_x={max_x_die})"
    );
}

#[test]
fn particles_bounce_off_collider() {
    // After enough frames the bounced particles should have reversed and be heading
    // back in -X, and none should have tunnelled through the wall.
    let (scene, em) = scene_with_wall(CollisionResponse::Bounce);
    let (count, max_x, any_forward) = run(scene, em, 90);
    assert!(count > 0, "bounced particles should still be alive");
    assert!(
        max_x <= 3.1,
        "bounced particles must not tunnel through the wall (max_x={max_x})"
    );
    assert!(
        !any_forward,
        "after bouncing, no particle should still travel toward the wall (+X)"
    );
}
