//! A scrolling Cutout surface primes as it draws (#1499): a `uv_scroll` variant with
//! no cut moves the UVs `fs_main` clips its albedo at, so its prepass must clip at the
//! moved UVs too. Clipped at the raw ones, the prepass keeps texels the forward pass
//! drops, and the primed frame shows a hole where the far wall should be. Skips with
//! no adapter.

use glam::{Quat, Vec3};

use crate::components::RenderMode;
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, default_post_processing, material, Primitive};
use crate::scene::{Camera, Scene};
use crate::shadergen::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};
use crate::shadergen::{bake_recipe, engine_shader_dir, DEFAULT_OUT_DIR};

const RES: u32 = 64;

/// A `uv_scroll` surface shader moving u by -0.5 a game second, and a Cutout albedo
/// clear where u < 0.5; both removed on drop.
struct Scroll {
    shader: String,
    albedo: String,
}

impl Scroll {
    fn bake() -> Self {
        let shader = format!("test_primed_scroll_{}", std::process::id());
        let speed = ("speed".to_string(), ParamValue::Vector(vec![-0.5, 0.0]));
        let recipe = ShaderRecipe {
            pass: PassKind::Surface,
            name: shader.clone(),
            blocks: vec![BlockSel {
                id: "uv_scroll".into(),
                params: [speed].into_iter().collect(),
            }],
        };
        bake_recipe(&recipe, engine_shader_dir(), DEFAULT_OUT_DIR).expect("bake succeeds");
        let image = image::RgbaImage::from_fn(4, 1, |x, _| {
            image::Rgba([255, 255, 255, if x < 2 { 0 } else { 255 }])
        });
        let path = crate::test_temp::dir().join(format!("{shader}_albedo.png"));
        image.save(&path).expect("albedo written");
        let albedo = path.to_string_lossy().into_owned();
        Self { shader, albedo }
    }
}

impl Drop for Scroll {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json", "recipe.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.shader));
        }
        let _ = std::fs::remove_file(&self.albedo);
    }
}

/// A box at `position` scaled by `scale`; returns its id.
fn block(scene: &mut Scene, position: Vec3, scale: Vec3) -> u32 {
    let id = create_entity(scene, "Block", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (position, scale);
    id
}

/// A far wall, and in front of it the scrolling Cutout wall one game second in,
/// under an AO volume.
fn scene(s: &Scroll) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.shader_time = 1.0;
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.0);
    block(
        &mut scene,
        Vec3::new(0.0, 0.0, -10.0),
        Vec3::new(20.0, 20.0, 0.5),
    );
    let front = block(
        &mut scene,
        Vec3::new(0.0, 0.0, -4.0),
        Vec3::new(4.0, 4.0, 0.2),
    );
    let key = material::ensure_material_key(&mut scene, front).unwrap();
    let m = &mut scene.materials;
    material::set_base_color_map(m, &key, s.albedo.clone());
    material::set_alpha_cutoff(m, &key, 0.5);
    material::set_render_mode(m, &key, RenderMode::Cutout);
    material::set_shader(m, &key, s.shader.clone());
    let volume = create_entity(&mut scene, "Volume", None);
    let mut vc = default_post_processing();
    vc.ssao.active = true;
    scene.world.set_post_processing(volume, Some(vc));
    let lens = crate::components::CameraComponent::default();
    scene.world.set_camera(volume, Some(lens));
    scene
}

/// One frame of `scene`, primed or not, as RGBA8.
fn frame(r: &mut Renderer, scene: &Scene, primed: bool) -> Vec<u8> {
    r.depth_priming = primed;
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 2.0), -90.0, 0.0);
    r.render(&mut view, scene, &cam, &out, false);
    let primes = primed && r.priming_recommended;
    assert_eq!(r.frame_counters.primed_passes, u32::from(primes));
    let target = view.color_target().unwrap();
    readback::read_texture_rgba8(&r.device, &r.queue, target, RES, RES)
}

#[test]
fn gpu_a_scrolling_cutout_primes_to_the_unprimed_frame() {
    let s = Scroll::bake();
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let scene = scene(&s);
    let plain = frame(&mut r, &scene, false);
    let id = r.surface_shaders.pipeline_id(&r.device, Some(&s.shader));
    assert!(
        r.surface_shaders.cut(id).is_some(),
        "the variant built its depth"
    );
    // On Metal nothing primes: the second frame is unprimed too, and must match.
    let primed = frame(&mut r, &scene, true);
    let differing = plain
        .chunks(4)
        .zip(primed.chunks(4))
        .filter(|(a, b)| a != b);
    assert_eq!(differing.count(), 0, "priming changed pixels");
}
