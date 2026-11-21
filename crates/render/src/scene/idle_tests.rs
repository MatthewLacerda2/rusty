use glam::Vec3;

use super::fixture::{frame, fresh, held, yard, RES};
use crate::RenderCounters;
use rusty_components::RenderMode;

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
    let red = scene.materials["red"].clone();
    scene.materials.insert("red".into(), red);
    assert_eq!(extracted(&frame(&mut r, &scene).1), (0, 0), "same value");
    scene.materials.get_mut("red").unwrap().base_color[1] = 0.5;
    assert_eq!(extracted(&frame(&mut r, &scene).1), (0, 0), "one record");
    assert_eq!(held(&r, &scene), fresh(&mut r, &scene));
    scene.materials.get_mut("red").unwrap().render_mode = RenderMode::Cutout;
    assert_eq!(extracted(&frame(&mut r, &scene).1), (1, 0), "one material");
    assert_eq!(held(&r, &scene), fresh(&mut r, &scene));
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
fn gpu_an_undo_and_a_load_rebuild_what_a_fresh_build_holds() {
    let Some((mut r, pair)) = crate::test_gpu::headless_pair_or_skip(RES, RES) else {
        return;
    };
    let mut scene = yard(6);
    frame(&mut r, &scene);
    let id = scene.world.ids_with_mesh()[2];
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 2.0, -3.0);
    scene.world.rewind_changes();
    let (_, undone) = frame(&mut r, &scene);
    assert_eq!(extracted(&undone), (6, 0), "an Undo rebuilds");
    assert_eq!(held(&r, &scene), fresh(&mut r, &scene));
    let mut loaded = yard(6);
    loaded.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 2.0, -3.0);
    let (after_load, _) = frame(&mut r, &loaded);
    let mut other = pair.renderer(RES, RES);
    assert_eq!(
        after_load,
        frame(&mut other, &loaded).0,
        "a load draws its own World"
    );
}

#[test]
fn gpu_a_preview_tab_beside_a_moving_scene_keeps_both() {
    let Some((mut r, pair)) = crate::test_gpu::headless_pair_or_skip(RES, RES) else {
        return;
    };
    let (mut main, preview) = (yard(6), yard(2));
    let mover = main.world.ids_with_mesh()[0];
    for step in 0..5 {
        main.world.transform_mut(mover).unwrap().position.y = step as f32 * 0.2;
        let (_, m) = frame(&mut r, &main);
        let (_, p) = r.aside(|r| frame(r, &preview));
        if step > 1 {
            assert_eq!(extracted(&m), (0, 1), "the main scene re-places its mover");
            assert_eq!(extracted(&p), (0, 0), "the preview holds still");
        }
    }
    assert_eq!(held(&r, &main), fresh(&mut r, &main));
    let mut other = pair.renderer(RES, RES);
    assert_eq!(frame(&mut r, &main).0, frame(&mut other, &main).0);
}
