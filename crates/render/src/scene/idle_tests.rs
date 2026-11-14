//! crates/render/src/scene/idle_tests.rs — what an extract costs (#1706): a frame that
//! changed nothing walks no object and no mesh column, shadows included (probe
//! field, library and lightmaps included), a colour edit no object and a key edit
//! that material's users alone, a
//! move re-places only what moved, and a load, an Undo or a Play restore rebuilds,
//! then holds what a fresh build holds.

use glam::Vec3;

use super::fixture::{frame, fresh, held, yard, RES};
use crate::RenderCounters;
use rusty_components::RenderMode;
use rusty_scene::snapshot::SceneSnapshot;

fn extracted(c: &RenderCounters) -> (u32, u32) {
    (c.render_extracted, c.render_posed)
}

#[test]
fn gpu_a_frame_with_nothing_changed_walks_no_object() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = yard(6);
    scene.probes.add_probe(Vec3::new(0.0, 1.0, -4.0));
    let (first, at_first) = frame(&mut r, &scene);
    assert_eq!(
        extracted(&at_first),
        (6, 0),
        "the first frame builds every object"
    );
    let walks = rusty_ecs::world::walks::of::<rusty_components::MeshComponent>;
    let before = walks();
    let (again, idle) = frame(&mut r, &scene);
    assert_eq!(extracted(&idle), (0, 0), "nothing changed: {idle:?}");
    assert_eq!(
        walks(),
        before,
        "the frame, its shadow sweeps included, walks no mesh"
    );
    assert_eq!(idle.visible_entities, at_first.visible_entities);
    assert_eq!(first, again, "the retained frame draws the same pixels");
    // A write of what the library already holds re-derives nothing.
    let red = scene.materials["red"].clone();
    scene.materials.insert("red".into(), red);
    assert_eq!(extracted(&frame(&mut r, &scene).1), (0, 0), "same value");
    // A colour edit rewrites the material's record alone (#1706): no object.
    scene.materials.get_mut("red").unwrap().base_color[1] = 0.5;
    assert_eq!(extracted(&frame(&mut r, &scene).1), (0, 0), "one record");
    assert_eq!(held(&r, &scene), fresh(&mut r, &scene));
    // A key edit re-derives that material's users alone: one red prop of six.
    scene.materials.get_mut("red").unwrap().render_mode = RenderMode::Cutout;
    assert_eq!(extracted(&frame(&mut r, &scene).1), (1, 0), "one material");
    assert_eq!(held(&r, &scene), fresh(&mut r, &scene));
    // A re-baked probe re-samples every object.
    scene.probes.probes_mut()[0].sh.coeffs[0] = [0.5; 3];
    assert_eq!(extracted(&frame(&mut r, &scene).1), (6, 0), "a probe bake");
    assert_eq!(held(&r, &scene), fresh(&mut r, &scene));
}

#[test]
fn gpu_a_move_re_places_the_moved_subtree_alone() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = yard(6);
    let ids = scene.world.ids_with_mesh();
    let (parent, child) = (ids[0], ids[1]);
    scene.set_parent(child, Some(parent)).unwrap();
    frame(&mut r, &scene);
    scene.world.transform_mut(parent).unwrap().position.y += 0.5;
    let (_, moved) = frame(&mut r, &scene);
    assert_eq!(
        extracted(&moved),
        (0, 2),
        "the parent and its child: {moved:?}"
    );
    assert_eq!(held(&r, &scene), fresh(&mut r, &scene));
}

#[test]
fn gpu_an_undo_a_load_and_a_play_restore_rebuild_what_a_fresh_build_holds() {
    let Some((mut r, pair)) = crate::test_gpu::headless_pair_or_skip(RES, RES) else {
        return;
    };
    let mut scene = yard(6);
    frame(&mut r, &scene);
    // An Undo step rewinds every cursor.
    let id = scene.world.ids_with_mesh()[2];
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 2.0, -3.0);
    scene.world.rewind_changes();
    let (_, undone) = frame(&mut r, &scene);
    assert_eq!(extracted(&undone), (6, 0), "an Undo rebuilds");
    assert_eq!(held(&r, &scene), fresh(&mut r, &scene));
    // Play, then Stop: the snapshot replaces the World.
    let snapshot = SceneSnapshot::capture(&scene);
    let gone = scene.world.ids_with_mesh()[0];
    scene.world.despawn(gone);
    frame(&mut r, &scene);
    snapshot.restore(&mut scene);
    let (restored, stopped) = frame(&mut r, &scene);
    assert_eq!(stopped.render_extracted, 6, "Stop rebuilds: {stopped:?}");
    assert_eq!(held(&r, &scene), fresh(&mut r, &scene));
    // A load: a new World under a new identity draws what a new renderer draws.
    let mut loaded = yard(6);
    loaded.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 2.0, -3.0);
    let (after_load, _) = frame(&mut r, &loaded);
    let mut other = pair.renderer(RES, RES);
    assert_eq!(
        after_load,
        frame(&mut other, &loaded).0,
        "a load draws its own World"
    );
    assert_eq!(
        restored,
        frame(&mut other, &scene).0,
        "the restore draws the edit World"
    );
}

#[test]
fn gpu_a_preview_tab_beside_a_moving_scene_keeps_both() {
    let Some((mut r, pair)) = crate::test_gpu::headless_pair_or_skip(RES, RES) else {
        return;
    };
    let (mut main, preview) = (yard(6), yard(2));
    let mover = main.world.ids_with_mesh()[0];
    // The preview's primitives are newer edits of the shared Box and Sphere, so its
    // first frame re-uploads them and the main scene's second re-reads its sharers.
    for step in 0..5 {
        main.world.transform_mut(mover).unwrap().position.y = step as f32 * 0.2;
        let (_, m) = frame(&mut r, &main);
        let (_, p) = frame(&mut r, &preview);
        if step > 1 {
            assert_eq!(extracted(&m), (0, 1), "the main scene re-places its mover");
            assert_eq!(extracted(&p), (0, 0), "the preview holds still");
        }
    }
    assert_eq!(held(&r, &main), fresh(&mut r, &main));
    let mut other = pair.renderer(RES, RES);
    assert_eq!(frame(&mut r, &main).0, frame(&mut other, &main).0);
}
