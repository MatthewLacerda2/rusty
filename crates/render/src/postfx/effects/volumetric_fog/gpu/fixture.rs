use glam::{Vec2, Vec3};

use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::{
    LightComponent, LightMode, LightType, MaterialAsset, MaterialComponent, Tonemap, VolumetricFog,
};
use rusty_core::settings::quality::EffectQuality;
use rusty_scene::create::{create_entity, default_light, default_post_processing, Primitive};
use rusty_scene::{Camera, Scene};

pub(super) const W: u32 = 96;
pub(super) const H: u32 = 72;

pub(super) const LAMP_AT: Vec3 = Vec3::new(0.0, 1.5, -4.0);

pub(super) const BRIGHT: f32 = 3.0;

pub(super) fn renderer() -> Option<Renderer> {
    crate::test_gpu::headless_or_skip(W, H)
}

pub(super) fn eye() -> Camera {
    Camera::new(Vec3::new(0.0, 1.5, 0.0), -90.0, 0.0)
}

pub(super) fn fog() -> VolumetricFog {
    VolumetricFog {
        active: true,
        mean_free_path: 10.0,
        maximum_height: 100.0,
        depth_extent: 16.0,
        quality: EffectQuality::Custom,
        resolution: 48,
        slices: 32,
        ..VolumetricFog::default()
    }
}

pub(super) fn block(scene: &mut Scene, name: &str, at: (Vec3, Vec3)) -> u32 {
    tinted(scene, name, at, [0.0; 3])
}

pub(super) fn tinted(
    scene: &mut Scene,
    name: &str,
    (position, scale): (Vec3, Vec3),
    color: [f32; 3],
) -> u32 {
    let id = create_entity(scene, name, Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (position, scale);
    drop(t);
    let asset = MaterialAsset {
        base_color: color,
        roughness: 1.0,
        ..MaterialAsset::default()
    };
    scene.materials.insert(name.into(), asset);
    scene
        .world
        .set_material(id, Some(MaterialComponent::new(name.to_string())));
    scene.world.set_static(id, true);
    id
}

pub(super) fn room(fog: VolumetricFog) -> Scene {
    let mut scene = open(fog);
    block(
        &mut scene,
        "back",
        (Vec3::new(0.0, 3.0, -12.0), Vec3::new(20.0, 6.0, 0.2)),
    );
    scene
}

pub(super) fn open(fog: VolumetricFog) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_color = Vec3::ZERO;
    let cam = scene.add_entity("Camera".to_string());
    scene.world.set_camera(cam, Some(Default::default()));
    let mut volume = default_post_processing();
    (volume.bloom.active, volume.ssao.active, volume.ssr.active) = (false, false, false);
    volume.tonemapping.mode = Tonemap::None;
    volume.volumetric_fog = fog;
    scene.world.set_post_processing(cam, Some(volume));
    block(
        &mut scene,
        "floor",
        (Vec3::new(0.0, -0.05, -6.0), Vec3::new(20.0, 0.1, 20.0)),
    );
    scene
}

pub(super) fn lamp(scene: &mut Scene, at: Vec3, intensity: f32, mode: LightMode) -> u32 {
    let id = scene.add_entity("Lamp".to_string());
    scene.world.transform_mut(id).unwrap().position = at;
    let light = LightComponent {
        light_type: LightType::Point,
        intensity,
        range: 6.0,
        cast_shadows: false,
        mode,
        ..default_light()
    };
    scene.world.set_light(id, Some(light));
    if mode == LightMode::Baked {
        scene.lightmaps.took_in_baked(id);
    }
    id
}

pub(super) fn set_fog(scene: &mut Scene, fog: VolumetricFog) {
    for id in scene.world.ids_with_post_processing() {
        scene.world.post_processing_mut(id).unwrap().volumetric_fog = fog;
    }
}

pub(super) fn view(r: &Renderer) -> RenderView {
    RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H)
}

pub(super) fn draw(r: &mut Renderer, view: &mut RenderView, scene: &Scene) -> Vec<u8> {
    let out = view.color_target_view().unwrap();
    r.render(view, scene, &eye(), &out, false);
    let (w, h) = (view.size().width, view.size().height);
    readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), w, h)
}

pub(super) fn settled(r: &mut Renderer, scene: &Scene, frames: u32) -> Vec<u8> {
    let mut v = view(r);
    let mut last = Vec::new();
    for _ in 0..frames {
        last = draw(r, &mut v, scene);
    }
    last
}

pub(super) fn glow(frame: &[u8], p: Vec3) -> f32 {
    (0..3).map(|c| channel(frame, p, c)).sum()
}

pub(super) fn channel(frame: &[u8], p: Vec3, c: usize) -> f32 {
    let clip = eye().build_view_projection(W as f32 / H as f32) * p.extend(1.0);
    let ndc: Vec2 = clip.truncate().truncate() / clip.w;
    let x = (((ndc.x * 0.5 + 0.5) * W as f32) as u32).min(W - 1);
    let y = (((0.5 - ndc.y * 0.5) * H as f32) as u32).min(H - 1);
    let mut sum = 0.0;
    for y in y.saturating_sub(2)..(y + 3).min(H) {
        for x in x.saturating_sub(2)..(x + 3).min(W) {
            let i = ((y * W + x) * 4) as usize;
            sum += rusty_core::color::srgb_to_linear(frame[i + c]);
        }
    }
    sum
}
