//! `MotionVectors::begin` (#1601): which frames hold a velocity target, which record
//! entity poses, and a resize making a target of the new size.
//!
//! Equivalent mutant: `create_target`'s `|` → `^` between `TextureUsages` flags.
//! The flags are distinct bits, so XOR sets the same ones OR does.

use glam::Mat4;

use super::{MotionVectors, Velocity};
use crate::render::PixelSize;
use crate::scene::Scene;

fn size_of(mv: &MotionVectors) -> Option<(u32, u32)> {
    let t = mv.active_target()?;
    Some((t.texture.width(), t.texture.height()))
}

#[test]
fn gpu_begin_holds_a_target_only_while_something_reads_it() {
    let Some(r) = crate::render::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let (key, cams) = (
        super::tests::key(Scene::new().id(), Some(1)),
        super::tests::one(Mat4::IDENTITY),
    );
    let mut mv = MotionVectors::default();
    let frame = |mv: &mut MotionVectors, size: PixelSize, velocity| {
        mv.begin(&r.device, size, key, velocity, &cams);
        (size_of(mv), mv.active(), mv.target_view().is_some())
    };
    let small = PixelSize::new(16, 8);
    assert_eq!(
        frame(&mut mv, small, Velocity::Motion),
        (Some((16, 8)), true, true)
    );
    assert_eq!(
        frame(&mut mv, small, Velocity::Depths),
        (Some((16, 8)), false, true),
        "SSR's depths alone record no poses"
    );
    assert_eq!(
        frame(&mut mv, PixelSize::new(24, 12), Velocity::Motion),
        (Some((24, 12)), true, true),
        "a resize makes a target of the new size"
    );
    assert_eq!(frame(&mut mv, small, Velocity::Off), (None, false, false));
}
