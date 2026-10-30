//! A cube whose header passed but whose load fails (#1609): refused with one warning
//! until it is re-written, and kept out of the frame it failed in.

use std::cell::RefCell;

use glam::Vec3;

use super::super::gpu_tests::{cube_file, probes_at, renderer};
use crate::scripting::ConsoleLogs;

#[test]
fn gpu_a_cube_that_fails_to_load_is_left_out_until_rewritten() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let (broken, fine) = (
        cube_file("broken_8.ktx2", 8, 5),
        cube_file("fine_4.ktx2", 4, 5),
    );
    let scene = probes_at(&[(-2.0, &broken), (2.0, &fine)]);
    // Its header is read while whole; the file is cut short before the load, by a
    // write the freshness watch does not see.
    assert!(renderer.reflection_probes.shape(&broken).is_some());
    let bytes = std::fs::read(&broken).unwrap();
    std::fs::write(&broken, &bytes[..bytes.len() - 1]).unwrap();
    crate::logging::init();
    let console = RefCell::new(ConsoleLogs::new());
    let frame = |renderer: &mut crate::render::Renderer| {
        renderer.update_reflection_probes(&scene, Vec3::ZERO);
        let logged = crate::logging::forward(&console);
        logged
            .iter()
            .filter(|m| m.contains(broken.as_str()))
            .count()
    };
    assert_eq!(frame(&mut renderer), 1, "the failed load warns once");
    let probes = &renderer.reflection_probes;
    assert_eq!(probes.probe_for(Vec3::new(-2.0, 0.0, 0.0)), 0, "left out");
    assert_ne!(probes.probe_for(Vec3::new(2.0, 0.0, 0.0)), 0, "kept");
    assert_eq!(
        probes.face(),
        4,
        "the array is not sized for the refused cube"
    );
    let loads = probes.loads;
    assert_eq!(frame(&mut renderer), 0, "the next frame does not warn");
    assert_eq!(renderer.reflection_probes.loads, loads, "nor read it again");
    std::fs::write(&broken, &bytes).unwrap();
    renderer.invalidate_textures(std::slice::from_ref(&broken));
    assert_eq!(frame(&mut renderer), 0);
    let probes = &renderer.reflection_probes;
    assert_ne!(probes.probe_for(Vec3::new(-2.0, 0.0, 0.0)), 0, "re-written");
}
