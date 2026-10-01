//! src/app/trails.rs — the trail recording tick (#441).
//!
//! Each active entity's `TrailComponent` samples the entity's **world** position
//! once per fixed tick and ages its points on the same scaled `dt`. It runs after
//! `LateUpdate`, so a follow-script or a parent that moved this tick is already
//! settled when the trail records where the entity ended up. Pure CPU, sim-side:
//! the renderer only reads the recorded points, so a headless replay records the
//! identical trail. An inactive entity's trail is frozen, as Unity's is.

use super::resources::Resources;
use super::world::World;

/// Advance every active trail by `res.frame_dt`.
pub(super) fn tick_trails(world: &mut World, res: &mut Resources) {
    let dt = res.frame_dt;
    let mut scene = world.scene.borrow_mut();
    for id in scene.world.ids_with_trail() {
        if !scene.world.is_active(id) {
            continue;
        }
        let position = scene.compute_world_matrix(id).w_axis.truncate();
        if let Some(mut trail) = scene.world.trail_mut(id) {
            trail.advance(position, dt);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use glam::Vec3;

    use crate::app::GameWorld;
    use crate::components::TrailComponent;
    use crate::core::input::InputState;
    use crate::navigation::NavigationGraph;
    use crate::scene::Scene;
    use crate::scripting::ConsoleLogs;

    fn game() -> GameWorld {
        let nav = NavigationGraph::new(-20.0, 20.0, -20.0, 20.0, 1.0);
        GameWorld::new(
            Rc::new(RefCell::new(Scene::new())),
            Rc::new(RefCell::new(InputState::new())),
            Rc::new(RefCell::new(nav)),
            Rc::new(RefCell::new(ConsoleLogs::new())),
        )
    }

    #[test]
    fn a_moving_entity_records_its_world_path_in_play() {
        let mut game = game();
        let (parent, id) = {
            let mut s = game.world.scene.borrow_mut();
            let parent = s.add_entity("Parent".into());
            let id = s.add_entity("Rocket".into());
            s.set_parent(id, Some(parent)).unwrap();
            s.world.transform_mut(parent).unwrap().position = Vec3::new(0.0, 5.0, 0.0);
            let trail = TrailComponent {
                min_vertex_distance: 0.0,
                time: 10.0,
                ..Default::default()
            };
            s.world.set_trail(id, Some(trail));
            (parent, id)
        };
        game.set_playing(true);
        for step in 0..4 {
            let mut s = game.world.scene.borrow_mut();
            s.world.transform_mut(parent).unwrap().position.x = step as f32;
            drop(s);
            game.tick(1.0 / 60.0);
        }
        let s = game.world.scene.borrow();
        let trail = s.world.trail(id).unwrap();
        let path: Vec<Vec3> = trail.positions().collect();
        assert!(path.len() >= 3, "recorded {path:?}");
        assert!(path.iter().all(|p| p.y == 5.0), "world space: {path:?}");
        assert_eq!(path.last().unwrap().x, 3.0);
    }
}
