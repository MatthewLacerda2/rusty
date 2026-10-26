//! Post-FX reads each view's own camera (#1434), Unity URP's camera-stack rule: the
//! window uses its screen stack's base camera, a render texture its own camera, and
//! neither depends on which camera entity was created first.

use crate::temp::dir;
use glam::Vec3;
use rusty::components::{CameraComponent, PostProcessingComponent, RenderTarget};
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::{capture, capture_into};
use rusty::scene::authoring::default_post_processing;
use rusty::scene::{Camera, MeshComponent, Scene};

use super::super::render_texture_scene::{pip_scene, screen_camera, SIZE};

fn graded(saturation: f32) -> PostProcessingComponent {
    let mut vc = default_post_processing();
    vc.color_adjustments.saturation = saturation;
    vc
}

/// The PiP scene with the screen's own camera carrying a grey grade and the
/// render-texture camera a full-colour one; `main_first` creates the screen's camera
/// before the render-texture camera rather than after.
fn grey_window_colour_pip(main_first: bool) -> Scene {
    let mut scene = pip_scene(None);
    let mut add = |name: &str, target: Option<RenderTarget>, saturation: f32| {
        let id = scene.add_entity(name.to_string());
        let comp = CameraComponent {
            fov: 60.0,
            target_texture: target,
            ..CameraComponent::default()
        };
        scene.world.set_camera(id, Some(comp));
        scene
            .world
            .set_post_processing(id, Some(graded(saturation)));
        // Identity rotation: looks down -Z, at the box (the screen's pose is the base).
        scene.world.transform_mut(id).expect("transform").position = Vec3::new(0.0, 0.0, 6.0);
    };
    let pip = || Some(RenderTarget::new("pip", 64, 128));
    if main_first {
        add("Main", None, 0.0);
        add("PipCamera", pip(), 1.0);
    } else {
        add("PipCamera", pip(), 1.0);
        add("Main", None, 0.0);
    }
    scene
}

#[test]
fn a_render_texture_keeps_its_own_grade_and_the_window_its_own() {
    for main_first in [true, false] {
        let path = dir().join(format!("rusty_postfx_per_camera_{main_first}.png"));
        let scene = grey_window_colour_pip(main_first);
        let shot = capture_into(
            &mut CaptureHost::new(),
            &scene,
            &screen_camera(),
            &path,
            SIZE,
            SIZE,
        );
        if !shot.expect("capture must not error") {
            eprintln!("[postfx per camera] no GPU/software adapter — skipping");
            return;
        }
        let img = image::open(&path).expect("png").to_rgb8();
        // The UI Image draws after post-FX, so the PiP shows the texture's own grade.
        let [r, g, b] = img.get_pixel(32, 64).0;
        assert!(
            r > g.saturating_add(40) && r > b.saturating_add(40),
            "PiP in colour: {:?}",
            [r, g, b]
        );
        // The window's right half is the screen camera's sky, graded to grey.
        let [r, g, b] = img.get_pixel(96, 64).0;
        let spread = r.max(g).max(b) - r.min(g).min(b);
        assert!(
            spread <= 2,
            "window grey (main_first={main_first}): {:?}",
            [r, g, b]
        );
    }
}

/// A box seen off-axis by the screen camera, whose FXAA is `main_fxaa`; `decoy` adds
/// an earlier-created render-texture camera with FXAA on, which nothing shows.
fn box_scene(main_fxaa: bool, decoy: bool) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 1.0;
    if decoy {
        let id = scene.add_entity("Decoy".to_string());
        let comp = CameraComponent {
            target_texture: Some(RenderTarget::new("unused", 16, 16)),
            ..CameraComponent::default()
        };
        scene.world.set_camera(id, Some(comp));
    }
    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = rusty::components::mesh::primitives::generate_box(2.0, 2.0, 2.0);
    let mesh = MeshComponent::new("Box", vertices, indices);
    scene.world.set_mesh(id, Some(mesh));
    let comp = CameraComponent {
        fxaa_active: main_fxaa,
        ..CameraComponent::default()
    };
    scene.world.set_camera(id, Some(comp));
    scene
}

#[test]
fn the_window_takes_fxaa_from_its_own_camera_not_the_first_one() {
    let cam = Camera::new(Vec3::new(2.5, 1.5, 4.0), -125.0, -18.0);
    let shot = |name: &str, scene: Scene| {
        let path = dir().join(format!("rusty_postfx_fxaa_{name}.png"));
        capture(&scene, &cam, &path, 128, 128)
            .expect("capture")
            .then(|| std::fs::read(path).unwrap())
    };
    let Some(off) = shot("off", box_scene(false, false)) else {
        eprintln!("[postfx per camera] no GPU/software adapter — skipping");
        return;
    };
    let with_decoy = shot("decoy", box_scene(false, true)).expect("adapter");
    let on = shot("on", box_scene(true, false)).expect("adapter");
    assert_ne!(off, on, "FXAA must reach the image at all");
    assert_eq!(
        off, with_decoy,
        "an earlier camera's FXAA must not reach the window"
    );
}
