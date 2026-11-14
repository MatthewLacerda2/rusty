//! TAA's gate (#1702): which views run the resolve, adapter-free, since the GPU
//! tests all draw a Game view (`editor_mode = false`) and a full post-FX chain.

use super::super::taa::wanted;
use super::*;

fn taa_cam(on: bool) -> CameraComponent {
    CameraComponent {
        taa_active: on,
        ..cam()
    }
}

/// Whether `camera` runs TAA under `graphics`, drawn `full` (not a display-only
/// render texture) and as the Scene view or not.
fn runs(
    scene: &Scene,
    camera: Option<u32>,
    graphics: GraphicsSettings,
    full: bool,
    scene_view: bool,
) -> bool {
    wanted(
        borrow_post_source(scene, camera).source(),
        graphics,
        full,
        scene_view,
    )
}

/// The camera's switch, then the player's option: TAA is off by default, runs when
/// the camera asks and the player allows it, and needs no volume (a camera effect,
/// like FXAA).
#[test]
fn taa_runs_when_the_camera_asks_and_the_player_allows() {
    let mut scene = Scene::new();
    let off = add(&mut scene, None, Some(cam()));
    let on = add(&mut scene, None, Some(taa_cam(true)));
    assert!(
        !runs(&scene, Some(off), high(), true, false),
        "off by default"
    );
    assert!(
        runs(&scene, Some(on), high(), true, false),
        "on, no volume needed"
    );
    let forbidden = forbidding(GraphicsOption::Taa);
    assert!(
        !runs(&scene, Some(on), forbidden, true, false),
        "the player's option"
    );
    assert!(
        !runs(&scene, None, high(), true, false),
        "no camera, no TAA"
    );
}

/// The views that never jitter, whatever the camera says: the editor's Scene view
/// (it keeps FXAA) and a display-only render texture (`post_fx = false`).
#[test]
fn the_scene_view_and_a_display_only_view_never_run_taa() {
    let (scene, id) = scene_with_vc(vc(), Some(taa_cam(true)));
    assert!(
        runs(&scene, Some(id), high(), true, false),
        "the Game view runs it"
    );
    assert!(
        !runs(&scene, Some(id), high(), true, true),
        "the Scene view does not"
    );
    assert!(
        !runs(&scene, Some(id), high(), false, false),
        "display-only does not"
    );
}
