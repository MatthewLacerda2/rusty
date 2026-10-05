//! A custom ui shader on a world canvas (#427): the graphic draws through the
//! variant's world pipeline — a hologram recolours white to blue — and a RectMask
//! still cuts it at the mask's edge, per fragment, like any other world graphic.

use glam::{Vec2, Vec4};
use rusty::components::{CanvasComponent, CanvasRenderMode, RectMaskComponent, UiShader};
use rusty::shadergen::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};
use rusty::shadergen::{bake_recipe, engine_shader_dir, DEFAULT_OUT_DIR};

use super::world_ui_scene::{canvas, dark_scene, dominant, fill, image};
use super::world_ui_screenshot::shot;

/// A blue, non-flickering hologram ui shader; removed on drop.
struct Holo(String);

impl Holo {
    fn bake() -> Self {
        let name = format!("test_world_ui_holo_{}", std::process::id());
        let params =
            [("hue", 240.0), ("flicker", 0.0)].map(|(k, v)| (k.to_string(), ParamValue::Scalar(v)));
        let recipe = ShaderRecipe {
            pass: PassKind::Ui,
            name: name.clone(),
            blocks: vec![BlockSel {
                id: "hologram".into(),
                params: params.into_iter().collect(),
            }],
        };
        bake_recipe(&recipe, engine_shader_dir(), DEFAULT_OUT_DIR).expect("ui bake");
        Self(name)
    }
}

impl Drop for Holo {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.0));
        }
    }
}

#[test]
fn a_custom_shaded_world_graphic_draws_and_is_cut_at_its_mask() {
    let holo = Holo::bake();
    let mut scene = dark_scene();
    // A 2×2 m sign 5 m in front of the camera: pixels ~25..71, centre 48.
    let sign = canvas(
        &mut scene,
        CanvasComponent {
            render_mode: CanvasRenderMode::WorldSpace,
            reference_resolution: Vec2::splat(400.0),
            pixels_per_unit: 200.0,
            ..Default::default()
        },
    );
    let mut half = fill();
    half.anchor_max = Vec2::new(0.5, 1.0);
    let mask = image(&mut scene, sign, half, Vec4::ZERO);
    scene
        .world
        .set_rect_mask(mask, Some(RectMaskComponent::default()));
    let mut over = fill();
    over.anchor_max = Vec2::splat(2.0);
    let child = image(&mut scene, mask, over, Vec4::ONE);
    scene.world.image_mut(child).unwrap().shader = Some(UiShader {
        name: holo.0.clone(),
        ..Default::default()
    });
    let Some(img) = shot(&scene, "rusty_world_ui_shader.png") else {
        return;
    };
    let px = |x: u32, y: u32| img.get_pixel(x, y).0;
    assert!(
        dominant(px(36, 48), 2),
        "hologram blue in the mask: {:?}",
        px(36, 48)
    );
    assert!(
        !dominant(px(60, 48), 2),
        "cut right of it: {:?}",
        px(60, 48)
    );
    assert!(
        !dominant(px(36, 10), 2),
        "cut above the sign: {:?}",
        px(36, 10)
    );
}
