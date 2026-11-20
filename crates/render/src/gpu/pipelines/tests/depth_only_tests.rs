//! The depth prepass draws a batch nothing clips with no fragment stage and no
//! material group (#2347), and the depth it leaves is the clipping pipeline's,
//! byte for byte. A plain textured opaque box (its albedo clear on one half, which
//! an opaque never clips), a Cutout box and a dissolving box: only the plain one
//! is depth-only, and its depth is unchanged when it is drawn through the clipping
//! pipeline instead (Cutout with a cutoff of 0, which clips nothing). The
//! recorder's probe (`draw::record::probe`) shows the path each draw took: which
//! pipeline it was recorded with, by identity, and whether it bound a material.
//! Skips with no adapter.

use glam::Vec3;

use crate::draw::record::probe::{self, PrepassDraws};
use crate::gpu::pipelines::surface::cut_depth_tests::dissolve;
use crate::gpu::pipelines::surface::mask_tests::Dissolve;
use crate::readback::read_texture_bytes;
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_asset::MapRef;
use rusty_components::{CameraComponent, RenderMode};
use rusty_scene::create::{create_entity, default_post_processing, Primitive};
use rusty_scene::{Camera, Scene};

const RES: u32 = 64;

/// A box at `x` on a floor, its material's key.
fn boxed(scene: &mut Scene, x: f32) -> (u32, String) {
    let id = create_entity(scene, "Box", Some(Primitive::Box));
    scene.world.transform_mut(id).unwrap().position = Vec3::new(x, 0.5, 0.0);
    (id, scene.ensure_material_key(id).unwrap())
}

/// The floor and the three boxes under an AO volume (so the prepass runs): the
/// plain one textured with `albedo`, the Cutout one clipping at 0.5, the third
/// drawn by `cut`'s shader. The scene, the plain box's key, and the floor's and
/// the boxes' entities, in that order.
fn yard(albedo: &str, cut: &str) -> (Scene, String, [u32; 4]) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let floor = create_entity(&mut scene, "Floor", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(floor).unwrap();
    (t.position, t.scale) = (Vec3::new(0.0, -0.5, 0.0), Vec3::new(20.0, 1.0, 20.0));
    drop(t);
    let (plain_id, plain) = boxed(&mut scene, -2.0);
    let (cutout_id, cutout) = boxed(&mut scene, 0.0);
    let (cutting, key) = boxed(&mut scene, 2.0);
    for (key, mode) in [(&plain, RenderMode::Opaque), (&cutout, RenderMode::Cutout)] {
        let m = scene.materials.get_mut(key).unwrap();
        (m.render_mode, m.alpha_cutoff) = (mode, 0.5);
        m.base_color_map = Some(MapRef::from(albedo.to_owned()));
    }
    scene.materials.get_mut(&key).unwrap().shader = Some(cut.to_owned());
    let volume = create_entity(&mut scene, "Volume", None);
    let mut vc = default_post_processing();
    vc.ssao.active = true;
    scene.world.set_post_processing(volume, Some(vc));
    scene
        .world
        .set_camera(volume, Some(CameraComponent::default()));
    (scene, plain, [floor, plain_id, cutout_id, cutting])
}

/// Whether each of `ids`' one slot is depth-only, after a frame.
fn classes(r: &Renderer, scene: &Scene, ids: [u32; 4]) -> [bool; 4] {
    let kept = r.retained.scene(scene.id()).expect("extracted");
    let of = |id| kept.objects().find(|(e, _)| *e == id).map(|(_, o)| o);
    ids.map(|id| {
        let object = of(id).expect("kept");
        assert_eq!(object.slots.len(), 1, "a box draws one slot");
        object.slots[0].key.depth_only
    })
}

/// One frame from above the boxes; the view's depth after it, and what its
/// prepass recorded.
fn depth(r: &mut Renderer, scene: &Scene) -> (Vec<u8>, PrepassDraws) {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 4.0, 5.0), -90.0, -40.0);
    probe::take();
    r.render(&mut view, scene, &cam, &out, false);
    let drawn = probe::take();
    let bytes = read_texture_bytes(&r.device, &r.queue, &view.depth_texture, (RES, RES), 4);
    (bytes, drawn)
}

/// A file removed when dropped, a panic's unwinding included.
struct Removed(std::path::PathBuf);

impl Drop for Removed {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn gpu_only_an_unclipped_batch_draws_the_prepass_depth_only_and_its_depth_holds() {
    // Clear where u < 0.5: the Cutout box shows its hole there, the plain one none.
    let albedo = image::RgbaImage::from_fn(4, 1, |x, _| {
        image::Rgba([255, 255, 255, 255 * (x / 2) as u8])
    });
    let mask = image::RgbaImage::from_fn(4, 1, |x, _| image::Rgba([255 * (x / 2) as u8; 4]));
    let d = Dissolve::with_mask("depth_only", &mask);
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let path = rusty_test_support::temp::dir()
        .join(format!("rusty_depth_only_{}.png", std::process::id()));
    albedo.save(&path).expect("albedo written");
    let path = Removed(path);
    let (scene, plain, ids) = yard(&path.0.to_string_lossy(), &d.shader);
    let cutting = ids[3];
    let scene = std::rc::Rc::new(std::cell::RefCell::new(scene));
    dissolve(&scene, cutting, &d, 0.5);
    let (today, drawn) = depth(&mut r, &scene.borrow());
    // One prepass of four batches: the floor and the plain box through the
    // fragment-less pipeline with no material, the other two through the clipping.
    let path_of = |depth_only, material_binds| PrepassDraws {
        draws: 4,
        depth_only,
        material_binds,
    };
    assert_eq!(drawn, path_of(2, 2), "the depth-only path taken");
    // The floor and the plain box; neither the Cutout box nor the dissolving one.
    let classed = classes(&r, &scene.borrow(), ids);
    assert_eq!(classed, [true, true, false, false], "by box");

    // The plain box through the clipping pipeline: Cutout, clipping nothing.
    let mut s = scene.borrow_mut();
    let m = s.materials.get_mut(&plain).unwrap();
    (m.render_mode, m.alpha_cutoff) = (RenderMode::Cutout, 0.0);
    drop(s);
    let (clipped, drawn) = depth(&mut r, &scene.borrow());
    assert_eq!(
        drawn,
        path_of(1, 3),
        "the plain box now draws the clipping path"
    );
    let classed = classes(&r, &scene.borrow(), ids);
    assert_eq!(classed, [true, false, false, false], "the plain box clips");
    let drawn = today.chunks(4).filter(|d| *d != 1f32.to_le_bytes()).count();
    assert!(
        drawn > (RES * RES / 2) as usize,
        "control: the yard fills the view"
    );
    assert!(today == clipped, "the depth-only pipeline moved the depth");
}
