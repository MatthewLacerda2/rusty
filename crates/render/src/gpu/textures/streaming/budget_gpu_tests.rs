use super::gpu_tests::{boxes, gradient, settle};

const RES: u32 = 256;

#[test]
fn gpu_a_budget_below_the_scenes_need_holds_texture_memory_under_it() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    r.stream_textures(true);
    let maps: Vec<_> = (0..4)
        .map(|i| gradient(&format!("budget_{i}"), 512))
        .collect();
    let placed: Vec<_> = maps.iter().map(|m| (m, 4.0)).collect();
    let scene = boxes(&placed);
    let all: Vec<_> = maps.iter().collect();
    r.quality.texture_streaming.budget_mb = 64;
    settle(&mut r, &scene, RES, &all);
    let need = r.frame_counters.texture_bytes;
    assert!(
        need > 1 << 20,
        "the scene needs {need} bytes: more than the budget below"
    );
    r.quality.texture_streaming.budget_mb = 1;
    settle(&mut r, &scene, RES, &all);
    let held = r.frame_counters.texture_bytes;
    assert!(held <= 1 << 20, "{held} bytes resident over a 1 MB budget");
    r.quality.texture_streaming.budget_mb = 64;
    settle(&mut r, &scene, RES, &all);
    let levels: Vec<_> = all
        .iter()
        .map(|m| super::gpu_tests::dropped(&r, m))
        .collect();
    assert_eq!(
        levels, [1; 4],
        "a raised budget brings back what the screen size needs"
    );
    assert!(r.frame_counters.texture_bytes > held);
}
