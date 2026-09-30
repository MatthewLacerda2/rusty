//! src/ui/sway.rs — the `ScreenSpaceCamera` canvas lag (#429).
//!
//! A visor HUD that swings a little behind the view when the camera turns, then
//! settles back. Each tick, every `ScreenSpaceCamera` canvas's lag grows by how far
//! the camera turned (yaw, pitch) and decays exponentially with the time constant
//! [`SWAY_RECOVERY`]; `canvas.sway` scales how much of it shows (`ui::space`). The
//! state is a runtime-only slot on the canvas (never saved), advanced on the fixed
//! tick, so the lag is a pure function of the camera's path — deterministic.

use glam::Vec2;

use crate::components::CanvasRenderMode;
use crate::ecs::World;
use crate::scene::Camera;

/// How fast a swayed canvas settles: the lag's exponential time constant, seconds.
pub const SWAY_RECOVERY: f32 = 0.15;
/// The most a canvas lags, degrees per axis — a whip-turn never flings the HUD away.
pub const MAX_SWAY: f32 = 30.0;

/// Advance every `ScreenSpaceCamera` canvas's lag by one `dt`-second tick of `camera`.
pub fn update_sway(world: &mut World, camera: &Camera, dt: f32) {
    let look = Vec2::new(camera.yaw, camera.pitch);
    let decay = (-dt.max(0.0) / SWAY_RECOVERY).exp();
    for id in world.ids_with_canvas() {
        let Some(mut canvas) = world.canvas_mut(id) else {
            continue;
        };
        if canvas.render_mode != CanvasRenderMode::ScreenSpaceCamera {
            continue;
        }
        let state = &mut canvas.sway_state;
        let turned = state.last_look.map_or(Vec2::ZERO, |last| wrap(look - last));
        state.offset =
            ((state.offset + turned) * decay).clamp(Vec2::splat(-MAX_SWAY), Vec2::splat(MAX_SWAY));
        state.last_look = Some(look);
    }
}

/// An angle difference folded into `(-180, 180]` degrees, per axis — turning across
/// the ±180° yaw seam is a small turn, not a full one.
fn wrap(d: Vec2) -> Vec2 {
    d - (d / 360.0).round() * 360.0
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;
    use crate::components::CanvasComponent;
    use crate::scene::Scene;

    fn camera_canvas() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Visor".to_string());
        let canvas = CanvasComponent {
            render_mode: CanvasRenderMode::ScreenSpaceCamera,
            sway: 1.0,
            ..Default::default()
        };
        scene.world.set_canvas(id, Some(canvas));
        (scene, id)
    }

    fn offset(scene: &Scene, id: u32) -> Vec2 {
        scene.world.canvas(id).expect("canvas").sway_state.offset
    }

    #[test]
    fn a_turn_lags_then_settles() {
        let (mut scene, id) = camera_canvas();
        let mut cam = Camera::new(Vec3::ZERO, 0.0, 0.0);
        update_sway(&mut scene.world, &cam, 0.02);
        assert_eq!(
            offset(&scene, id),
            Vec2::ZERO,
            "the first tick only records"
        );
        cam.yaw = 10.0;
        cam.pitch = -4.0;
        update_sway(&mut scene.world, &cam, 0.02);
        let lag = offset(&scene, id);
        assert!(lag.x > 8.0 && lag.x < 10.0 && lag.y < -3.0, "{lag}");
        for _ in 0..100 {
            update_sway(&mut scene.world, &cam, 0.02);
        }
        assert!(offset(&scene, id).length() < 1e-3);
    }

    #[test]
    fn the_yaw_seam_is_a_small_turn_and_the_lag_is_capped() {
        let (mut scene, id) = camera_canvas();
        let mut cam = Camera::new(Vec3::ZERO, 179.0, 0.0);
        update_sway(&mut scene.world, &cam, 0.0);
        cam.yaw = -179.0;
        update_sway(&mut scene.world, &cam, 0.0);
        assert!((offset(&scene, id).x - 2.0).abs() < 1e-3);
        cam.yaw = 120.0;
        update_sway(&mut scene.world, &cam, 0.0);
        assert_eq!(offset(&scene, id).x, -MAX_SWAY);
    }
}
