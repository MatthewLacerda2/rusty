//! src/app/play.rs — play-mode systems.
//!
//! Each free function is an engine "system" operating on the `GameWorld`. There is
//! NO gameplay here: no control scheme, no weapon, no damage constants. The player
//! controller, the weapon, and the health/death behaviour are bundled GAME scripts
//! (`assets/scripts/player_controller.lua`, `bot.lua`) attached to entities; the
//! loop below only runs systems (nav, physics, scripts, animator). Entity lookups
//! for the debug nav path still go through the name map (the demo's "Player" /
//! "Enemy_1"); that is debug visualisation, not gameplay.

use super::game::GameWorld;

const PLAYER_NAME: &str = "Player";
const ENEMY_NAME: &str = "Enemy_1";

/// Rebake cadence in play-mode frames. At the fixed 1/60 timestep this is "once a
/// second", but the trigger is the frame count, not the wall clock — that is what
/// makes a headless replay deterministic.
const REBAKE_INTERVAL_FRAMES: u64 = 60;

/// Run one play-mode frame — engine systems only. Gameplay (player control, the
/// weapon, damage/death) lives in the entities' scripts, driven by `update_scripts`.
pub(super) fn run(g: &mut GameWorld, dt: f32) {
    rebake_and_path(g);
    g.script_manager.update_scripts(dt);
    tick_nav(g, dt);

    let triggers = {
        let mut s = g.scene.borrow_mut();
        match g.physics.borrow_mut().as_mut() {
            Some(physics) => physics.step(&mut s, dt),
            None => Vec::new(),
        }
    };
    if !triggers.is_empty() {
        g.script_manager.dispatch_trigger_events(triggers);
    }

    animate(g, dt);
    g.play_frame += 1;
}

/// Rebake the navmesh once per second (every `REBAKE_INTERVAL_FRAMES` frames) and
/// recompute the Enemy→Player debug path.
fn rebake_and_path(g: &mut GameWorld) {
    if !g.play_frame.is_multiple_of(REBAKE_INTERVAL_FRAMES) {
        return;
    }
    let s = g.scene.borrow();
    g.nav.borrow_mut().bake(&s);
    let enemy_pos = s
        .find_entity_by_name(ENEMY_NAME)
        .and_then(|id| s.get_entity(id))
        .map(|e| e.transform.position);
    let player_pos = s
        .find_entity_by_name(PLAYER_NAME)
        .and_then(|id| s.get_entity(id))
        .map(|e| e.transform.position);
    if let (Some(enemy_pos), Some(player_pos)) = (enemy_pos, player_pos) {
        let grid = g.nav.borrow();
        let (es_x, es_z) = grid.world_to_grid(enemy_pos);
        let (pl_x, pl_z) = grid.world_to_grid(player_pos);
        let mut pts = vec![enemy_pos];
        if let Some(grid_pts) = grid.find_path(es_x, es_z, pl_x, pl_z) {
            for &(gx, gz) in &grid_pts {
                pts.push(grid.grid_to_world(gx, gz));
            }
        }
        pts.push(player_pos);
        g.pathfinding_points = pts;
    }
}

fn tick_nav(g: &mut GameWorld, dt: f32) {
    let mut s = g.scene.borrow_mut();
    let nav = g.nav.borrow();
    nav.tick_nav_agents(&mut s, dt);
}

fn animate(g: &mut GameWorld, dt: f32) {
    let mut s = g.scene.borrow_mut();
    for id in s.entity_ids() {
        if let Some(mut entity) = s.get_entity_mut(id) {
            if !entity.active {
                continue;
            }
            if let Some(anim) = &mut entity.animator {
                if anim.is_playing && !anim.freeze {
                    anim.time += dt;
                }
            }
        }
    }
}
