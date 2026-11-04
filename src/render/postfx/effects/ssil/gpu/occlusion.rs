//! What the gather must not see (#1750): light through a partition between a floor
//! point and a lit wall, a flat floor's own light, and fog taken for light.

use glam::Vec3;

use super::fixture::{block, floor, lamp, on, patch, renderer, scene, settled, LAMP};
use crate::components::Ssil;
use crate::scene::{FogMode, Scene};

/// The floor's colour with SSIL on less with it off at `p`: the bounce alone.
fn bounce(r: &mut crate::render::Renderer, build: impl Fn(Ssil) -> Scene, p: Vec3) -> Vec3 {
    let lit = settled(r, &build(on(1.0)), 6);
    let plain = settled(r, &build(Ssil::default()), 1);
    Vec3::from(patch(&lit, p)) - Vec3::from(patch(&plain, p))
}

/// A floor, a red wall at z = -5 lit by a lamp in front of it, and, when
/// `partition`, a 1 m partition at z = -4 between the wall and the floor at -3.5.
fn cover(ssil: Ssil, partition: bool) -> Scene {
    let mut scene = floor(ssil);
    let wall = (Vec3::new(0.0, 1.5, -5.1), Vec3::new(6.0, 3.0, 0.2));
    block(&mut scene, "wall", wall, [1.0, 0.05, 0.05]);
    if partition {
        let low = (Vec3::new(0.0, 0.5, -4.0), Vec3::new(6.0, 1.0, 0.1));
        block(&mut scene, "partition", low, [0.9, 0.9, 0.9]);
    }
    lamp(&mut scene, Vec3::new(0.0, 2.2, -4.5), LAMP);
    scene
}

#[test]
fn gpu_a_partition_keeps_the_walls_light_off_the_floor_behind_it() {
    let Some(mut r) = renderer() else { return };
    // 2.1 m from the floor point to the wall's top over the partition: in reach.
    let p = Vec3::new(1.0, 0.0, -3.5);
    let open = bounce(&mut r, |s| cover(s, false), p).x;
    let covered = bounce(&mut r, |s| cover(s, true), p).x;
    eprintln!("[ssil] the floor's bounce at {p}: {open} open, {covered} behind the partition");
    assert!(open > 0.0 && covered <= open * 0.25, "{covered} vs {open}");
}

#[test]
fn gpu_a_white_floor_alone_bounces_nothing_onto_itself() {
    let Some(mut r) = renderer() else { return };
    // A lamp low over the floor far out, where a pixel spans the most depth: depth
    // noise must not pass the floor's own samples as another surface.
    let mut scene = floor(on(4.0));
    let far = lamp(&mut scene, Vec3::new(0.0, 0.6, -18.0), LAMP * 8.0);
    scene.world.light_mut(far).unwrap().range = 10.0;
    let mut v = super::fixture::view(&r);
    for _ in 0..6 {
        super::fixture::draw(&mut r, &mut v, &scene, &super::fixture::eye(0.0));
    }
    let result = v.post_fx.ssil_result(&r.device, &r.queue);
    let most = result.iter().map(|c| c.max_element()).fold(0.0, f32::max);
    eprintln!("[ssil] a lit floor alone: its brightest bounce {most}");
    assert!(most <= 1e-3, "the floor lit itself: {most}");
}

#[test]
fn gpu_fog_is_not_gathered_as_light() {
    let Some(mut r) = renderer() else { return };
    // A dark scene: no ambient, a dim lamp on the red wall, and thick grey fog: the
    // wall's fogged colour is mostly fog, and only the lamp's red is light.
    let at = super::fixture::FLOOR;
    let build = |fogged: bool| {
        move |ssil| {
            let (mut scene, _) = scene(ssil, LAMP * 0.05);
            scene.ambient_color = Vec3::ZERO;
            if fogged {
                scene.fog.mode = FogMode::Exponential;
                (scene.fog.density, scene.fog.start) = (0.3, 0.0);
                scene.fog.color = Vec3::splat(0.8);
            }
            scene
        }
    };
    let clear = bounce(&mut r, build(false), at);
    let fogged = bounce(&mut r, build(true), at);
    eprintln!("[ssil] a dim lamp's bounce: {clear} clear, {fogged} in fog");
    // Fog dims the lamp's light, so its bounce too; it bounces none of its own grey.
    assert!(clear.x > 0.0 && fogged.x <= clear.x, "{fogged} vs {clear}");
    assert!(fogged.y <= 1e-3, "the fog's grey bounced: {fogged}");
}
