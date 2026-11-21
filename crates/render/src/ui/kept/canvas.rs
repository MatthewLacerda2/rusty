use glam::{Vec2, Vec3};

use super::{hud, Hud, Rig};
use rusty_components::CanvasRenderMode;
use rusty_scene::Camera;

fn visor() -> (Hud, Rig) {
    let mut hud = hud();
    let mut canvas = hud.scene.world.canvas_mut(hud.a).expect("canvas");
    (canvas.render_mode, canvas.sway) = (CanvasRenderMode::ScreenSpaceCamera, 1.0);
    drop(canvas);
    let mut rig = Rig::new();
    rig.screen = Vec2::splat(1000.0);
    rig.camera = Some(Camera::new(Vec3::ZERO, 0.0, 0.0));
    (hud, rig)
}

fn turn(hud: &mut Hud, rig: &mut Rig, yaw: f32) {
    let cam = rig.camera.as_mut().expect("camera");
    cam.yaw += yaw;
    rusty_ui::sway::update_sway(&mut hud.scene.world, cam, 1.0 / 60.0);
}

#[test]
fn a_turning_camera_meshes_no_canvas() {
    let (mut hud, mut rig) = visor();
    assert_eq!(
        rig.frame(&hud.scene),
        2,
        "the first frame builds every canvas"
    );
    for _ in 0..30 {
        turn(&mut hud, &mut rig, 3.0);
        assert_eq!(rig.frame(&hud.scene), 0, "the visor sways on its kept mesh");
    }
    let lag = hud.scene.world.canvas(hud.a).expect("canvas").sway_state;
    assert_ne!(lag.offset, Vec2::ZERO, "the turn did write the visor's lag");
}

const FIELDS: [(&str, usize); 8] = [
    ("render_mode", 1),
    ("sort_order", 0),
    ("reference_resolution", 1),
    ("match_width_or_height", 1),
    ("pixels_per_unit", 0),
    ("plane_distance", 0),
    ("tilt", 0),
    ("sway", 0),
];

#[test]
fn each_canvas_field_rebuilds_only_what_its_mesh_reads() {
    let (mut hud, mut rig) = visor();
    rig.frame(&hud.scene);
    for (step, (field, rebuilt)) in FIELDS.iter().enumerate() {
        let mut c = hud.scene.world.canvas_mut(hud.a).expect("canvas");
        match step {
            0 => c.render_mode = CanvasRenderMode::ScreenSpaceOverlay,
            1 => c.sort_order = 7,
            2 => c.reference_resolution = Vec2::new(1280.0, 720.0),
            3 => c.match_width_or_height = 1.0,
            4 => c.pixels_per_unit = 50.0,
            5 => c.plane_distance = 3.0,
            6 => c.tilt = Vec2::new(10.0, -5.0),
            _ => c.sway = 0.0,
        }
        drop(c);
        if step == 3 {
            let mut c = hud.scene.world.canvas_mut(hud.a).expect("canvas");
            c.render_mode = CanvasRenderMode::ScreenSpaceCamera;
            drop(c);
            assert_eq!(rig.frame(&hud.scene), 1, "{field}, then the camera mode");
        } else {
            assert_eq!(rig.frame(&hud.scene), *rebuilt, "{field}");
        }
        assert_eq!(rig.frame(&hud.scene), 0, "{field}: then still");
    }
}
