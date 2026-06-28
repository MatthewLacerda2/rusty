//! Editor overlay gizmos for baked lighting probes (#284). Visualization only — a
//! glance at the scene shows where light/reflection probes sit and what they captured,
//! so you don't bake blind. Nothing here touches the bake, the SH/cubemap data, or the
//! lit-scene shading; the gizmos draw through the SAME editor-overlay line path as the
//! grid/AABB/axis overlays and ride the SAME editor-mode toggle (built in
//! `precreate_overlays` only when `editor_mode`, never in a shipped game render).
//!
//! Two families, both line-list geometry on the `line_pipeline`:
//!   * Light probes — a small 3-axis cross marker at each probe position, TINTED by the
//!     probe's baked SH irradiance (sampled toward world-up), so dead/dark or
//!     mis-bounced probes read at a glance.
//!   * Reflection probes — a wireframe of the parallax box (`box_min`..`box_max`, via
//!     the shared `aabb_wireframe` helper) plus a cross marker at the capture position.
//!
//! The pure helpers (`sh_marker_tint`, `probe_marker_verts`) are GPU-free and unit
//! tested; the GPU pre-creation builds per-probe vertex + uniform buffers per frame,
//! exactly like `precreate_aabb`.

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::render::draw::overlays::aabb_wireframe;
use crate::render::draw::resources::ProbeResource;
use crate::render::gpu::mesh::Vertex;
use crate::render::{EntityUniform, Renderer};
use crate::scene::lighting::sh::Sh9;
use crate::scene::Scene;

/// Half-length of a probe cross marker's arms, in world units.
const MARKER_HALF: f32 = 0.15;
/// Tint for reflection-probe gizmos (capture marker + parallax box): a warm amber, kept
/// distinct from the SH-driven light-probe markers and the cyan collider AABBs.
const REFLECTION_TINT: [f32; 4] = [1.0, 0.65, 0.1, 0.9];

impl Renderer {
    /// Pre-create the light- and reflection-probe gizmo resources for one editor pass.
    /// Each entry owns its line-list vertex buffer + flat-unlit uniform buffer + bind
    /// group; the bone palette is the renderer's shared identity buffer (no per-probe
    /// allocation), mirroring `precreate_aabb`. Empty when both probe sets are empty.
    pub(crate) fn precreate_probes(&self, scene: &Scene) -> Vec<ProbeResource> {
        let mut resources = Vec::new();
        // Light probes: a marker per probe, tinted by its baked SH irradiance.
        for probe in &scene.probes.probes {
            let verts = probe_marker_verts(probe.position, MARKER_HALF);
            let tint = sh_marker_tint(&probe.sh);
            resources.push(self.build_probe_resource(&verts, tint));
        }
        // Reflection probes: a parallax-box wireframe + a capture-position marker.
        for probe in &scene.reflection_probes.probes {
            let box_verts = aabb_wireframe(probe.box_min, probe.box_max);
            resources.push(self.build_probe_resource(&box_verts, REFLECTION_TINT));
            let marker_verts = probe_marker_verts(probe.position, MARKER_HALF);
            resources.push(self.build_probe_resource(&marker_verts, REFLECTION_TINT));
        }
        resources
    }

    /// Build one probe gizmo's GPU resources: a vertex buffer for its line-list geometry
    /// (already in world space) plus a flat-unlit uniform buffer + bind group.
    fn build_probe_resource(&self, verts: &[Vertex], tint: [f32; 4]) -> ProbeResource {
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Probe Gizmo Vertices"),
                contents: bytemuck::cast_slice(verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let entity_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Probe Gizmo Uniform"),
                contents: bytemuck::bytes_of(&flat_overlay_uniform(tint)),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind_group =
            self.entity_bind_group("Probe Gizmo", &entity_buf, self.shared_bones_buffer());
        (vertex_buffer, entity_buf, bind_group, verts.len() as u32)
    }

    /// Draw the probe gizmos into the scene pass. Caller has already bound the
    /// `line_pipeline`, group 0 (camera/lighting) and group 3 (shadow); this binds each
    /// probe's group-1 uniform + group-2 default material (line draws never sample it).
    pub(crate) fn draw_probe_overlays<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        probe_resources: &'a [ProbeResource],
    ) {
        render_pass.set_bind_group(2, &self.default_material_bind_group, &[]);
        for (vertex_buffer, _entity_buf, bind_group, count) in probe_resources {
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_bind_group(1, bind_group, &[]);
            render_pass.draw(0..*count, 0..1);
        }
    }
}

/// A flat, unlit overlay uniform: identity model matrix (gizmo vertices are authored in
/// world space), the given `tint`, no textures/SH/cutout. The shared shape every line
/// overlay draws with.
fn flat_overlay_uniform(tint: [f32; 4]) -> EntityUniform {
    EntityUniform {
        model_matrix: Mat4::IDENTITY.to_cols_array(),
        color_tint: tint,
        use_texture: 0,
        is_lit: 0,
        metallic: 0.0,
        roughness: 0.5,
        use_metallic_map: 0,
        use_roughness_map: 0,
        use_normal_map: 0,
        use_emissive_map: 0,
        emissive: [0.0; 4],
        use_sh: 0,
        use_cutout: 0,
        alpha_cutoff: 0.0,
        _sh_pad: 0,
        sh: [[0.0; 4]; 9],
    }
}

/// The representative marker colour for a light probe, derived from its baked SH. We
/// reconstruct diffuse irradiance toward world-up (`Sh9::eval(Vec3::Y)`) — the same eval
/// the shading path uses — so the tint reads as "what light is arriving from above" at
/// the probe. A zero/unbaked probe → black; a bright probe → bright; a warm/cool bounce
/// shows in the hue. The colour is tone-mapped (Reinhard) into `[0, 1]` so HDR
/// irradiance stays a legible tint instead of clipping to flat white; alpha is fixed.
pub(crate) fn sh_marker_tint(sh: &Sh9) -> [f32; 4] {
    let irradiance = sh.eval(Vec3::Y).max(Vec3::ZERO);
    let mapped = irradiance / (irradiance + Vec3::ONE);
    [mapped.x, mapped.y, mapped.z, 1.0]
}

/// Line-list vertices for a 3-axis cross marker centred at `center`, each arm extending
/// `half` along ±X, ±Y, ±Z. Three line segments → 6 vertices. Used for both light-probe
/// and reflection-probe capture markers.
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
