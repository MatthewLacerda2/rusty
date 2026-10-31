//! A boot opens the first of its plan's scenes that loads (#1618).

use super::load_boot_scene;
use crate::core::project::BootPlan;
use crate::scene::Scene;
use crate::scripting::{ConsoleLogs, LogLevel};

fn errors(console: &ConsoleLogs) -> usize {
    let is_error = |(_, level): &&(String, LogLevel)| *level == LogLevel::Error;
    console.messages.iter().filter(is_error).count()
}

#[test]
fn a_scene_that_fails_to_load_falls_back_to_the_next() {
    let root = crate::test_temp::dir().join("load_boot_scene");
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(&root).unwrap();
    let broken = root.join("broken.scene").to_string_lossy().into_owned();
    let good = root.join("good.scene").to_string_lossy().into_owned();
    std::fs::write(&broken, "{ not a scene").unwrap();
    Scene::new().save_to_file(&good).unwrap();
    let plan = BootPlan {
        scenes: vec![broken, good.clone()],
        warnings: vec!["the startup scene is missing".into()],
    };
    let (mut scene, mut console) = (Scene::new(), ConsoleLogs::new());
    let loaded = load_boot_scene(&mut scene, &mut console, &plan, false);
    assert_eq!(loaded, Some(good));
    assert_eq!(errors(&console), 1, "the broken scene's error is logged");
    let warned = |(m, l): &(String, LogLevel)| *l == LogLevel::Warning && m.contains("missing");
    assert!(
        console.messages.iter().any(warned),
        "{:?}",
        console.messages
    );
}

#[test]
fn an_empty_plan_leaves_an_untitled_scene_and_says_nothing() {
    let (mut scene, mut console) = (Scene::new(), ConsoleLogs::new());
    let loaded = load_boot_scene(&mut scene, &mut console, &BootPlan::default(), true);
    assert_eq!(loaded, None);
    assert!(console.messages.is_empty());
}
