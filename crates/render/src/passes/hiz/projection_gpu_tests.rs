use super::gpu_tests::{drawn, facing_the_wall, offscreen, shot, walled, H, W};
use rusty_components::Projection;

#[test]
fn gpu_an_orthographic_camera_culls_and_a_far_near_plane_does_not() {
    let Some(mut r) = crate::test_gpu::culling::culling_or_skip(W, H) else {
        return;
    };
    let (scene, _) = walled();
    let mut ortho = facing_the_wall();
    ortho.projection = Projection::Orthographic { size: 4.0 };
    let mut view = offscreen(&r);
    let (on, _) = shot(&mut r, &mut view, &scene, &ortho);
    assert_eq!(drawn(&r, &view), [1], "the wall alone, orthographic");
    let mut far = facing_the_wall();
    far.near = 8.0;
    let mut view = offscreen(&r);
    let (far_on, c) = shot(&mut r, &mut view, &scene, &far);
    assert_eq!(c.hiz_tested, 2, "{c:?}");
    assert_eq!(drawn(&r, &view), [2], "nothing hides the box");
    r.quality.gpu_occlusion_culling = false;
    for (name, cam, with) in [("orthographic", ortho, on), ("far near", far, far_on)] {
        let mut fresh = offscreen(&r);
        let (off, _) = shot(&mut r, &mut fresh, &scene, &cam);
        assert!(with == off, "{name}: Hi-Z moved pixels");
    }
}
