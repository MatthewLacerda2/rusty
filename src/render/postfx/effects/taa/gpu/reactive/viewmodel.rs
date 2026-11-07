//! The reactive mask under a camera stack (#1845): a world window behind a
//! viewmodel's gun makes the window reactive and leaves the gun's pixels as they
//! were, since the resolve counts only translucents drawn by the pixel's own camera
//! or one above it, and a viewmodel's muzzle flash over the gun, whose clear
//! corners lie on the gun too, changes none of that.

use glam::{Quat, Vec3};

use super::super::fixture::{cube, diff, eye, frame, renderer, taa_scene, view};
use super::super::wall::noise;
use super::coverage::flash;
use crate::components::{
    CameraComponent, MaterialAsset, MaterialComponent, RenderMode, TaaReactive,
};
use crate::render::motion::gpu_fixture::{H, W};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{ClearFlags, Scene};

/// The viewmodel's layer: only the overlay camera draws it, so the window lies
/// under the gun in the world camera's image.
const VIEWMODEL: u8 = 7;

/// The world camera running TAA, a tinted window 4.5 m out of `reactive`, a cube
/// behind it, and a `DepthOnly` overlay holding a noise-textured gun cube 3 m
/// ahead.
fn stack(reactive: TaaReactive) -> (Scene, u32) {
    let (mut scene, world) = taa_scene(true, false);
    scene.world.camera_mut(world).unwrap().culling_mask = !(1 << VIEWMODEL);
    cube(&mut scene, Vec3::new(1.0, 0.3, -7.0));
    let window = MaterialAsset {
        render_mode: RenderMode::Transparent,
        base_color: [0.9, 0.3, 0.2],
        alpha: 0.5,
        reactive,
        ..MaterialAsset::default()
    };
    scene.materials.insert("window".to_string(), window);
    let pane = create_entity(&mut scene, "window", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(pane).unwrap();
    (t.position, t.scale) = (Vec3::new(0.0, 0.0, -4.5), Vec3::new(6.0, 4.0, 0.05));
    drop(t);
    let m = MaterialComponent::new("window".to_string());
    scene.world.set_material(pane, Some(m));
    let overlay = scene.add_entity("ViewmodelCamera".to_string());
    let lens = CameraComponent {
        render_order: 1,
        clear_flags: ClearFlags::DepthOnly,
        culling_mask: 1 << VIEWMODEL,
        ..Default::default()
    };
    scene.world.set_camera(overlay, Some(lens));
    // Bright noise on the gun, so the weight shows in the jitter's sub-pixel shift.
    let lit = noise([1.0; 3], [0.25, 0.25]);
    let gun_look = MaterialAsset {
        emissive: [1.0; 3],
        emissive_map: lit.base_color_map.clone(),
        ..lit
    };
    scene.materials.insert("gun".to_string(), gun_look);
    let gun = cube(&mut scene, Vec3::new(0.0, 0.0, -3.0));
    let m = MaterialComponent::new("gun".to_string());
    scene.world.set_material(gun, Some(m));
    scene.set_parent(gun, Some(overlay)).unwrap();
    scene.world.set_layer(gun, VIEWMODEL);
    (scene, overlay)
}

/// Eight frames of the stack turning 3° a frame, the gun's glow flickering, so
/// its history and its frame differ where the weight matters; the last. With
/// `muzzle`, a viewmodel sprite 2 m ahead, off the gun's centre, its clear corners
/// over the gun.
fn turned(reactive: TaaReactive, muzzle: bool) -> Option<Vec<u8>> {
    let mut r = renderer()?;
    let (mut scene, overlay) = stack(reactive);
    let sprite = muzzle.then(|| flash(&mut scene, Vec3::ZERO, 1.6, TaaReactive::Auto));
    if let Some(id) = sprite {
        scene.world.set_layer(id, VIEWMODEL);
    }
    let mut v = view(&r);
    let mut last = Vec::new();
    for step in 0..8 {
        let yaw = step as f32 * 3.0;
        let turn = Quat::from_rotation_y(-yaw.to_radians());
        scene.world.transform_mut(overlay).unwrap().rotation = turn;
        if let Some(id) = sprite {
            let at = turn * Vec3::new(0.3, 0.3, -2.0);
            scene.world.particles_mut(id).unwrap().runtime.particles[0].position = at;
        }
        let glow = if step % 2 == 0 { 0.4 } else { 1.6 };
        scene.materials.get_mut("gun").unwrap().emissive = [glow; 3];
        last = frame(&mut r, &mut v, &scene, &eye(yaw));
    }
    Some(last)
}

/// The gun's pixels.
fn gun() -> impl Iterator<Item = (u32, u32)> {
    (29..=35).flat_map(|x| (21..=27).map(move |y| (x, y)))
}

#[test]
fn gpu_a_window_behind_a_viewmodel_leaves_the_gun_alone() {
    let (Some(full), Some(off)) = (
        turned(TaaReactive::Full, false),
        turned(TaaReactive::Off, false),
    ) else {
        return;
    };
    let worst = gun().map(|p| diff(&full, &off, p)).max().unwrap();
    let all = (0..W).flat_map(|x| (0..H).map(move |y| (x, y)));
    let window = all.map(|p| diff(&full, &off, p)).max().unwrap();
    eprintln!("[taa 1845] gun: {worst} between Full and Off; window: {window}");
    assert_eq!(worst, 0, "the window's reactivity reached the gun");
    assert!(
        window > 4,
        "the window's reactivity changed nothing: {window}"
    );
}

#[test]
fn gpu_a_muzzle_flash_over_the_gun_keeps_the_window_off_it() {
    let (Some(full), Some(off)) = (
        turned(TaaReactive::Full, true),
        turned(TaaReactive::Off, true),
    ) else {
        return;
    };
    let worst = gun().map(|p| diff(&full, &off, p)).max().unwrap();
    eprintln!("[taa 1845] gun under a muzzle flash: {worst} between Full and Off");
    assert_eq!(
        worst, 0,
        "the window's reactivity reached the gun under the flash"
    );
}
