use glam::Vec3;

use super::cone::{Bounds, Cone, WIDEST_CULLED_CONE};

use rusty_components::LightComponent;
use rusty_scene::{LightType, Scene};

pub(crate) const KIND_POINT: u32 = 0;
pub(crate) const KIND_SPOT: u32 = 1;
pub(crate) const KIND_AREA: u32 = 2;
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
    pub inner_cone: f32,
    pub outer_cone: f32,
    pub shadow: u32,
    pub baked: f32,
    pub shadow_fade: f32,
    pub near: f32,
    pub bounce_baked: f32,
    pub mask_channel: f32,
}

impl LocalLight {
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
            baked: 0.0,
            shadow_fade: Some(light.shadow_fade_distance)
                .filter(|d| d.is_finite() && *d > 0.0)
                .unwrap_or(0.0),
            near: if near.is_finite() { near } else { 0.0 },
            bounce_baked: 0.0,
            mask_channel: -1.0,
        })
    }

    pub(crate) fn brightness(&self) -> f32 {
        self.intensity * rusty_core::color::luminance(self.color)
    }

    pub(crate) fn one_frustum(&self) -> bool {
        self.kind != KIND_POINT
    }

    /// The light's range sphere: centre and range. What every per-camera cut
    /// (the binner's and the shadow atlas's) ranks by.
    pub(crate) fn sphere(&self) -> (Vec3, f32) {
        (Vec3::from(self.position), self.range)
    }

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

pub(crate) fn local_lights(scene: &Scene, capture: bool) -> Vec<(LocalLight, bool)> {
    scene
        .world
        .ids_with_light()
        .into_iter()
        .filter(|&id| scene.world.is_active_in_hierarchy(id))
        .filter_map(|id| {
            let light = scene.world.light(id)?;
            let mode = crate::clusters::drawn_mode(scene, id, &light);
            if !mode.renders_live(capture) {
                return None;
            }
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
