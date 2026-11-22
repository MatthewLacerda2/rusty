use glam::Vec2;

use rusty_components::{CanvasRenderMode, CanvasSway};
use rusty_ecs::World;
use rusty_scene::Camera;

/// How fast a swayed canvas settles: the lag's exponential time constant, seconds.
pub const SWAY_RECOVERY: f32 = 0.15;
/// The most a canvas lags, degrees per axis — a whip-turn never flings the HUD away.
const MAX_SWAY: f32 = 30.0;

/// A lag this small (degrees, per axis) is snapped to none, so a settled HUD
/// stops changing instead of decaying for ever.
const SETTLED: f32 = 1e-4;

/// Advance every `ScreenSpaceCamera` canvas's lag by one `dt`-second tick of `camera`.
pub fn update_sway(world: &mut World, camera: &Camera, dt: f32) {
    let look = Vec2::new(camera.yaw, camera.pitch);
    let decay = (-dt.max(0.0) / SWAY_RECOVERY).exp();
    for id in world.ids_with_canvas() {
        let Some(state) = world
            .canvas(id)
            .filter(|c| c.render_mode == CanvasRenderMode::ScreenSpaceCamera)
            .map(|c| c.sway_state)
        else {
            continue;
        };
        let turned = state.last_look.map_or(Vec2::ZERO, |last| wrap(look - last));
        let offset =
            ((state.offset + turned) * decay).clamp(Vec2::splat(-MAX_SWAY), Vec2::splat(MAX_SWAY));
        let offset = Vec2::select(offset.abs().cmplt(Vec2::splat(SETTLED)), Vec2::ZERO, offset);
        let next = CanvasSway {
            offset,
            last_look: Some(look),
        };
        if next != state {
            if let Some(mut canvas) = world.canvas_mut(id) {
                canvas.sway_state = next;
            }
        }
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
    use rusty_components::CanvasComponent;
    use rusty_scene::Scene;

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

    #[test]
    fn only_a_canvas_whose_lag_moved_is_written() {
        let (mut scene, id) = camera_canvas();
        let overlay = scene.add_entity("Hud".to_string());
        scene
            .world
            .set_canvas(overlay, Some(CanvasComponent::default()));
        let mut cam = Camera::new(Vec3::ZERO, 0.0, 0.0);
        let mut cursor = scene.world.change_cursor::<CanvasComponent>();
        let mut written = |scene: &Scene| match scene.world.changes(&mut cursor) {
            rusty_ecs::Changes::Ids(ids) => ids,
            rusty_ecs::Changes::Rebuild => panic!("no rebuild"),
        };
        update_sway(&mut scene.world, &cam, 0.02);
        assert_eq!(written(&scene), [id], "the first tick records the look");
        cam.yaw = 10.0;
        update_sway(&mut scene.world, &cam, 0.02);
        assert_eq!(written(&scene), [id], "a turn lags");
        for _ in 0..400 {
            update_sway(&mut scene.world, &cam, 0.02);
        }
        assert_eq!(offset(&scene, id), Vec2::ZERO, "settled to none");
        written(&scene);
        update_sway(&mut scene.world, &cam, 0.02);
        assert!(
            written(&scene).is_empty(),
            "a settled or overlay canvas is not written"
        );
    }
}
