//! GPU occlusion culling under other projections (#1963): an orthographic camera
//! culls the box behind the wall too, since the test reads the depth it drew; a
//! near plane that cuts the wall open hides nothing, so the box is drawn.

use super::gpu_tests::{drawn, facing_the_wall, offscreen, shot, walled, H, W};
use crate::components::Projection;

#[test]
fn gpu_an_orthographic_camera_culls_and_a_far_near_plane_does_not() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let (scene, _) = walled();
    let mut ortho = facing_the_wall();
    ortho.projection = Projection::Orthographic { size: 4.0 };
    let mut view = offscreen(&r);
    let (on, _) = shot(&mut r, &mut view, &scene, &ortho);
    assert_eq!(drawn(&r, &view), [1], "the wall alone, orthographic");
    // A near plane through the wall (7.8 to 8.2 m out): its front face is clipped
    // and its back face turned away, so the box shows through it.
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
