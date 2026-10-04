//! Test fixture for custom ui shaders on the GPU (#427): bake a ui recipe where the
//! renderer resolves it, build a canvas with one shaded white graphic over an opaque
//! black backdrop (one reference unit per pixel), and render it to pixels.

use std::cell::RefCell;

use glam::{Vec2, Vec3, Vec4};
use mlua::Lua;

use crate::components::{CanvasComponent, ImageComponent, RectTransformComponent, UiShader};
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{Camera, Scene};
use crate::shadergen::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};
use crate::shadergen::{bake_recipe, DEFAULT_OUT_DIR, ENGINE_SHADER_DIR};

/// The shot's size in pixels (and the canvas's in reference units).
pub(super) const RES: u32 = 64;

/// The shaded graphic's rect: `[x, y, width, height]`, centred.
pub(super) const RECT: [f32; 4] = [16.0, 16.0, 32.0, 32.0];

/// A ui shader baked under a name unique to this process; removed on drop.
pub(super) struct Baked(pub(super) String);

impl Baked {
    /// Bake `blocks` (each an id and its params) as a ui shader tagged `tag`.
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
        bake_recipe(&recipe, ENGINE_SHADER_DIR, DEFAULT_OUT_DIR).expect("ui bake succeeds");
        Self(name)
    }
}

impl Drop for Baked {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.0));
        }
    }
}

/// A bottom-left-anchored element at `r` (`[x, y, w, h]`) under `parent`.
pub(super) fn element(scene: &mut Scene, parent: u32, r: [f32; 4], color: Option<Vec4>) -> u32 {
    let id = scene.add_entity("E".to_string());
    let rt = RectTransformComponent {
        anchor_min: Vec2::ZERO,
        anchor_max: Vec2::ZERO,
        pivot: Vec2::ZERO,
        anchored_position: Vec2::new(r[0], r[1]),
        size_delta: Vec2::new(r[2], r[3]),
        world_anchor: None,
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

/// A canvas over a black backdrop with a white Image at [`RECT`] naming `shader`
/// (none: the standard shader). Returns the scene, the canvas and the Image's id.
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

/// Name `shader` on `id`'s Image.
pub(super) fn shade(scene: &mut Scene, id: u32, shader: Option<&str>) {
    scene.world.image_mut(id).expect("image").shader = shader.map(|name| UiShader {
        name: name.to_string(),
        ..Default::default()
    });
}

/// Run `script` against the `UI` namespace (a throwaway screen and camera).
pub(super) fn lua(scene: &RefCell<Scene>, script: &str) {
    let screen = RefCell::new(crate::ui::ScreenSize::default());
    let video = RefCell::new(crate::core::video::VideoSettings::default());
    let camera = RefCell::new(Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0));
    let lua = Lua::new();
    lua.scope(|s| {
        crate::api::ui::register(&lua, s, scene, (&screen, &video), &camera).unwrap();
        lua.load(script).exec()
    })
    .unwrap();
}

/// A rendered frame's pixels, RGBA, addressed bottom-up like the canvas.
pub(super) struct Shot(Vec<u8>);

impl Shot {
    pub(super) fn px(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (((RES - 1 - y) * RES + x) * 4) as usize;
        [self.0[i], self.0[i + 1], self.0[i + 2], self.0[i + 3]]
    }

    /// The red channel along row `y`, over the graphic's rect.
    pub(super) fn row(&self, y: u32) -> Vec<u8> {
        let x0 = RECT[0] as u32;
        (x0..x0 + RECT[2] as u32)
            .map(|x| self.px(x, y)[0])
            .collect()
    }
}

/// Render `scene` in `view` (made on first use) and read it back.
pub(super) fn shot(renderer: &mut Renderer, view: &mut Option<RenderView>, scene: &Scene) -> Shot {
    let view = view.get_or_insert_with(|| {
        RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, RES, RES, 2)
    });
    let out = view.color_target_view().expect("offscreen target");
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    renderer.render(view, scene, &cam, &out, false);
    let target = view.color_target().expect("target");
    let pixels = readback::read_texture_rgba8(&renderer.device, &renderer.queue, target, RES, RES);
    assert!(pixels.chunks(4).any(|p| p[3] > 0), "{DROPPED}");
    Shot(pixels)
}

/// Why an all-zero-alpha shot is not a shader's fault (#769). Every frame is opaque
/// (the sky; the backdrop over it), and a ui shader that fails to resolve falls back
/// to the standard one, so its graphic still draws. A readback with no alpha anywhere
/// means none of the frame's GPU work landed: the device failed the command buffers
/// and the backend reported nothing (wgpu's Metal backend counts a failed command
/// buffer as completed and never surfaces it).
const DROPPED: &str = "the GPU dropped the frame: no pixel has any alpha, not even the \
    opaque backdrop. Not a ui-shader or block bug: the device failed the command \
    buffers silently (#769)";

/// Bake `blocks`, shade a fresh scene with it at UI clock `time`, and shoot it.
pub(super) fn look(tag: &str, blocks: &[(&str, &[(&str, f32)])], time: f32) -> Option<Shot> {
    let mut renderer = crate::render::test_gpu::headless_or_skip(RES, RES)?;
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
