//! The frame's point and spot lights as one GPU array (#434), in the order every
//! camera's cluster lists index.

use glam::Vec3;

use crate::components::{LightComponent, TransformComponent};
use crate::scene::{LightType, Scene};

/// `LocalLight::kind` for a point light (no cone).
pub(crate) const KIND_POINT: u32 = 0;
/// `LocalLight::kind` for a spotlight (cone cosines apply).
pub(crate) const KIND_SPOT: u32 = 1;
/// `LocalLight::shadow` for a light with no tile in the shadow atlas (#468).
pub(crate) const NO_SHADOW: u32 = u32::MAX;

/// One point or spot light, mirroring `LocalLight` in `common.wgsl` byte-for-byte.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LocalLight {
    pub position: [f32; 3],
    /// Past this distance the light contributes nothing; also its binning radius.
    pub range: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    /// Where a spotlight points (unit); unused for a point light.
    pub direction: [f32; 3],
    pub kind: u32,
    /// Cosines of the spotlight's inner and outer cone half-angles.
    pub inner_cone: f32,
    pub outer_cone: f32,
    /// Its first tile in the shadow atlas (#468) — a spotlight's one, a point light's
    /// six cube faces in order — or [`NO_SHADOW`]. Set by the atlas plan each frame.
    pub shadow: u32,
    /// 1.0 for a `Baked` light (#438): lightmapped surfaces skip its direct light.
    pub baked: f32,
}

impl LocalLight {
    /// The light's record, or `None` for a type that is not a local light.
    pub(crate) fn new(transform: &TransformComponent, light: &LightComponent) -> Option<Self> {
        let kind = match light.light_type {
            LightType::Point => KIND_POINT,
            LightType::Spotlight => KIND_SPOT,
            LightType::Ambient | LightType::Directional => return None,
        };
        Some(Self {
            position: transform.position.to_array(),
            range: light.range.max(0.0),
            color: light.color.to_array(),
            intensity: light.intensity,
            direction: (transform.rotation * Vec3::NEG_Z).normalize().to_array(),
            kind,
            inner_cone: light.inner_cone.to_radians().cos(),
            outer_cone: light.outer_cone.to_radians().cos(),
            shadow: NO_SHADOW,
            baked: f32::from(u8::from(light.mode.bakes_direct())),
        })
    }

    /// The bounding sphere the binner tests: centre and range. A spotlight is
    /// bounded by its whole range sphere, which is conservative.
    pub(crate) fn sphere(&self) -> (Vec3, f32) {
        (Vec3::from(self.position), self.range)
    }
}

/// Every active point and spot light in the scene that the frame (or, with
/// `capture`, a bake capture) draws live (`LightMode::renders_live`, #809), in entity
/// order, each with whether it asks for a shadow (`LightComponent::cast_shadows`,
/// #468). The one place `Baked` local lights leave the realtime path.
pub(crate) fn local_lights(scene: &Scene, capture: bool) -> Vec<(LocalLight, bool)> {
    scene
        .world
        .ids_with_light()
        .into_iter()
        .filter(|&id| scene.world.is_active(id))
        .filter_map(|id| {
            let light = scene.world.light(id)?;
            if !light.mode.renders_live(capture) {
                return None;
            }
            let transform = scene.world.transform(id)?;
            Some((LocalLight::new(&transform, &light)?, light.cast_shadows))
        })
        .collect()
}
