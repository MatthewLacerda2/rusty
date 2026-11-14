//! What the shadowmask GPU tests share: a white floor, a shadow-casting point lamp
//! 2.5 m over it 5 m out, seen from 1.5 m up looking down -Z and a little down,
//! with no tonemap, so a frame reads back linear; and a bake into a temp folder.

use glam::{Vec2, Vec3};

use crate::bake::lightmap::bake_scene_lightmaps;
use rusty_components::{
    LightComponent, LightMode, LightType, MaterialAsset, MaterialComponent, Tonemap,
};
use rusty_render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_scene::authoring::{create_entity, default_light, default_post_processing, Primitive};
use rusty_scene::lighting::settings::BakeSettings;
use rusty_scene::lighting::MixedLightingMode;
use rusty_scene::{Camera, Scene};

pub(super) const W: u32 = 96;
pub(super) const H: u32 = 72;

/// Where the lamp hangs.
pub(super) const LAMP: Vec3 = Vec3::new(0.0, 2.5, -5.0);

pub(super) fn renderer() -> Option<Renderer> {
    rusty_render::test_gpu::headless_or_skip(W, H)
}

pub(super) fn eye() -> Camera {
    Camera::new(Vec3::new(0.0, 1.5, 0.0), -90.0, -12.0)
}

/// A box named `name` at `position`, `scale`, white, at `roughness`, static or not.
pub(super) fn block(
    scene: &mut Scene,
    name: &str,
    at: (Vec3, Vec3),
    roughness: f32,
    is_static: bool,
) -> u32 {
    let id = create_entity(scene, name, Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = at;
    drop(t);
    let asset = MaterialAsset {
        base_color: [0.9, 0.9, 0.9],
        roughness,
        ..MaterialAsset::default()
    };
    scene.materials.insert(name.into(), asset);
    scene
        .world
        .set_material(id, Some(MaterialComponent::new(name.to_string())));
    scene.world.set_static(id, is_static);
    id
}

/// The camera, a dim ambient, a floor at `roughness` and the lamp in `mode`, the
/// scene on `mixed`. Returns it and the lamp.
pub(super) fn scene(mode: LightMode, mixed: MixedLightingMode, roughness: f32) -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_color = Vec3::splat(0.02);
    scene.lighting_settings.mixed_lighting_mode = mixed;
    let cam = scene.add_entity("Camera".to_string());
    scene.world.set_camera(cam, Some(Default::default()));
    let mut volume = default_post_processing();
    (volume.bloom.active, volume.ssao.active, volume.ssr.active) = (false, false, false);
    volume.tonemapping.mode = Tonemap::None;
    scene.world.set_post_processing(cam, Some(volume));
    let floor = (Vec3::new(0.0, -0.05, -6.0), Vec3::new(12.0, 0.1, 14.0));
    block(&mut scene, "floor", floor, roughness, true);
    let lamp = scene.add_entity("Lamp".to_string());
    scene.world.transform_mut(lamp).unwrap().position = LAMP;
    let light = LightComponent {
        light_type: LightType::Point,
        intensity: 6.0,
        range: 6.0,
        cast_shadows: true,
        mode,
        ..default_light()
    };
    scene.world.set_light(lamp, Some(light));
    (scene, lamp)
}

/// Bake `scene`'s lightmaps into a temp folder `name`.
pub(super) fn bake(scene: &mut Scene, name: &str) {
    let dir = rusty_test_support::temp::dir().join(format!("rusty_shadowmask_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let settings = BakeSettings {
        texels_per_unit: 4.0,
        samples: 16,
        bounces: 1,
        seed: 1,
        max_lightmap_size: 256,
        denoiser: rusty_scene::lighting::settings::Denoiser::Filter,
        filter_radius: 1,
        directional: false,
        backface_tolerance: BakeSettings::DEFAULT_BACKFACE_TOLERANCE,
        ..Default::default()
    };
    let maps = bake_scene_lightmaps(scene, Some(&path), &settings).expect("baked");
    assert!(maps > 0, "the floor is lightmapped");
}

/// The last of 3 frames of `scene` drawn into one fresh view from [`eye`].
pub(super) fn settled(r: &mut Renderer, scene: &Scene) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let mut last = Vec::new();
    for _ in 0..3 {
        let out = view.color_target_view().unwrap();
        r.render(&mut view, scene, &eye(), &out, false);
        last =
            readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), W, H);
    }
    last
}

/// The pixel world point `p` lands on through [`eye`].
pub(super) fn pixel_at(p: Vec3) -> (u32, u32) {
    let clip = eye().build_view_projection(W as f32 / H as f32) * p.extend(1.0);
    let ndc: Vec2 = clip.truncate().truncate() / clip.w;
    let x = ((ndc.x * 0.5 + 0.5) * W as f32) as u32;
    let y = ((0.5 - ndc.y * 0.5) * H as f32) as u32;
    (x.min(W - 1), y.min(H - 1))
}

/// The mean linear luminance-ish (summed RGB / 3) of the 3x3 patch around `p`.
pub(super) fn patch(frame: &[u8], p: Vec3) -> f32 {
    let (x, y) = pixel_at(p);
    let mut sum = 0.0;
    let mut n = 0.0;
    for y in y.saturating_sub(1)..(y + 2).min(H) {
        for x in x.saturating_sub(1)..(x + 2).min(W) {
            let i = ((y * W + x) * 4) as usize;
            for c in 0..3 {
                sum += rusty_core::color::srgb_to_linear(frame[i + c]);
            }
            n += 3.0;
        }
    }
    sum / n
}
