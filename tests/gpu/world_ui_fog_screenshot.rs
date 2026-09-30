//! World-space canvases fog like the world (#429 over #437): a red sign and a red
//! box side by side at the same distance, under linear fog, come out the same.

use glam::{Vec2, Vec3, Vec4};
use rusty::components::{CanvasComponent, CanvasRenderMode};
use rusty::scene::{FogMode, FogSettings};

use super::world_ui_scene::{canvas, dark_scene, emissive_box, fill, image};
use super::world_ui_screenshot::{shot, SIZE};

#[test]
fn a_world_canvas_fogs_like_the_world_beside_it() {
    let mut scene = dark_scene();
    scene.fog = FogSettings {
        mode: FogMode::Linear,
        color: Vec3::new(0.0, 0.0, 1.0),
        start: 0.0,
        end: 10.0,
        ..Default::default()
    };
    // A red sign on the right, a red box on the left, both 5 m out: half fogged.
    let sign = CanvasComponent {
        render_mode: CanvasRenderMode::WorldSpace,
        reference_resolution: Vec2::new(300.0, 400.0),
        ..Default::default()
    };
    let sign = canvas(&mut scene, sign);
    scene.world.transform_mut(sign).expect("t").position = Vec3::new(1.5, 0.0, 0.0);
    image(&mut scene, sign, fill(), Vec4::new(1.0, 0.0, 0.0, 1.0));
    let size = Vec3::new(3.0, 4.0, 0.01);
    emissive_box(
        &mut scene,
        "Box",
        Vec3::new(-1.5, 0.0, 0.0),
        size,
        [1.0, 0.0, 0.0],
    );
    let Some(img) = shot(&scene, "rusty_world_ui_fog.png") else {
        return;
    };
    let (sign, boxed) = (
        img.get_pixel(SIZE * 3 / 4, SIZE / 2).0,
        img.get_pixel(SIZE / 4, SIZE / 2).0,
    );
    let close = sign.iter().zip(boxed).all(|(a, b)| a.abs_diff(b) <= 4);
    assert!(close, "sign {sign:?} vs box {boxed:?}");
    assert!(sign[2] > 40 && sign[0] > 40, "fogged half way: {sign:?}");
}
