//! A bake's write (#1970's review): the old pages are removed only once the new
//! ones are written, so a write that fails leaves the scene on its old pages.

use std::path::Path;

use super::encoder::Encoder;
use super::preview::{lock, Previews};
use super::preview_tests::preview_pages;
use super::tests::{quick, scene, scene_path};
use super::*;
use rusty_scene::lighting::lightmap::Lightmap;

#[test]
fn a_failed_write_leaves_the_old_pages_and_the_scene_on_them() {
    let (mut scene, _, _) = scene();
    let path = scene_path("write_fails");
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    let before = (*scene.lightmaps).clone();
    let other = BakeSettings { seed: 7, ..quick() };
    let maps = rusty_scene::lighting::lightmap::bake(&gather_bake_input(&scene), &other);
    let encoded = encode_pages(&maps, other.page(), &BakeProgress::default())
        .unwrap()
        .unwrap();
    // A folder where the new page's PNG goes (named by its KTX2's tag), so
    // writing it fails.
    let ktx2 = rusty_asset::lightmap_page::write(&encoded.colour[0].0);
    let name = format!("lightmap_0_{:08x}.png", content_tag(&ktx2));
    let blocker = lightmap_dir(&path).join(name);
    std::fs::create_dir_all(&blocker).unwrap();
    assert!(apply_encoded(&mut scene, &path, &encoded).is_err());
    assert_eq!(*scene.lightmaps, before, "the scene stays on its pages");
    for page in before.pages.iter().chain(&before.directions) {
        assert!(Path::new(page).exists(), "{page} was removed");
        let png = Path::new(page).with_extension("png");
        assert!(
            png.exists() || before.directions.contains(page),
            "{page}'s PNG"
        );
    }
    std::fs::remove_dir(&blocker).unwrap();
    apply_encoded(&mut scene, &path, &encoded).unwrap();
    assert!(
        before.pages.iter().all(|p| !Path::new(p).exists()),
        "now replaced"
    );
    assert!(scene.lightmaps.pages.iter().all(|p| Path::new(p).exists()));
}

/// Pass `k` of 9, as the bake hands it to the preview writer.
fn pass(k: u32) -> rusty_scene::lighting::lightmap::Pass {
    let (done, samples, view, passes, target) = (k, k, k, 9, 9);
    rusty_scene::lighting::lightmap::Pass {
        done,
        passes,
        samples,
        view,
        target,
    }
}

/// One 4² lightmap of `value`, as a pass hands it to the preview writer.
fn pass_maps(value: f32) -> Vec<Lightmap> {
    let texels = vec![glam::Vec3::splat(value); 16];
    let (directions, masks) = (Vec::new(), Vec::new());
    vec![Lightmap {
        entity: 1,
        size: 4,
        texels,
        directions,
        masks,
    }]
}

#[test]
fn the_preview_writer_keeps_only_the_shown_and_the_newest_pass_on_disk() {
    let path = scene_path("preview_writer");
    let progress = Arc::new(BakeProgress::default());
    let mut previews = Previews::new(&path);
    let taken = Default::default();
    let encoder = Encoder::start(&previews, 32, taken, progress.clone()).unwrap();
    let latest = previews.latest.clone();
    let put = |k: u32| {
        // Held, the writer's hand-over lock keeps the pass in flight for `busy`.
        let held = lock(&latest);
        encoder.put(pass(k), pass_maps(k as f32 * 0.1));
        assert!(
            encoder.busy(),
            "a pass waiting is work: the bake builds none"
        );
        drop(held);
        while encoder.busy() {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    };
    // Three passes no frame showed: each replaced one's page is removed.
    (1..=3).for_each(put);
    assert_eq!(preview_pages(&path), 1);
    let mut scene = Scene::new();
    assert_eq!(previews.show(&mut scene).map(|p| p.done), Some(3));
    put(4);
    assert_eq!(preview_pages(&path), 2, "the shown pass and the newest");
    progress.cancel();
    put(5);
    assert_eq!(
        preview_pages(&path),
        2,
        "a pass written after a cancel goes"
    );
    drop(encoder);
    previews.restore(&mut scene);
    assert_eq!(preview_pages(&path), 0);
}

#[test]
fn a_dropped_bakes_writer_removes_only_its_own_pages() {
    let path = scene_path("preview_two_writers");
    // Write one pass of `value` as `p`'s bake, and wait until it is handed over.
    let write = |p: &Previews, progress: &Arc<BakeProgress>, value| {
        let e = Encoder::start(p, 32, Default::default(), progress.clone()).unwrap();
        e.put(pass(1), pass_maps(value));
        while e.busy() {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    };
    let (old, new) = (Previews::new(&path), Previews::new(&path));
    let (gone, going) = (Arc::new(BakeProgress::default()), Arc::default());
    write(&new, &going, 0.5);
    assert_eq!(preview_pages(&path), 1, "the new bake's pass");
    gone.cancel();
    write(&old, &gone, 0.25);
    assert_eq!(
        preview_pages(&path),
        1,
        "the dropped bake's pass went alone"
    );
    new.clear();
}

#[test]
fn a_page_already_written_is_left_as_it_is() {
    let dir = lightmap_dir(&scene_path("write_unchanged"));
    std::fs::create_dir_all(&dir).unwrap();
    let png = b"same bytes";
    let page = write::write_png(&dir, "lightmap_mask", 0, png).unwrap();
    std::fs::write(&page, b"planted!!!").unwrap();
    let again = write::write_png(&dir, "lightmap_mask", 0, png).unwrap();
    assert_eq!(again, page);
    assert_eq!(
        std::fs::read(&page).unwrap(),
        b"planted!!!",
        "not rewritten"
    );
}
