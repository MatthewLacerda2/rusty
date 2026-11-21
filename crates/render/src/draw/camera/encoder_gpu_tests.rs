use super::SPLIT;
use crate::test_gpu::ssil::{block, draw, eye, on, renderer, scene, view, LAMP};
use crate::{EarlyFlushes, Renderer};
use glam::Vec3;
use rusty_components::{CameraComponent, RenderMode};
use rusty_scene::{ClearFlags, Scene};

fn lit(stacked: bool) -> Scene {
    let (mut scene, _) = scene(on(1.0), LAMP);
    let pane = (Vec3::new(-1.0, 1.0, -4.0), Vec3::new(1.5, 1.5, 0.1));
    block(&mut scene, "glass", pane, [0.2, 0.4, 1.0]);
    let glass = scene.materials.get_mut("glass").expect("made above");
    (glass.render_mode, glass.alpha) = (RenderMode::Transparent, 0.5);
    let (a, b) = (Vec3::new(-1.0, 1.0, -5.0), Vec3::new(1.0, 1.5, -5.0));
    scene.debug_lines.push(a, b, [0.0, 1.0, 0.0, 1.0], 600);
    if stacked {
        let overlay = scene.add_entity("ViewmodelCamera".to_string());
        let lens = CameraComponent {
            render_order: 1,
            clear_flags: ClearFlags::DepthOnly,
            culling_mask: 0,
            fov: 20.0,
            ..Default::default()
        };
        scene.world.set_camera(overlay, Some(lens));
    }
    scene
}

fn frames(r: &mut Renderer, scene: &Scene, split: bool, flushes: EarlyFlushes) -> Vec<Vec<u8>> {
    SPLIT.with(|s| s.set(split));
    r.switches.set_flushes(flushes);
    let mut v = view(r);
    v.game_gizmos = true;
    let shots = (0..4).map(|_| draw(r, &mut v, scene, &eye(0.0))).collect();
    SPLIT.with(|s| s.set(false));
    shots
}

#[test]
fn gpu_a_frame_in_one_encoder_draws_what_a_submit_per_stage_drew() {
    let Some(mut r) = renderer() else { return };
    for stacked in [false, true] {
        let scene = lit(stacked);
        let apart = frames(&mut r, &scene, true, EarlyFlushes::None);
        assert!(r.frame_counters.ssil_samples > 0, "SSIL ran");
        for flushes in EarlyFlushes::ALL {
            let one = frames(&mut r, &scene, false, flushes);
            let green = |img: &[u8]| img.chunks(4).filter(|p| p[1] > 200 && p[0] < 80).count();
            assert!(
                green(&one[3]) > 0,
                "the gizmo line drew (stacked {stacked}, {flushes})"
            );
            assert!(
                apart == one,
                "the frames differ (stacked {stacked}, {flushes})"
            );
        }
    }
}
