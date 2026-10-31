//! A capture honours the player graphics options (#1438): a harness screenshot of
//! the bloom scene with `bloom` forbidden in the world's graphics cell is the frame
//! without bloom, so a `play` scenario or an MCP shot shows what
//! `Graphics.GetOption` says. Before, the capture renderer kept its own defaults.

use glam::Vec3;
use rusty::core::settings::graphics_options::GraphicsOption;
use rusty::dev::capture::CaptureHost;
use rusty::dev::harness::Harness;
use rusty::dev::screenshot::capture_world_into;
use rusty::scene::camera::{set_view_orientation, set_view_position};

use crate::gpu::bloom_screenshot::scene;
use crate::gpu::mean_brightness;

#[test]
fn a_capture_drops_the_effect_an_option_forbids() {
    let out = crate::temp::dir().join(format!("rusty_options_gpu_{}", std::process::id()));
    let h = Harness::new(&out, "");
    let world = h.world.borrow();
    *world.scene().borrow_mut() = scene(true);
    // The shot is the main view: the scene's camera entity, moved as `Camera.*` does.
    {
        let (mut cam, mut scene) = (world.camera().borrow_mut(), world.scene().borrow_mut());
        set_view_position(&mut cam, &mut scene, Vec3::new(0.0, 0.0, 5.0));
        set_view_orientation(&mut cam, &mut scene, -90.0, 0.0);
    }
    let mut host = CaptureHost::new();
    let mut shot = |allow: bool, name: &str| {
        let cell = world.script_manager().graphics_cell();
        cell.borrow_mut()
            .options
            .choose(GraphicsOption::Bloom, allow);
        let path = out.join(name);
        capture_world_into(&mut host, &world, &path, 64, 64)
            .expect("capture")
            .then(|| mean_brightness(&path))
    };
    let (Some(on), Some(off)) = (shot(true, "on.png"), shot(false, "off.png")) else {
        assert!(
            !rusty::render::gpu_required(),
            "no adapter, but one is required"
        );
        return;
    };
    eprintln!("[options] bloom allowed={on:.3} forbidden={off:.3}");
    assert!(
        on > off + 5.0,
        "forbidding bloom must drop it from the shot (allowed={on:.3}, forbidden={off:.3})"
    );
    drop(world);
    let _ = std::fs::remove_dir_all(out);
}
