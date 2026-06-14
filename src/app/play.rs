//! src/app/play.rs — play-mode systems, lifted verbatim out of main.rs's god-loop.
//!
//! Each free function is a "system" operating on the `GameWorld`. Entity lookups
//! go through the World's name map (the demo's "Player" and "Enemy_1"); the old
//! hardcoded `Player == 2` / `Enemy == 5` ids are gone. Order matches the
//! original loop exactly.

use glam::Vec3;

use super::game::GameWorld;
use crate::physics::{cast_ray_in_scene, tick_physics, Ray};

const PLAYER_NAME: &str = "Player";
const ENEMY_NAME: &str = "Enemy_1";

/// Rebake cadence in play-mode frames. At the fixed 1/60 timestep this is "once a
/// second", but the trigger is the frame count, not the wall clock — that is what
/// makes a headless replay deterministic.
const REBAKE_INTERVAL_FRAMES: u64 = 60;

/// Run one play-mode frame.
pub(super) fn run(g: &mut GameWorld, dt: f32) {
    rebake_and_path(g);
    drive_player(g, dt);
    g.script_manager.update_scripts(dt);
    tick_nav(g, dt);

    let triggers = {
        let mut s = g.scene.borrow_mut();
        tick_physics(&mut s, dt)
    };
    if !triggers.is_empty() {
        g.script_manager.dispatch_trigger_events(triggers);
    }

    hitscan(g);
    animate(g, dt);
    g.play_frame += 1;
}

/// Rebake the navmesh once per second (every `REBAKE_INTERVAL_FRAMES` frames) and
/// recompute the Enemy→Player debug path.
fn rebake_and_path(g: &mut GameWorld) {
    if g.play_frame % REBAKE_INTERVAL_FRAMES != 0 {
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

/// Drive the Player entity from input and keep the third-person camera behind it.
fn drive_player(g: &mut GameWorld, dt: f32) {
    let mut move_dir = Vec3::ZERO;
    {
        let inp = g.input.borrow();
        if inp.is_key_down("W") {
            move_dir += g.camera.forward();
        }
        if inp.is_key_down("S") {
            move_dir -= g.camera.forward();
        }
        if inp.is_key_down("A") {
            move_dir -= g.camera.right();
        }
        if inp.is_key_down("D") {
            move_dir += g.camera.right();
        }
    }
    move_dir.y = 0.0;
    if move_dir.length_squared() > 0.001 {
        let speed = 5.0 * dt;
        let mut s = g.scene.borrow_mut();
        if let Some(player_id) = s.find_entity_by_name(PLAYER_NAME) {
            if let Some(mut player) = s.get_entity_mut(player_id) {
                player.transform.position += move_dir.normalize() * speed;
            }
            s.update_entity_collider(player_id);
        }
    }

    {
        let inp = g.input.borrow();
        let look = 90.0 * dt;
        if inp.is_key_down("LEFT") {
            g.camera.yaw -= look;
        }
        if inp.is_key_down("RIGHT") {
            g.camera.yaw += look;
        }
        if inp.is_key_down("UP") {
            g.camera.pitch += look;
        }
        if inp.is_key_down("DOWN") {
            g.camera.pitch -= look;
        }
        g.camera.pitch = g.camera.pitch.clamp(-80.0, 80.0);
    }

    let s = g.scene.borrow();
    let player = s
        .find_entity_by_name(PLAYER_NAME)
        .and_then(|id| s.get_entity(id));
    if let Some(player) = player {
        let offset = -g.camera.forward() * 4.5 + Vec3::new(0.0, 1.5, 0.0);
        g.camera.position = player.transform.position + offset;
    }
}

fn tick_nav(g: &mut GameWorld, dt: f32) {
    let mut s = g.scene.borrow_mut();
    let nav = g.nav.borrow();
    nav.tick_nav_agents(&mut s, dt);
}

/// Fire a hitscan ray on Space / left-click and damage whatever it strikes.
fn hitscan(g: &mut GameWorld) {
    let should_shoot = {
        let inp = g.input.borrow();
        inp.mouse_left_clicked || inp.space_pressed
    };
    if !should_shoot {
        return;
    }
    {
        let mut inp = g.input.borrow_mut();
        inp.mouse_left_clicked = false;
        inp.space_pressed = false;
        inp.set_key_state("SPACE", false);
    }
    g.console
        .borrow_mut()
        .info("💥 Fired hitscan laser beam!".to_string());

    let ray = Ray::new(g.camera.position, g.camera.forward());
    let hit = {
        let s = g.scene.borrow();
        cast_ray_in_scene(&ray, &s)
    };
    match hit {
        Some((hit_id, hit_t)) => {
            let name = g.scene.borrow().get_entity(hit_id).map(|e| e.name.clone());
            if let Some(name) = name {
                g.console.borrow_mut().info(format!(
                    "  🎯 Direct hit! Struck {} at distance t={:.2} units",
                    name, hit_t
                ));
            }
            g.script_manager.trigger_damage(hit_id, 25.0);
        }
        None => g
            .console
            .borrow_mut()
            .info("  💨 Shot missed into empty space.".to_string()),
    }
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
