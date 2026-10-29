//! A list whose length changed is one override to Apply or Revert (#1527), as
//! Unity applies a whole array: its size entry and every entry under it go
//! together, whichever of them the verb is given.

use super::component_tests::bare;
use super::list_tests::{script, scripts};
use crate::scene::prefab::apply::{apply_instance_field_to_source, revert_instance_field};
use crate::scene::prefab::read_prefab_file;
use crate::scene::{list_instance_overrides, Scene};

/// The scripts' paths on `child`.
fn paths(scene: &Scene, child: u32) -> Vec<String> {
    scripts(scene, child).into_iter().map(|(p, _)| p).collect()
}

/// The source's scripts on its child, as paths and speeds.
fn source_scripts(tag: &str) -> Vec<(String, bool)> {
    let path = crate::test_temp::dir().join(format!("rusty_1527_{tag}.prefab"));
    let prefab = read_prefab_file(&path.to_string_lossy()).unwrap();
    let list = &prefab.entities[1].scripts;
    list.iter()
        .map(|s| (s.path.clone(), s.values.contains_key("speed")))
        .collect()
}

/// A linked `Root { Child [a.lua] }` whose child grew a `b.lua`.
fn grown(tag: &str) -> (Scene, u32, u32) {
    let (mut scene, root, child) = bare(tag, |s, c| {
        s.world.scripts_mut(c).unwrap().push(script("a.lua", None));
    });
    let mut list = scene.world.scripts_mut(child).unwrap();
    list.push(script("b.lua", None));
    drop(list);
    (scene, root, child)
}

/// A linked `Root { Child [a.lua (speed), b.lua] }` whose child removed `a.lua`.
fn shrunk(tag: &str) -> (Scene, u32, u32) {
    let (mut scene, root, child) = bare(tag, |s, c| {
        let mut list = s.world.scripts_mut(c).unwrap();
        list.extend([script("a.lua", Some(1.0)), script("b.lua", None)]);
    });
    scene.world.scripts_mut(child).unwrap().remove(0);
    (scene, root, child)
}

#[test]
fn applying_either_key_of_a_grown_list_writes_the_whole_list() {
    for (n, key) in ["/scripts/#size", "/scripts/1"].into_iter().enumerate() {
        let tag = format!("grown_apply_{n}");
        let (mut scene, root, child) = grown(&tag);
        apply_instance_field_to_source(&mut scene, child, key).unwrap();
        let both = [("a.lua".to_string(), false), ("b.lua".to_string(), false)];
        assert_eq!(source_scripts(&tag), both, "{key}");
        assert_eq!(paths(&scene, child), ["a.lua", "b.lua"], "{key}");
        assert!(list_instance_overrides(&scene, root).unwrap().is_empty());
    }
}

#[test]
fn reverting_either_key_of_a_grown_list_reverts_the_whole_list() {
    for (n, key) in ["/scripts/#size", "/scripts/1"].into_iter().enumerate() {
        let (mut scene, root, child) = grown(&format!("grown_revert_{n}"));
        assert_eq!(revert_instance_field(&mut scene, child, key), Ok(true));
        assert_eq!(paths(&scene, child), ["a.lua"], "{key}");
        assert!(list_instance_overrides(&scene, root).unwrap().is_empty());
    }
}

#[test]
fn applying_either_key_of_a_shrunk_list_leaves_the_source_the_second_script() {
    let keys = ["/scripts/#size", "/scripts/0/path"];
    for (n, key) in keys.into_iter().enumerate() {
        let tag = format!("shrunk_apply_{n}");
        let (mut scene, root, child) = shrunk(&tag);
        let listed = list_instance_overrides(&scene, root).unwrap();
        assert!(listed.contains(&format!("1:{key}")), "{listed:?}");
        apply_instance_field_to_source(&mut scene, child, key).unwrap();
        assert_eq!(
            source_scripts(&tag),
            [("b.lua".to_string(), false)],
            "{key}"
        );
        assert_eq!(paths(&scene, child), ["b.lua"], "{key}");
        assert!(list_instance_overrides(&scene, root).unwrap().is_empty());
    }
}

#[test]
fn reverting_either_key_of_a_shrunk_list_brings_both_scripts_back() {
    let keys = ["/scripts/#size", "/scripts/0/path"];
    for (n, key) in keys.into_iter().enumerate() {
        let (mut scene, root, child) = shrunk(&format!("shrunk_revert_{n}"));
        assert_eq!(revert_instance_field(&mut scene, child, key), Ok(true));
        assert_eq!(paths(&scene, child), ["a.lua", "b.lua"], "{key}");
        assert!(list_instance_overrides(&scene, root).unwrap().is_empty());
    }
}
