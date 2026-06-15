//! src/app/play.rs — play-mode systems.
//!
//! Each free function is an engine "system" operating on the `GameWorld`. There is
//! NO gameplay here: no control scheme, no weapon, no damage constants. The player
//! controller, the weapon, and the health/death behaviour are bundled GAME scripts
//! (`assets/scripts/player_controller.lua`, `bot.lua`) attached to entities; the
//! systems below only run engine logic (nav, physics, scripts, animator). Entity
//! lookups for the debug nav path still go through the name map (the demo's
//! "Player" / "Enemy_1"); that is debug visualisation, not gameplay.
//!
//! These systems are no longer called by a hand-wired sequence: [`register`]
//! pushes them into the schedule's `FixedUpdate` stage, in the order they used to
//! run, and `GameWorld::tick` drives the schedule. `FixedUpdate` is the
//! deterministic fixed-dt stage the headless harness steps, so every simulation
//! system belongs there; the per-frame order is unchanged.

use super::game::GameWorld;
use super::registry::App;
use super::stage::Stage;

const PLAYER_NAME: &str = "Player";
const ENEMY_NAME: &str = "Enemy_1";

/// Rebake cadence in play-mode frames. At the fixed 1/60 timestep this is "once a
/// second", but the trigger is the frame count, not the wall clock — that is what
/// makes a headless replay deterministic.
const REBAKE_INTERVAL_FRAMES: u64 = 60;

/// Register the play-mode systems into the schedule, in the exact order the old
/// hand-wired loop ran them. All are sim systems, so they live in `FixedUpdate`.
pub(super) fn register(app: &mut App) {
    app.add_system(Stage::FixedUpdate, rebake_and_path)
        .add_system(Stage::FixedUpdate, update_scripts)
        .add_system(Stage::FixedUpdate, tick_nav)
        .add_system(Stage::FixedUpdate, step_physics)
        .add_system(Stage::FixedUpdate, animate)
        .add_system(Stage::FixedUpdate, super::particles::tick_particles)
        .add_system(Stage::FixedUpdate, advance_frame);
}

/// Drive every entity script's `Update`.
fn update_scripts(g: &mut GameWorld, dt: f32) {
    g.script_manager.update_scripts(dt);
}

/// Step the rapier world and dispatch any resulting trigger events to scripts.
fn step_physics(g: &mut GameWorld, dt: f32) {
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
}

/// Advance the deterministic play-mode frame counter (drives the rebake cadence).
fn advance_frame(g: &mut GameWorld, _dt: f32) {
    g.play_frame += 1;
}

/// Rebake the navmesh once per second (every `REBAKE_INTERVAL_FRAMES` frames) and
/// recompute the Enemy→Player debug path.
fn rebake_and_path(g: &mut GameWorld, _dt: f32) {
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
