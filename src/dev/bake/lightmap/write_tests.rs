//! A bake's write (#1970's review): the old pages are removed only once the new
//! ones are written, so a write that fails leaves the scene on its old pages.

use std::path::Path;

use super::tests::{quick, scene, scene_path};
use super::*;

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
