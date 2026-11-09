//! A bake's write (#1970's review): the old pages are removed only once the new
//! ones are written, so a write that fails leaves the scene on its old pages.

use std::path::Path;

use super::tests::{quick, scene, scene_path};
use super::*;
use crate::scene::lighting::lightmap::Lightmap;

#[test]
fn a_failed_write_leaves_the_old_pages_and_the_scene_on_them() {
    let (mut scene, _, _) = scene();
    let path = scene_path("write_fails");
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    let before = (*scene.lightmaps).clone();
    let other = BakeSettings { seed: 7, ..quick() };
    let maps = crate::scene::lighting::lightmap::bake(&gather_bake_input(&scene), &other);
    let encoded = encode_pages(&maps, other.page(), &BakeProgress::default())
        .unwrap()
        .unwrap();
    // A folder where the new page's PNG goes (named by its KTX2's tag), so
    // writing it fails.
    let ktx2 = crate::asset::lightmap_page::write(&encoded.colour[0].0);
    let name = format!("lightmap_0_{:08x}.png", content_tag(&ktx2));
    let blocker = lightmap_dir(&path).join(name);
    std::fs::create_dir_all(&blocker).unwrap();
    assert!(apply_encoded(&mut scene, &path, &encoded).is_err());
    assert_eq!(*scene.lightmaps, before, "the scene stays on its pages");
    let on_disk = before.pages.iter().chain(&before.directions);
    for page in on_disk {
        assert!(Path::new(page).exists(), "{page} was removed");
        assert!(Path::new(page).with_extension("png").exists());
    }
    std::fs::remove_dir(&blocker).unwrap();
    apply_encoded(&mut scene, &path, &encoded).unwrap();
    assert!(
        before.pages.iter().all(|p| !Path::new(p).exists()),
        "now replaced"
    );
    assert!(scene.lightmaps.pages.iter().all(|p| Path::new(p).exists()));
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
    use super::encoder::Encoder;
    use super::preview::Previews;
    use super::preview_tests::preview_pages;
    use crate::scene::lighting::lightmap::Pass;
    let path = scene_path("preview_writer");
    let progress = Arc::new(BakeProgress::default());
    let mut previews = Previews::new(&path);
    let taken = Default::default();
    let encoder = Encoder::start(&previews, 32, taken, progress.clone()).unwrap();
    let put = |k: u32| {
        let pass = Pass {
            done: k,
            passes: 9,
            samples: k,
            view: k,
            target: 9,
        };
        encoder.put(pass, pass_maps(k as f32 * 0.1));
        assert!(
            encoder.busy(),
            "a pass waiting is work: the bake builds none"
        );
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
