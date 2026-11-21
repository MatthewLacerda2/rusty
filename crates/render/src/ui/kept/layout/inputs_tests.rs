use glam::{Vec2, Vec3};

use std::rc::Rc;

use super::{element, ui, Rig, Ui, Why};
use rusty_components::{CanvasComponent, CanvasRenderMode, WorldAnchor};
use rusty_scene::Camera;

fn change(i: usize, ui: &mut Ui, rig: &mut Rig) -> &'static str {
    let w = &mut ui.scene.world;
    match i {
        0 => {
            _ = w
                .rect_transform_mut(ui.plain)
                .map(|mut r| r.size_delta.x = 90.0)
        }
        1 => _ = w.text_mut(ui.sized).map(|mut t| t.text = "AMMO 300".into()),
        2 => {
            _ = w
                .image_mut(ui.icon)
                .map(|mut m| m.texture = Some("b.png".into()))
        }
        3 => {
            _ = w
                .transform_mut(ui.plain)
                .map(|mut t| t.scale = Vec3::splat(2.0))
        }
        4 => rig.screen = Vec2::new(1280.0, 720.0),
        5 => Rc::make_mut(&mut rig.textures).set("b.png", Vec2::splat(9.0)),
        6 => _ = w.set_active(ui.sized, false),
        _ => {
            _ = w
                .canvas_mut(ui.canvas)
                .map(|mut c| c.reference_resolution.x = 800.0)
        }
    }
    let names = [
        "a rect",
        "a sized string",
        "a fitted texture",
        "an element's scale",
    ];
    let more = [
        "a resize",
        "a texture size",
        "deactivated",
        "a canvas's scale",
    ];
    names.into_iter().chain(more).nth(i).unwrap_or("?")
}

#[test]
fn a_still_ui_lays_out_once_and_each_input_lays_it_out_again() {
    let (mut ui, mut rig) = (ui(), Rig::new());
    assert!(rig.frame(&ui.scene.world), "the first frame computes");
    assert!(!rig.frame(&ui.scene.world), "a still frame keeps it");
    let (s, c) = (Why::Structure, Why::Content);
    let whys = [
        s,
        c,
        c,
        Why::Transform,
        Why::Screen,
        Why::Textures,
        s,
        Why::Screen,
    ];
    for (i, why) in whys.into_iter().enumerate() {
        let name = change(i, &mut ui, &mut rig);
        assert!(rig.frame(&ui.scene.world), "{name}: computes again");
        assert_eq!(rig.why(), Some(why), "{name}: says why");
        assert!(!rig.frame(&ui.scene.world), "{name}: then keeps it");
    }
}

#[test]
fn what_only_places_the_rects_lays_nothing_out() {
    let mut ui = ui();
    let mut rig = Rig::new();
    let w = &mut ui.scene.world;
    let mut canvas = w.canvas_mut(ui.canvas).expect("canvas");
    (canvas.render_mode, canvas.sway) = (CanvasRenderMode::ScreenSpaceCamera, 1.0);
    drop(canvas);
    let sign = ui.scene.add_entity("Sign".to_string());
    let world_canvas = CanvasComponent {
        render_mode: CanvasRenderMode::WorldSpace,
        ..Default::default()
    };
    ui.scene.world.set_canvas(sign, Some(world_canvas));
    element(&mut ui.scene, sign);
    assert!(rig.frame(&ui.scene.world));
    for i in 0..3 {
        let cam = rig.camera.as_mut().expect("camera");
        cam.yaw += 5.0;
        rusty_ui::sway::update_sway(&mut ui.scene.world, cam, 1.0 / 60.0);
        let w = &mut ui.scene.world;
        w.text_mut(ui.plain).expect("text").text = format!("AMMO {i}");
        w.transform_mut(sign).expect("t").position.x = i as f32;
        assert!(
            !rig.frame(&ui.scene.world),
            "tick {i}: the spaces move, the rects hold"
        );
    }
}

#[test]
fn a_marker_lays_out_again_when_the_camera_or_its_target_moves() {
    let mut ui = ui();
    let mut rig = Rig::new();
    let crate_ = ui.scene.add_entity("Crate".to_string());
    let bystander = ui.scene.add_entity("Bystander".to_string());
    assert!(rig.frame(&ui.scene.world));
    let marker = element(&mut ui.scene, ui.canvas);
    let anchor = WorldAnchor {
        target: Some(crate_),
        ..Default::default()
    };
    let w = &mut ui.scene.world;
    w.rect_transform_mut(marker).expect("rt").world_anchor = Some(anchor);
    assert!(rig.frame(&ui.scene.world), "a marker appears");
    assert!(!rig.frame(&ui.scene.world), "a still view keeps it");
    w_move(&mut ui.scene.world, bystander);
    assert!(!rig.frame(&ui.scene.world), "an entity no marker follows");
    w_move(&mut ui.scene.world, crate_);
    assert!(rig.frame(&ui.scene.world), "the target moved");
    rig.camera.as_mut().expect("camera").yaw += 1.0;
    assert!(rig.frame(&ui.scene.world), "the camera turned");
    rig.camera = None;
    assert!(
        rig.frame(&ui.scene.world),
        "no camera: the marker sits by its anchors"
    );
    assert!(!rig.frame(&ui.scene.world));
    rig.camera = Some(Camera::new(Vec3::ZERO, 0.0, 0.0));
    assert!(rig.frame(&ui.scene.world), "a camera again");
    let screen = rig.screen;
    rig.screen = screen * Vec2::new(1.0, 0.5);
    assert!(rig.frame(&ui.scene.world), "a resize");
}

fn w_move(world: &mut rusty_ecs::World, id: u32) {
    world.transform_mut(id).expect("transform").position.y += 1.0;
}
