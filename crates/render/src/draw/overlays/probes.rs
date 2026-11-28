use glam::{Quat, Vec3};

use super::{aabb_wireframe, OverlayDraws};
use crate::draw::resources::LineResource;
use crate::Renderer;
use rusty_components::mesh::Vertex;
use rusty_core::application::{Gizmo, SceneGizmos};
use rusty_scene::lighting::sh::Sh9;
use rusty_scene::{ProbeShape, ReflectionProbe, Scene};

/// Metres: half a light-probe cross marker's arm.
const MARKER_HALF: f32 = 0.15;
const REFLECTION_TINT: [f32; 4] = [1.0, 0.65, 0.1, 0.9];

impl Renderer {
    pub(crate) fn precreate_probes(
        &self,
        scene: &Scene,
        gizmos: &SceneGizmos,
        draws: &mut OverlayDraws,
    ) -> Option<LineResource> {
        let verts = probe_lines(scene, gizmos);
        self.vertex_lines(draws, &verts, "Probe & Area Light Gizmo Vertices")
    }
}

pub(crate) fn probe_lines(scene: &Scene, gizmos: &SceneGizmos) -> Vec<Vertex> {
    let mut out = Vec::new();
    let mut add = |verts: Vec<Vertex>, tint: [f32; 4]| {
        out.extend(verts.into_iter().map(|v| Vertex { tangent: tint, ..v }));
    };
    if gizmos.on(Gizmo::LightProbes) {
        let sets = scene.probe_sets();
        for probe in sets.iter().flat_map(|(_, v)| v.probes()) {
            let tint = sh_marker_tint(&probe.sh);
            add(probe_marker_verts(probe.position, MARKER_HALF), tint);
        }
    }
    let inspected = scene.inspected_reflection_probe;
    let probe = inspected.and_then(|i| scene.reflection_probes.probes.get(i));
    if let Some(probe) = probe.filter(|_| gizmos.on(Gizmo::ReflectionProbes)) {
        add(reflection_volume_verts(probe), REFLECTION_TINT);
    }
    for (verts, tint) in super::lights::area_light_gizmos(scene) {
        add(verts, tint);
    }
    out
}

pub(crate) fn sh_marker_tint(sh: &Sh9) -> [f32; 4] {
    let lambert = sh.eval(Vec3::Y).max(Vec3::ZERO) / std::f32::consts::PI;
    let mapped = lambert / (lambert + Vec3::ONE);
    [mapped.x, mapped.y, mapped.z, 1.0]
}

pub(crate) fn probe_marker_verts(center: Vec3, half: f32) -> Vec<Vertex> {
    let normal = Vec3::Y;
    let uv = [0.0, 0.0];
    let mut verts = Vec::with_capacity(6);
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        let arm = axis * half;
        verts.push(Vertex::new(center - arm, normal, uv));
        verts.push(Vertex::new(center + arm, normal, uv));
    }
    verts
}

pub(crate) fn reflection_volume_verts(probe: &ReflectionProbe) -> Vec<Vertex> {
    const SEGMENTS: usize = 32;
    if probe.shape == ProbeShape::Box {
        let mut verts = aabb_wireframe(probe.box_min, probe.box_max);
        if probe.rotation != Quat::IDENTITY {
            let c = probe.position;
            for v in &mut verts {
                v.position = (c + probe.rotation * (Vec3::from(v.position) - c)).to_array();
            }
        }
        return verts;
    }
    let mut verts = Vec::with_capacity(3 * SEGMENTS * 2);
    let at = |a: Vec3, b: Vec3, k: usize| {
        let t = k as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let p = probe.position + (a * t.cos() + b * t.sin()) * probe.radius;
        Vertex::new(p, Vec3::Y, [0.0, 0.0])
    };
    for (a, b) in [(Vec3::X, Vec3::Y), (Vec3::Y, Vec3::Z), (Vec3::Z, Vec3::X)] {
        for k in 0..SEGMENTS {
            verts.push(at(a, b, k));
            verts.push(at(a, b, k + 1));
        }
    }
    verts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_sh_tints_black() {
        let tint = sh_marker_tint(&Sh9::zero());
        assert_eq!(tint, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn bright_sh_tints_bright() {
        let mut sh = Sh9::zero();
        sh.coeffs[0] = [10.0, 10.0, 10.0];
        let tint = sh_marker_tint(&sh);
        assert!(tint[0] > 0.5, "expected a bright marker, got {tint:?}");
        for c in &tint[..3] {
            assert!(*c > 0.0 && *c <= 1.0, "channel out of [0,1]: {c}");
        }
        assert_eq!(tint[3], 1.0);
    }

    #[test]
    fn a_uniform_sky_tints_by_its_radiance() {
        let mut sh = Sh9::zero();
        let l = 1.0;
        sh.coeffs[0] = [l * 0.282_094_8 * 4.0 * std::f32::consts::PI; 3];
        let tint = sh_marker_tint(&sh);
        assert!((tint[0] - l / (l + 1.0)).abs() < 1e-4, "tint {tint:?}");
    }

    #[test]
    fn warm_sh_keeps_warm_hue() {
        let mut sh = Sh9::zero();
        sh.coeffs[0] = [8.0, 2.0, 1.0];
        let tint = sh_marker_tint(&sh);
        assert!(
            tint[0] > tint[1] && tint[1] > tint[2],
            "warm hue not preserved: {tint:?}"
        );
    }

    #[test]
    fn a_reflection_volume_wireframe_follows_its_rotation_and_shape() {
        let mut p = ReflectionProbe::new(Vec3::new(2.0, 0.0, 0.0), Vec3::ONE);
        p.rotation = Quat::from_rotation_y(0.5);
        let corners = p.corners();
        for v in reflection_volume_verts(&p) {
            let near = |c: &Vec3| c.distance(Vec3::from(v.position)) < 1e-5;
            assert!(corners.iter().any(near), "{:?}", v.position);
        }
        (p.shape, p.radius) = (ProbeShape::Sphere, 3.0);
        for v in reflection_volume_verts(&p) {
            let r = Vec3::from(v.position).distance(p.position);
            assert!((r - 3.0).abs() < 1e-4, "{r}");
        }
    }

    #[test]
    fn marker_has_three_axis_segments() {
        let verts = probe_marker_verts(Vec3::new(1.0, 2.0, 3.0), 0.25);
        assert_eq!(verts.len(), 6);
        assert_eq!(verts[0].position, [0.75, 2.0, 3.0]);
        assert_eq!(verts[1].position, [1.25, 2.0, 3.0]);
        assert_eq!(verts[4].position, [1.0, 2.0, 2.75]);
        assert_eq!(verts[5].position, [1.0, 2.0, 3.25]);
    }
}

#[cfg(test)]
#[path = "probes_tests.rs"]
mod lines_tests;
