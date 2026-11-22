use glam::{Vec2, Vec3, Vec4};

use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::{CanvasComponent, ImageComponent, RectTransformComponent, UiShader};
use rusty_scene::{Camera, Scene};
use rusty_shadergen::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};
use rusty_shadergen::{bake_recipe, engine_shader_dir, DEFAULT_OUT_DIR};

pub(super) const RES: u32 = 64;

pub(super) const RECT: [f32; 4] = [16.0, 16.0, 32.0, 32.0];

pub(super) struct Baked(pub(super) String);

impl Baked {
    pub(super) fn new(tag: &str, blocks: &[(&str, &[(&str, f32)])]) -> Self {
        let name = format!("test_ui_{tag}_{}", std::process::id());
        let blocks = blocks.iter().map(|(id, params)| BlockSel {
            id: id.to_string(),
            params: params
                .iter()
                .map(|(k, v)| (k.to_string(), ParamValue::Scalar(*v)))
                .collect(),
        });
        let recipe = ShaderRecipe {
            pass: PassKind::Ui,
            name: name.clone(),
            blocks: blocks.collect(),
        };
        bake_recipe(&recipe, engine_shader_dir(), DEFAULT_OUT_DIR).expect("ui bake succeeds");
        Self(name)
    }
}

impl Drop for Baked {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json", "recipe.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.0));
        }
    }
}

pub(super) fn element(scene: &mut Scene, parent: u32, r: [f32; 4], color: Option<Vec4>) -> u32 {
    let id = scene.add_entity("E".to_string());
    let rt = RectTransformComponent {
        anchor_min: Vec2::ZERO,
        anchor_max: Vec2::ZERO,
        pivot: Vec2::ZERO,
        anchored_position: Vec2::new(r[0], r[1]),
        size_delta: Vec2::new(r[2], r[3]),
        ..Default::default()
    };
    scene.world.set_rect_transform(id, Some(rt));
    let image = color.map(|color| ImageComponent {
        color,
        ..Default::default()
    });
    scene.world.set_image(id, image);
    scene.set_parent(id, Some(parent)).expect("parent exists");
    id
}

pub(super) fn scene(shader: Option<&str>) -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    let root = scene.add_entity("Canvas".to_string());
    let canvas = CanvasComponent {
        reference_resolution: Vec2::splat(RES as f32),
        ..Default::default()
    };
    scene.world.set_canvas(root, Some(canvas));
    let full = RES as f32;
    element(
        &mut scene,
        root,
        [0.0, 0.0, full, full],
        Some(Vec4::new(0.0, 0.0, 0.0, 1.0)),
    );
    let id = element(&mut scene, root, RECT, Some(Vec4::ONE));
    shade(&mut scene, id, shader);
    (scene, root, id)
}

pub(super) fn shade(scene: &mut Scene, id: u32, shader: Option<&str>) {
    scene.world.image_mut(id).expect("image").shader = shader.map(|name| UiShader {
        name: name.to_string(),
        ..Default::default()
    });
}

pub(super) fn set_param(scene: &mut Scene, id: u32, name: &str, value: f32) -> Vec<f32> {
    let mut image = scene.world.image_mut(id).expect("the fixture's image");
    let shader = image.shader.as_mut().expect("a ui shader");
    let layout = rusty_shadergen::params::load(&shader.name).expect("a baked layout");
    let slot = layout.resolve(name).expect("a known param");
    let value = layout.coerce(slot, vec![value]).expect("a fitting value");
    shader.params.insert(layout.name(slot), value.clone());
    value
}

pub(super) struct Shot(Vec<u8>);

impl Shot {
    pub(super) fn px(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (((RES - 1 - y) * RES + x) * 4) as usize;
        [self.0[i], self.0[i + 1], self.0[i + 2], self.0[i + 3]]
    }

    pub(super) fn row(&self, y: u32) -> Vec<u8> {
        let x0 = RECT[0] as u32;
        (x0..x0 + RECT[2] as u32)
            .map(|x| self.px(x, y)[0])
            .collect()
    }
}

pub(super) fn shot(renderer: &mut Renderer, view: &mut Option<RenderView>, scene: &Scene) -> Shot {
    let view = view
        .get_or_insert_with(|| RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, RES, RES));
    let out = view.color_target_view().expect("offscreen target");
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    renderer.render(view, scene, &cam, &out, false);
    let target = view.color_target().expect("target");
    let pixels = readback::read_texture_rgba8(&renderer.device, &renderer.queue, target, RES, RES);
    assert!(pixels.chunks(4).any(|p| p[3] > 0), "{DROPPED}");
    Shot(pixels)
}

const DROPPED: &str = "the GPU dropped the frame: no pixel has any alpha, not even the \
    opaque backdrop. Not a ui-shader or block bug: the device failed the command \
    buffers silently (#769)";

pub(super) fn look(tag: &str, blocks: &[(&str, &[(&str, f32)])], time: f32) -> Option<Shot> {
    let mut renderer = crate::test_gpu::headless_or_skip(RES, RES)?;
    let baked = Baked::new(tag, blocks);
    let (mut scene, _, _) = scene(Some(&baked.0));
    scene.ui_time = time;
    let shot = shot(&mut renderer, &mut None, &scene);
    assert!(
        renderer.ui_renderer.shaders.built(&baked.0),
        "ui shader {:?} did not build, so the graphic drew with the standard shader",
        baked.0
    );
    Some(shot)
}
