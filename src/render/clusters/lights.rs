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
/// `LocalLight::kind` for an Area light's stand-in in a bake capture (#1674): a
/// spot shadowed as one, whose light fades as the Lambert `cos θ` off its facing
/// instead of a spotlight's squared cone (#1699).
pub(crate) const KIND_AREA: u32 = 2;
/// `LocalLight::shadow` for a light with no tile in the shadow atlas (#468).
pub(crate) const NO_SHADOW: u32 = u32::MAX;

/// One point or spot light, mirroring `LocalLight` in `common.wgsl` byte-for-byte.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LocalLight {
    pub position: [f32; 3],
    /// Past this distance the light contributes nothing; the per-camera cut ranks
    /// it by this sphere.
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
    /// The distance its inverse square is floored at (`components::falloff_near`,
    /// #1699): an Area light's stand-in's `sqrt(area / π)`, so a surface touching
    /// the panel gets its true limit of `π × intensity`, not one point's
    /// `intensity × area / ε²`; `0` for a point or spot light.
    pub near: f32,
    /// 1.0 when the scene's lightmap bake took this light in (#1750,
    /// `LightmapSet::baked_light`): SSIL gathers none of its light. Set by
    /// [`local_lights`], which knows the scene.
    pub bounce_baked: f32,
    /// Its shadowmask channel (#1695, `Scene::mask_channel`); `-1.0` for none.
    /// Set by [`local_lights`], which knows the scene.
    pub mask_channel: f32,
}

impl LocalLight {
    /// The light's record at world `position`, shining along world `forward`, or
    /// `None` for a type that is not a local light.
    ///
    /// An Area light (#1674) only ever gets here in a bake capture, being always
    /// `Baked`: it is drawn as its far field, a spot at its centre of intensity
    /// `intensity × area` ([`KIND_AREA`]) whose light fades as `cos θ` off its
    /// facing, a one-sided Lambertian emitter's falloff. The lightmap and the probes' direct term
    /// integrate the rectangle itself.
    pub(crate) fn new(position: Vec3, forward: Vec3, light: &LightComponent) -> Option<Self> {
        let (kind, intensity, cones, near) = match light.light_type {
            LightType::Point => (KIND_POINT, light.intensity, (0.0, 0.0), 0.0),
            LightType::Spotlight => (
                KIND_SPOT,
                light.intensity,
                (light.inner_cone, light.outer_cone),
                0.0,
            ),
            LightType::Area => {
                let area = (light.area_size.x * light.area_size.y).max(0.0);
                let near = (area / std::f32::consts::PI).sqrt();
                (KIND_AREA, light.intensity * area, (0.0, 90.0), near)
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
            near: if near.is_finite() { near } else { 0.0 },
            bounce_baked: 0.0,
            mask_channel: -1.0,
        })
    }

    /// How bright the light is, for ranking it against others (#834): intensity
    /// times its colour's Rec. 709 luminance.
    pub(crate) fn brightness(&self) -> f32 {
        self.intensity * crate::core::color::luminance(self.color)
    }

    /// Whether it shadows through one spot frustum (a spotlight or an Area
    /// light's stand-in) rather than a point light's six cube faces.
    pub(crate) fn one_frustum(&self) -> bool {
        self.kind != KIND_POINT
    }

    /// The light's range sphere: centre and range. What every per-camera cut
    /// (the binner's and the shadow atlas's) ranks by.
    pub(crate) fn sphere(&self) -> (Vec3, f32) {
        (Vec3::from(self.position), self.range)
    }

    /// What the binner tests (#1712): the range [`Self::sphere`], and for a
    /// spotlight its [`Cone`], which decides which clusters list it once kept. A
    /// cone wider than [`WIDEST_CULLED_CONE`], and an Area light's 90° stand-in,
    /// get no cone and list by their range sphere alone.
    pub(crate) fn bounds(&self) -> Bounds {
        let (apex, range) = self.sphere();
        let axis = Vec3::from(self.direction);
        let cos = self.outer_cone;
        let culled = self.kind == KIND_SPOT
            && cos >= WIDEST_CULLED_CONE.to_radians().cos()
            && cos <= 1.0
            && axis.is_finite();
        let cone = culled.then(|| Cone {
            apex,
            axis,
            range,
            cos,
            sin: (1.0 - cos * cos).max(0.0).sqrt(),
        });
        Bounds {
            sphere: (apex, range),
            cone,
        }
    }
}

/// Every active point and spot light in the scene that the frame (or, with
/// `capture`, a bake capture) draws live (`LightMode::renders_live`, #809), in entity
/// order, each with whether it asks for a shadow (`LightComponent::cast_shadows`,
/// #468). The one place `Baked` local lights leave the realtime path. A capture
/// takes no shadow fade (#1712): its camera is a probe, not the player, so a
/// fade measured from it would bake a lamp unshadowed through walls for good.
pub(crate) fn local_lights(scene: &Scene, capture: bool) -> Vec<(LocalLight, bool)> {
    scene
        .world
        .ids_with_light()
        .into_iter()
        .filter(|&id| scene.world.is_active_in_hierarchy(id))
        .filter_map(|id| {
            let light = scene.world.light(id)?;
            let mode = scene.light_mode(id, &light);
            if !mode.renders_live(capture) {
                return None;
            }
            // World space: a light on a child (a flashlight in a hand) lights
            // where it is drawn (#1196).
            let (at, forward) = (
                scene.world.world_position(id),
                scene.world.transform_direction(id, Vec3::NEG_Z),
            );
            let mut local = LocalLight::new(at, forward, &light)?;
            local.baked = f32::from(u8::from(mode.bakes_direct()));
            if capture {
                local.shadow_fade = 0.0;
            }
            local.bounce_baked = f32::from(u8::from(scene.lightmaps.baked_light(id)));
            local.mask_channel = scene.mask_channel(id).map_or(-1.0, f32::from);
            Some((local, light.cast_shadows))
        })
        .collect()
}
