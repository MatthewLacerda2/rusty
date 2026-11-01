//! The lights the lightmap bake sees (#438): each one's shape and falloff, and how
//! [`BakeScene::gather`] picks them out of a [`Scene`].

use glam::Vec3;

pub(in crate::scene::lighting::lightmap) mod area;

use super::input::BakeScene;
use crate::components::LightType;
use crate::scene::Scene;
use area::AreaShape;

/// Where a baked light sits and how it falls off — the forward shader's models.
#[derive(Clone, Copy, Debug)]
pub enum LightShape {
    /// Light travelling along `direction` (unit).
    Directional {
        direction: Vec3,
    },
    Point {
        position: Vec3,
        range: f32,
    },
    Spot {
        position: Vec3,
        direction: Vec3,
        range: f32,
        cos_inner: f32,
        cos_outer: f32,
    },
    /// An Area (Rectangle) light (#1674): see [`area`].
    Area(super::area::AreaShape),
}

/// A light the bake includes: a `Mixed` one (bounce only) or a `Baked` one (direct and
/// bounce). `Realtime` lights never get here.
#[derive(Clone, Copy, Debug)]
pub struct BakeLight {
    pub shape: LightShape,
    /// Colour times intensity (per unit area for an Area light).
    pub radiance: Vec3,
    /// Whether the receiving texel's own direct light is baked (`Baked`).
    pub bakes_direct: bool,
}

impl BakeScene {
    /// The active lights that take part in the bake (an `Ambient` light is in the
    /// sky already, through `Scene::drawn_ambient`).
    pub(super) fn gather_lights(&mut self, scene: &Scene) {
        for id in scene.world.ids_with_light() {
            if !scene.world.is_active_in_hierarchy(id) {
                continue;
            }
            let light = scene.world.light(id).expect("id came from ids_with_light");
            // World space: a light on a child bakes where it is drawn (#1196).
            let position = scene.world.world_position(id);
            let forward = scene
                .world
                .transform_direction(id, Vec3::NEG_Z)
                .try_normalize()
                .unwrap_or(Vec3::NEG_Y);
            let shape = match light.light_type {
                // The sky is `Scene::drawn_ambient`'s, which an Ambient light sets.
                LightType::Ambient => continue,
                LightType::Directional => LightShape::Directional { direction: forward },
                LightType::Point => LightShape::Point {
                    position,
                    range: light.range,
                },
                LightType::Spotlight => LightShape::Spot {
                    position,
                    direction: forward,
                    range: light.range,
                    cos_inner: light.inner_cone.to_radians().cos(),
                    cos_outer: light.outer_cone.to_radians().cos(),
                },
                LightType::Area => {
                    let axis = |local| {
                        scene
                            .world
                            .transform_direction(id, local)
                            .normalize_or_zero()
                    };
                    LightShape::Area(AreaShape {
                        center: position,
                        right: axis(Vec3::X) * light.area_size.x,
                        up: axis(Vec3::Y) * light.area_size.y,
                        forward,
                        range: light.range,
                    })
                }
            };
            let mode = light.effective_mode();
            if mode.bakes_indirect() {
                self.lights.push(BakeLight {
                    shape,
                    radiance: light.color * light.intensity,
                    bakes_direct: mode.bakes_direct(),
                });
            }
        }
    }
}
