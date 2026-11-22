use std::cell::RefCell;
use std::rc::Rc;

use glam::{Quat, Vec3};

use super::mask_tests::Dissolve;
use super::params_tests::{set_param, set_texture};
use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_scene::create::{create_entity, default_post_processing, Primitive};
use rusty_scene::{Camera, Scene};

const RES: u32 = 96;

fn block(scene: &mut Scene, position: Vec3, scale: Vec3, shader: Option<&str>) -> u32 {
    let id = create_entity(scene, "Block", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (position, scale);
    drop(t);
    if let Some(shader) = shader {
        let key = scene.ensure_material_key(id).unwrap();
        scene.materials.get_mut(&key).unwrap().shader =
            (!shader.is_empty()).then(|| shader.to_owned());
    }
    id
}

pub(crate) fn slab_yard(shader: Option<&str>) -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun =
        rusty_scene::create::create_live_light(&mut scene, "Sun", Primitive::DirectionalLight);
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.3);
    let ground = (Vec3::new(0.0, -0.5, 0.0), Vec3::new(40.0, 1.0, 40.0));
    block(&mut scene, ground.0, ground.1, None);
    let slab = block(&mut scene, Vec3::Y * 3.0, Vec3::new(6.0, 0.2, 6.0), shader);
    (scene, slab)
}

pub(crate) fn dissolve(scene: &Rc<RefCell<Scene>>, id: u32, d: &Dissolve, amount: f32) {
    set_texture(scene, id, "mask", &d.mask);
    set_param(scene, id, "dissolve.amount", amount);
}

fn frame(r: &mut Renderer, scene: &Scene, cam: &Camera) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, scene, cam, &out, false);
    let target = view.color_target().unwrap();
    readback::read_texture_rgba8(&r.device, &r.queue, target, RES, RES)
}

pub(crate) fn ground_halves(r: &mut Renderer, scene: &Scene) -> [f32; 2] {
    [-1.5, 1.5].map(|x| ground_at(r, scene, x))
}

pub(crate) fn ground_at(r: &mut Renderer, scene: &Scene, x: f32) -> f32 {
    let cam = Camera::new(Vec3::new(x, 1.5, -0.8), -90.0, -89.0);
    let px = frame(r, scene, &cam);
    let rgb = px.chunks(4).flat_map(|p| &p[..3]);
    rgb.map(|&c| f32::from(c)).sum::<f32>() / (RES * RES * 3) as f32
}

#[test]
fn gpu_a_half_dissolved_slab_casts_a_shadow_with_a_hole() {
    let mask = image::RgbaImage::from_fn(4, 1, |x, _| image::Rgba([255 * (x / 2) as u8; 4]));
    let d = Dissolve::with_mask("half", &mask);
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let (scene, slab) = slab_yard(Some(&d.shader));
    let scene = std::rc::Rc::new(RefCell::new(scene));

    dissolve(&scene, slab, &d, 0.0);
    let whole = ground_halves(&mut r, &scene.borrow());
    dissolve(&scene, slab, &d, 0.5);
    let [burned, kept] = ground_halves(&mut r, &scene.borrow());
    eprintln!("[cut shadow] whole {whole:?}; half-dissolved: burned {burned:.1}, kept {kept:.1}");
    assert!(
        (whole[0] - whole[1]).abs() < 5.0,
        "control: whole, both halves shade"
    );
    assert!((kept - whole[1]).abs() < 5.0, "the kept half still shades");
    assert!(burned > kept + 20.0, "the burned half's shadow is gone");
}

#[test]
fn gpu_a_dissolved_crate_leaves_no_ambient_occlusion() {
    let black = image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 255]));
    let d = Dissolve::with_mask("black", &black);
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let yard = |with_crate: bool| {
        let mut scene = Scene::new();
        scene.skybox_path = String::new();
        scene.ambient_intensity = 0.6;
        let floor = (Vec3::new(0.0, -0.5, 0.0), Vec3::new(20.0, 1.0, 20.0));
        block(&mut scene, floor.0, floor.1, None);
        let volume = create_entity(&mut scene, "Volume", None);
        let mut vc = default_post_processing();
        (vc.bloom.active, vc.ssr.active, vc.ssao.active) = (false, false, true);
        scene.world.set_post_processing(volume, Some(vc));
        scene
            .world
            .set_camera(volume, Some(rusty_components::CameraComponent::default()));
        let crate_id =
            with_crate.then(|| block(&mut scene, Vec3::Y, Vec3::splat(2.0), Some(&d.shader)));
        (Rc::new(RefCell::new(scene)), crate_id)
    };
    let cam = Camera::new(Vec3::new(0.0, 2.5, 5.0), -90.0, -20.0);
    let (empty, _) = yard(false);
    let (scene, crate_id) = yard(true);
    let crate_id = crate_id.unwrap();
    let bare = frame(&mut r, &empty.borrow(), &cam);
    let differs = |a: &[u8]| {
        a.iter()
            .zip(&bare)
            .filter(|(x, y)| x.abs_diff(**y) > 6)
            .count()
    };

    dissolve(&scene, crate_id, &d, 0.0);
    let shown = differs(&frame(&mut r, &scene.borrow(), &cam));
    dissolve(&scene, crate_id, &d, 0.5);
    let left = differs(&frame(&mut r, &scene.borrow(), &cam));
    eprintln!(
        "[cut ssao] channels differing from the bare floor: standing {shown}, dissolved {left}"
    );
    assert!(shown > 60, "control: the crate shows");
    assert_eq!(left, 0, "the dissolved crate occludes nothing");
}
