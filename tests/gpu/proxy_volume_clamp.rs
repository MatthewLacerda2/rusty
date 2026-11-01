//! A Light Probe Proxy Volume clamps to its box, as Unity's does (#1672): a pixel
//! outside a `Custom` box reads the box's nearest face, never the object's single
//! sample, so a mesh larger than its box (or an arm swung past its bind-pose
//! bounds) has no seam where the box ends.
//!
//! Skips (passes without asserting) when no GPU / software adapter is present.

use glam::Vec3;
use rusty::components::{LightProbeProxyVolumeComponent, ProxyBoundsMode, ProxyResolutionMode};

use super::proxy_volume_screenshot::{baked_room, top_and_foot};

#[test]
fn a_foot_outside_a_custom_box_reads_the_box_s_face_not_the_single_sample() {
    let Some((mut scene, locker)) = baked_room() else {
        eprintln!("[proxy clamp] no GPU/software adapter — skipping");
        return;
    };
    let (_, single_foot) = top_and_foot(&scene, "clamp_single");
    // A box over the locker's top quarter only (local y 0.25..0.5, world 1.5..2 m).
    let volume = LightProbeProxyVolumeComponent {
        bounds_mode: ProxyBoundsMode::Custom,
        origin: Vec3::new(0.0, 0.375, 0.0),
        size: Vec3::new(1.0, 0.25, 1.0),
        resolution_mode: ProxyResolutionMode::Custom,
        grid_resolution: [1, 2, 1],
        ..Default::default()
    };
    scene
        .world
        .set_light_probe_proxy_volume(locker, Some(volume));
    let (top, foot) = top_and_foot(&scene, "clamp_box");
    eprintln!("[proxy clamp] single foot={single_foot:.1}; boxed top={top:.1} foot={foot:.1}");
    assert!(
        foot > single_foot + 5.0,
        "the foot reads the lit box's bottom face: {foot:.1} vs {single_foot:.1}"
    );
}
