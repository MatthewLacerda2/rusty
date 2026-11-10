//! Editor overlay gizmos for baked lighting probes (#284). Visualization only — a
//! glance at the scene shows where light/reflection probes sit and what they captured,
//! so you don't bake blind. Nothing here touches the bake, the SH/cubemap data, or the
//! lit-scene shading; the gizmos draw through the SAME editor-overlay line path as the
//! grid/AABB/axis overlays, in the handles pass an editor camera runs after post-FX
//! (`overlays::handles`, #1224), never in a shipped game render.
//!
//! Two families, line-list geometry on the `line_pipeline`:
//!   * Light probes — a small 3-axis cross marker at each probe position, TINTED by the
//!     probe's baked SH irradiance (sampled toward world-up), so dead/dark or
//!     mis-bounced probes read at a glance.
//!   * Reflection probes — a wireframe of the parallax volume: the box
//!     (`box_min`..`box_max`, via the shared `aabb_wireframe` helper) turned by the
//!     probe's rotation, or three great circles of a sphere probe (#1995). The capture position is not a line: it is
//!     a mirror ball showing the probe's cube, or grey when the cube is not resident
//!     (`probe_ball`, #1911), drawn before these lines through its own pipeline.
//!
//! The pure helpers (`sh_marker_tint`, `probe_marker_verts`) are GPU-free and unit
//! tested; the GPU pre-creation builds per-probe vertex + uniform buffers per frame,
//! like the occlusion view's boxes. Each family draws only while the Scene view's
//! gizmo set has it on (`Gizmo::LightProbes`, `Gizmo::ReflectionProbes`, #1909);
//! Area lights are not probes and always draw.

use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

use super::{aabb_wireframe, OverlayBindings, OverlayDraws};
use crate::components::mesh::Vertex;
use crate::core::application::{Gizmo, SceneGizmos};
use crate::render::draw::resources::ProbeResource;
use crate::render::Renderer;
use crate::scene::lighting::sh::Sh9;
use crate::scene::{ProbeShape, ReflectionProbe, Scene};

/// Half-length of a probe cross marker's arms, in world units.
const MARKER_HALF: f32 = 0.15;
/// Tint for the reflection-probe gizmo's parallax box: a warm amber, kept
/// distinct from the SH-driven light-probe markers.
const REFLECTION_TINT: [f32; 4] = [1.0, 0.65, 0.1, 0.9];

impl Renderer {
    /// Pre-create the light- and reflection-probe gizmo resources for one editor pass,
    /// and the Area lights' (`lights`, #1674).
    /// Each entry owns its line-list vertex buffer and a slot of `draws` (its flat
    /// tint). Empty when `gizmos` leaves both probe kinds off (or there are none)
    /// and no Area light is active.
    pub(crate) fn precreate_probes(
        &self,
        scene: &Scene,
        gizmos: &SceneGizmos,
        draws: &mut OverlayDraws,
    ) -> Vec<ProbeResource> {
        let mut resources = Vec::new();
        // Light probes: a marker per probe, tinted by its baked SH irradiance.
        if gizmos.on(Gizmo::LightProbes) {
            for probe in scene.probes.probes() {
                let verts = probe_marker_verts(probe.position, MARKER_HALF);
                let tint = sh_marker_tint(&probe.sh);
                resources.push(self.build_probe_resource(draws, &verts, tint));
            }
        }
        // Reflection probes: a parallax-box wireframe; the capture point is a mirror
        // ball (`probe_ball`, #1911).
        if gizmos.on(Gizmo::ReflectionProbes) {
            for probe in &scene.reflection_probes.probes {
                let verts = reflection_volume_verts(probe);
                resources.push(self.build_probe_resource(draws, &verts, REFLECTION_TINT));
            }
        }
        // Area lights (#1674), baked only, ride the same line path.
        for (verts, tint) in super::lights::area_light_gizmos(scene) {
            resources.push(self.build_probe_resource(draws, &verts, tint));
        }
        resources
    }

    /// Build one probe (or Area light) gizmo's GPU resources: a vertex buffer for its line-list geometry
    /// (already in world space) and its slot of `draws`, drawn flat in `tint`.
    fn build_probe_resource(
        &self,
        draws: &mut OverlayDraws,
        verts: &[Vertex],
        tint: [f32; 4],
    ) -> ProbeResource {
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Probe & Area Light Gizmo Vertices"),
                contents: bytemuck::cast_slice(verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let slot = draws.push(Mat4::IDENTITY, tint);
        (vertex_buffer, slot, verts.len() as u32)
    }

    /// Draw the probe gizmos into the handles pass. Caller has already bound the
    /// `line_pipeline`, group 0 (camera/lighting) and group 3 (shadow); this binds each
    /// probe's slot of `bindings` + group-2 default material (line draws never sample it).
    pub(super) fn draw_probe_overlays<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        bindings: &'a OverlayBindings,
        probe_resources: &'a [ProbeResource],
    ) {
        render_pass.set_bind_group(2, &self.default_material_bind_group, &[]);
        for (vertex_buffer, slot, count) in probe_resources {
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            bindings.bind(render_pass, *slot);
            render_pass.draw(0..*count, 0..1);
        }
    }
}

/// The representative marker colour for a light probe, derived from its baked SH. We
/// reconstruct diffuse irradiance toward world-up (`Sh9::eval(Vec3::Y)`) over π — what
/// the shading path gives a white, upward-facing surface (#807) — so the tint reads as
/// "what light is arriving from above" at the probe, on the scale the scene renders. A
/// zero/unbaked probe → black; a bright probe → bright; a warm/cool bounce shows in the
/// hue. The colour is tone-mapped (Reinhard) into `[0, 1]` so HDR
/// irradiance stays a legible tint instead of clipping to flat white; alpha is fixed.
pub(crate) fn sh_marker_tint(sh: &Sh9) -> [f32; 4] {
    let lambert = sh.eval(Vec3::Y).max(Vec3::ZERO) / std::f32::consts::PI;
    let mapped = lambert / (lambert + Vec3::ONE);
    [mapped.x, mapped.y, mapped.z, 1.0]
}

/// Line-list vertices for a 3-axis cross marker centred at `center`, each arm extending
/// `half` along ±X, ±Y, ±Z. Three line segments → 6 vertices: a light probe's marker.
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

/// Line-list vertices of a reflection probe's volume (#1995): its box's 12 edges
/// turned by its rotation about its capture point, or a sphere's three great circles.
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

    /// A zero (unbaked) probe reconstructs no irradiance → a black marker.
    #[test]
    fn zero_sh_tints_black() {
        let tint = sh_marker_tint(&Sh9::zero());
        assert_eq!(tint, [0.0, 0.0, 0.0, 1.0]);
    }

    /// A probe carrying positive DC (constant ambient) reconstructs a positive,
    /// in-gamut grey marker — brighter than black, never above 1.0 after tone-mapping.
    #[test]
    fn bright_sh_tints_bright() {
        let mut sh = Sh9::zero();
        // DC band: a large constant white radiance. eval applies the band-0 cosine lobe.
        sh.coeffs[0] = [10.0, 10.0, 10.0];
        let tint = sh_marker_tint(&sh);
        assert!(tint[0] > 0.5, "expected a bright marker, got {tint:?}");
        for c in &tint[..3] {
            assert!(*c > 0.0 && *c <= 1.0, "channel out of [0,1]: {c}");
        }
        assert_eq!(tint[3], 1.0);
    }

    /// The tint is on the scene's scale (#807): a probe of a uniform sky of radiance L
    /// tints as a white floor under it renders, L, Reinhard-mapped to L / (L + 1) — not
    /// the irradiance πL.
    #[test]
    fn a_uniform_sky_tints_by_its_radiance() {
        let mut sh = Sh9::zero();
        // A uniform radiance L projects onto the DC band alone: c0 = L · Y00 · 4π.
        let l = 1.0;
        sh.coeffs[0] = [l * 0.282_094_8 * 4.0 * std::f32::consts::PI; 3];
        let tint = sh_marker_tint(&sh);
        assert!((tint[0] - l / (l + 1.0)).abs() < 1e-4, "tint {tint:?}");
    }

    /// The hue survives: a probe that captured a warm (red-dominant) DC term reads
    /// red-dominant in the marker tint, so a glance distinguishes warm from cool bounces.
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

    /// A turned box's wireframe ends at its turned corners; a sphere's sits on it.
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

    /// The cross marker is three segments (6 vertices) centred on the point, each arm
    /// `half` long on its axis.
    #[test]
    fn marker_has_three_axis_segments() {
        let verts = probe_marker_verts(Vec3::new(1.0, 2.0, 3.0), 0.25);
        assert_eq!(verts.len(), 6);
        // First segment spans ±X about the centre.
        assert_eq!(verts[0].position, [0.75, 2.0, 3.0]);
        assert_eq!(verts[1].position, [1.25, 2.0, 3.0]);
        // Third segment spans ±Z about the centre.
        assert_eq!(verts[4].position, [1.0, 2.0, 2.75]);
        assert_eq!(verts[5].position, [1.0, 2.0, 3.25]);
    }
}
