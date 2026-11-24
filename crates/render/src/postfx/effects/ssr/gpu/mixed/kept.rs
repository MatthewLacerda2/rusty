//! What *Mixed* built outlives a render of another scene that does not trace (the editor's
//! Inspector preview, every frame), and is freed when its own scene stops tracing.

use super::super::fixture::MIRROR;
use super::{room, shot};
use rusty_components::SsrTracing;

#[test]
fn gpu_mixed_keeps_its_base_colours_across_another_scene_and_frees_them_when_turned_off() {
    let Some(mut r) = super::super::fixture::renderer() else {
        return;
    };
    let mut traced = room(MIRROR, true, |_| {});
    let preview = room(MIRROR, true, |v| v.ssr.tracing = SsrTracing::RayMarching);
    for _ in 0..2 {
        shot(&mut r, &traced);
        // As the editor renders the Inspector's preview.
        r.aside(|r| shot(r, &preview));
        assert!(r.holds_ssr_rays(), "the other scene leaves the rays' state");
    }
    shot(&mut r, &traced);
    assert_eq!(r.ssr_maps_builds(), 1, "the array is built once");
    let cam = traced.find_entity_by_name("Camera").unwrap();
    traced.world.post_processing_mut(cam).unwrap().ssr.tracing = SsrTracing::RayMarching;
    shot(&mut r, &traced);
    assert!(
        !r.holds_ssr_rays(),
        "turned off in its scene, the rays' state is freed"
    );
}
