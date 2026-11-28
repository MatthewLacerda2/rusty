use glam::Vec3;

use crate::postfx::effects::taa::gpu::flicker::measure::{luma, swing, tiles_of};
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::{CameraComponent, MaterialAsset, MaterialComponent, SsrAlgorithm};
use rusty_scene::create::{create_entity, default_post_processing, Primitive};
use rusty_scene::{Camera, Scene};

pub(super) const W: u32 = 320;
pub(super) const H: u32 = 240;
const WARM: u32 = 27;
const FRAMES: u32 = 36;

pub(super) fn eye() -> Camera {
    Camera::new(Vec3::new(-1.6, 1.6, 0.0), -65.0, 20.0)
}

pub(super) fn put(
    s: &mut Scene,
    name: &str,
    shape: Primitive,
    (at, size): (Vec3, Vec3),
    m: MaterialAsset,
) {
    let id = create_entity(s, name, Some(shape));
    let mut t = s.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (at, size);
    drop(t);
    s.materials.insert(name.into(), m);
    s.world
        .set_material(id, Some(MaterialComponent::new(name.to_string())));
}

pub(super) fn surface(grey: f32, metallic: f32, roughness: f32) -> MaterialAsset {
    MaterialAsset {
        base_color: [grey; 3],
        metallic,
        roughness,
        ..MaterialAsset::default()
    }
}

/// A bar's back shelf, seen from below and to one side: a satin metal panel (roughness 0.35)
/// behind two rows of bottles whose necks are 1 to 3 px wide at 320×240, lit by the ambient
/// light alone, TAA on at the default anti-flicker. The panel reflects the bottles beside each
/// one, and the necks' hits come and go with the jitter.
pub(super) fn shelf(ssr: bool, algorithm: SsrAlgorithm) -> Scene {
    let mut s = Scene::new();
    (s.skybox_path, s.ambient_intensity) = (String::new(), 0.3);
    let cam = s.add_entity("Camera".to_string());
    let camera = CameraComponent {
        taa_active: true,
        fxaa_active: false,
        ..Default::default()
    };
    s.world.set_camera(cam, Some(camera));
    let mut volume = default_post_processing();
    (volume.ssr.active, volume.ssr.min_smoothness) = (ssr, 0.3);
    volume.ssr.algorithm = algorithm;
    (volume.bloom.active, volume.ssao.active) = (false, false);
    s.world.set_post_processing(cam, Some(volume));
    let panel = (Vec3::new(0.0, 2.4, -3.0), Vec3::new(6.0, 2.0, 0.05));
    put(
        &mut s,
        "panel",
        Primitive::Box,
        panel,
        surface(0.8, 1.0, 0.35),
    );
    for (row, y) in [1.85, 2.55].into_iter().enumerate() {
        bottles(&mut s, row, y);
    }
    for y in [-0.05, 3.5] {
        let place = (Vec3::new(0.0, y, -2.0), Vec3::new(8.0, 0.1, 8.0));
        put(
            &mut s,
            "room",
            Primitive::Box,
            place,
            surface(0.4, 0.0, 1.0),
        );
    }
    s
}

/// One shelf board at height `y` and its 22 bottles, a third of them light.
fn bottles(s: &mut Scene, row: usize, y: f32) {
    let board = (Vec3::new(0.0, y - 0.02, -2.85), Vec3::new(6.0, 0.04, 0.3));
    put(s, "board", Primitive::Box, board, surface(0.3, 0.0, 0.8));
    for i in 0..22 {
        let x = -2.1 + i as f32 * 0.2 + row as f32 * 0.1;
        let light = (i + row).is_multiple_of(3);
        let (name, grey) = if light {
            ("light", 0.9)
        } else {
            ("dark", 0.08)
        };
        let glass = surface(grey, 0.0, 0.4);
        let body = (Vec3::new(x, y + 0.13, -2.82), Vec3::new(0.07, 0.26, 0.07));
        put(s, name, Primitive::Cylinder, body, glass.clone());
        let thin = 0.012 + 0.008 * (i % 3) as f32;
        let neck = (Vec3::new(x, y + 0.34, -2.82), Vec3::new(thin, 0.16, thin));
        put(s, name, Primitive::Cylinder, neck, glass);
    }
}

/// The largest 8×8 tile step between still frames: of the SSR-resolved image (before TAA)
/// and after TAA.
pub(super) fn steps(r: &mut Renderer, scene: &Scene) -> (f32, f32) {
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let (mut hdr, mut taa) = (Vec::new(), Vec::new());
    for f in 0..WARM + FRAMES {
        let target = v.color_target_view().unwrap();
        r.render(&mut v, scene, &eye(), &target, false);
        if f >= WARM {
            hdr.push(tiles_of(r, &v.post_fx.scene_hdr.texture, luma));
            taa.push(tiles_of(r, v.post_fx.taa_latest().expect("TAA ran"), luma));
        }
    }
    (swing(&hdr), swing(&taa))
}
