//! The frame's point and spot lights as one GPU array (#434), in the order every
//! camera's cluster lists index.

use glam::Vec3;

use super::cone::{Bounds, Cone, WIDEST_CULLED_CONE};

use crate::components::LightComponent;
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
    /// The camera distance past which its shadow is not sampled (#1712,
    /// `LightComponent::shadow_fade_distance`); `0` never fades.
    pub shadow_fade: f32,
    pub _pad: [f32; 3],
}

impl LocalLight {
    /// The light's record at world `position`, shining along world `forward`, or
    /// `None` for a type that is not a local light.
    ///
    /// An Area light (#1674) only ever gets here in a bake capture, being always
    /// `Baked`: it is drawn as its far field, a spot at its centre of intensity
    /// `intensity × area` whose 0°–90° cone fades as `cos θ`, a one-sided
    /// Lambertian emitter's falloff. The lightmap and the probes' direct term
    /// integrate the rectangle itself.
    pub(crate) fn new(position: Vec3, forward: Vec3, light: &LightComponent) -> Option<Self> {
        let (kind, intensity, cones) = match light.light_type {
            LightType::Point => (KIND_POINT, light.intensity, (0.0, 0.0)),
            LightType::Spotlight => (
                KIND_SPOT,
                light.intensity,
                (light.inner_cone, light.outer_cone),
            ),
            LightType::Area => {
                let area = light.area_size.x * light.area_size.y;
                (KIND_SPOT, light.intensity * area, (0.0, 90.0))
            }
            LightType::Ambient | LightType::Directional => return None,
        };
        Some(Self {
            position: position.to_array(),
            range: light.range.max(0.0),
            color: light.color.to_array(),
            intensity,
            direction: forward.normalize().to_array(),
            kind,
            inner_cone: f32::to_radians(cones.0).cos(),
            outer_cone: f32::to_radians(cones.1).cos(),
            shadow: NO_SHADOW,
            baked: f32::from(u8::from(light.effective_mode().bakes_direct())),
            shadow_fade: Some(light.shadow_fade_distance)
                .filter(|d| d.is_finite() && *d > 0.0)
                .unwrap_or(0.0),
            _pad: [0.0; 3],
        })
    }

    /// How bright the light is, for ranking it against others (#834): intensity
    /// times its colour's Rec. 709 luminance.
    pub(crate) fn brightness(&self) -> f32 {
        self.intensity * crate::core::color::luminance(self.color)
    }

    /// Whether the light delivers anything (#1712): a positive brightness. A light
    /// at intensity 0 (a blinking alarm in its off half), black, or with a NaN or
    /// negative intensity is never binned, so no pixel loops over it.
    pub(crate) fn contributes(&self) -> bool {
        self.brightness() > 0.0
    }

    /// The light's range sphere: centre and range. What the shadow atlas ranks by;
    /// the binner tests the tighter [`Self::bounds`].
    pub(crate) fn sphere(&self) -> (Vec3, f32) {
        (Vec3::from(self.position), self.range)
    }

    /// What the binner tests (#1712): a bounding sphere (centre, radius), and for a
    /// spotlight its [`Cone`]. A point light's sphere is its range. A spot's is the
    /// smallest sphere around its range-capped cone (or its range sphere, if
    /// smaller), which a narrow beam fills far
    /// better than its range sphere; a cone wider than [`WIDEST_CULLED_CONE`] (an
    /// Area light's 90° stand-in among them) keeps its range sphere and no cone.
    pub(crate) fn bounds(&self) -> Bounds {
        let (at, range) = (Vec3::from(self.position), self.range);
        let axis = Vec3::from(self.direction);
        let cos = self.outer_cone;
        let culled = self.kind == KIND_SPOT
            && cos >= WIDEST_CULLED_CONE.to_radians().cos()
            && cos <= 1.0
            && axis.is_finite();
        if !culled {
            return Bounds {
                sphere: (at, range),
                cone: None,
            };
        }
        let sin = (1.0 - cos * cos).max(0.0).sqrt();
        // Up to 45° the sphere through the apex and the rim; past it, the one
        // around the rim circle (which then holds the apex and the cap too).
        let sphere = if cos >= std::f32::consts::FRAC_1_SQRT_2 {
            let r = range / (2.0 * cos);
            (at + axis * r, r)
        } else {
            (at + axis * range * cos, range * sin)
        };
        // Both spheres hold the cone; keep the smaller (the binner also tests the
        // range sphere per cluster, `Cone::may_reach_box`).
        let sphere = if sphere.1 < range {
            sphere
        } else {
            (at, range)
        };
        Bounds {
            sphere,
            cone: Some(Cone {
                apex: at,
                axis,
                range,
                cos,
                sin,
            }),
        }
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
        .filter(|&id| scene.world.is_active_in_hierarchy(id))
        .filter_map(|id| {
            let light = scene.world.light(id)?;
            if !light.effective_mode().renders_live(capture) {
                return None;
            }
            // World space: a light on a child (a flashlight in a hand) lights
            // where it is drawn (#1196).
            let (at, forward) = (
                scene.world.world_position(id),
                scene.world.transform_direction(id, Vec3::NEG_Z),
            );
            Some((LocalLight::new(at, forward, &light)?, light.cast_shadows))
        })
        .collect()
}
