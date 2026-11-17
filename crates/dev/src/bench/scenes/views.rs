//! Where `make bench-scenes` looks from: deterministic viewpoints no one authors.
//!
//! In order: every active screen camera the scene holds, at its own transform;
//! the player's spawn (the entity named `PlayerSpawn`, else `Player`), one view
//! along its facing; then points on the navmesh at eye height, each looking along
//! four yaws (0, 90, 180 and 270°), picked from a fixed seed until the cameras,
//! the spawn and the points make [`MAX_POINTS`]. A scene with no navmesh uses its
//! cameras alone; one with neither looks along +Z from its bounds' centre.

use glam::Vec3;

use rusty_navigation::{NavigationGraph, NOT_WALKABLE_AREA};
use rusty_scene::camera::{active_cameras, set_view_orientation, set_view_position};
use rusty_scene::Scene;

/// Eye height above the floor, metres.
pub const EYE_HEIGHT: f32 = 1.7;
/// The most points (cameras, the spawn, navmesh points) a scene is benched from.
pub const MAX_POINTS: usize = 8;
/// The yaws each navmesh point looks along, degrees.
pub const YAWS: [f32; 4] = [0.0, 90.0, 180.0, 270.0];
/// The navmesh points' seed: the same scene gives the same points on every run.
const SEED: u64 = 0x2294_BE7C_5CE7_E5ED;

/// Where one viewpoint looks from.
#[derive(Clone, Debug, PartialEq)]
pub enum View {
    /// A scene camera, as the only screen camera.
    Camera(u32),
    /// The main view moved here (yaw and pitch in degrees, `Camera`'s convention).
    Pose {
        position: Vec3,
        yaw: f32,
        pitch: f32,
    },
}

/// One viewpoint and the label its rows name it by.
#[derive(Clone, Debug, PartialEq)]
pub struct Viewpoint {
    pub label: String,
    pub view: View,
}

/// A scene's viewpoints, and what the report should say about how they were picked.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Views {
    pub views: Vec<Viewpoint>,
    pub warnings: Vec<String>,
}

/// The viewpoints for `scene` with its navmesh `nav`: see the module docs.
pub fn pick(scene: &Scene, nav: &NavigationGraph) -> Views {
    let mut out = Views::default();
    let cameras: Vec<u32> = active_cameras(scene)
        .filter(|(_, target)| target.is_none())
        .map(|(id, _)| id)
        .collect();
    for &id in &cameras {
        let name = scene.world.name(id).map_or(String::new(), |n| n.clone());
        out.views.push(Viewpoint {
            label: format!("camera {name}"),
            view: View::Camera(id),
        });
    }
    let spawn = spawn_view(scene, nav);
    let points = cameras.len() + usize::from(spawn.is_some());
    out.views.extend(spawn);
    let floors = walkable(nav);
    if floors.is_empty() {
        no_floor(&mut out, cameras.is_empty(), scene);
        return out;
    }
    for (n, floor) in seeded(&floors, MAX_POINTS.saturating_sub(points))
        .into_iter()
        .enumerate()
    {
        for yaw in YAWS {
            out.views.push(Viewpoint {
                label: format!("nav {n} yaw {yaw}"),
                view: View::Pose {
                    position: floor + Vec3::Y * EYE_HEIGHT,
                    yaw,
                    pitch: 0.0,
                },
            });
        }
    }
    out
}

/// A scene without a navmesh: its cameras alone, or, without a camera either,
/// one view from its bounds' centre.
fn no_floor(out: &mut Views, no_camera: bool, scene: &Scene) {
    if !no_camera {
        out.warnings
            .push("no navmesh: the scene's cameras are its only viewpoints".into());
        return;
    }
    out.warnings
        .push("no camera and no navmesh: one view from the scene bounds' centre, along +Z".into());
    out.views.push(Viewpoint {
        label: "bounds centre".into(),
        view: View::Pose {
            position: bounds_centre(scene),
            yaw: 90.0,
            pitch: 0.0,
        },
    });
}

/// The spawn's view: eye height over the floor under it (its own height without
/// a floor there), along the way it faces.
fn spawn_view(scene: &Scene, nav: &NavigationGraph) -> Option<Viewpoint> {
    let id = ["PlayerSpawn", "Player"]
        .iter()
        .find_map(|name| scene.find_entity_by_name(name))?;
    let at = scene.world.world_position(id);
    let floor = nav.span_under(at).map(|s| nav.span_world(s).y);
    let position = Vec3::new(at.x, floor.map_or(at.y, |y| y + EYE_HEIGHT), at.z);
    let forward = scene.world.world_rotation(id) * Vec3::NEG_Z;
    let yaw = forward.z.atan2(forward.x).to_degrees();
    Some(Viewpoint {
        label: "spawn".into(),
        view: View::Pose {
            position,
            yaw,
            pitch: 0.0,
        },
    })
}

/// Every walkable floor's point, in the graph's span order.
fn walkable(nav: &NavigationGraph) -> Vec<Vec3> {
    nav.span_refs()
        .filter(|s| nav.spans[s.index as usize].area != NOT_WALKABLE_AREA)
        .map(|s| nav.span_world(s))
        .collect()
}

/// Up to `n` distinct entries of `floors`, drawn from [`SEED`] (SplitMix64).
fn seeded(floors: &[Vec3], n: usize) -> Vec<Vec3> {
    let mut state = SEED;
    let mut picked: Vec<usize> = Vec::new();
    while picked.len() < n.min(floors.len()) {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        let at = ((z ^ (z >> 31)) % floors.len() as u64) as usize;
        if !picked.contains(&at) {
            picked.push(at);
        }
    }
    picked.into_iter().map(|i| floors[i]).collect()
}

/// The centre of every entity's world position (the origin in an empty scene).
fn bounds_centre(scene: &Scene) -> Vec3 {
    let positions = scene
        .world
        .ids()
        .iter()
        .map(|&id| scene.world.world_position(id));
    let (min, max) = positions.fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)));
    if min.x > max.x {
        Vec3::ZERO
    } else {
        (min + max) * 0.5
    }
}

/// Make `view` the one the next frame draws. A camera view turns every other
/// screen camera off and returns them, for [`restore`]; a pose moves the main view.
pub fn apply(scene: &mut Scene, camera: &mut rusty_scene::Camera, view: &View) -> Vec<u32> {
    match view {
        View::Camera(id) => {
            let others: Vec<u32> = active_cameras(scene)
                .filter(|(other, target)| target.is_none() && other != id)
                .map(|(other, _)| other)
                .collect();
            for &other in &others {
                if let Some(mut c) = scene.world.camera_mut(other) {
                    c.active = false;
                }
            }
            others
        }
        View::Pose {
            position,
            yaw,
            pitch,
        } => {
            set_view_position(camera, scene, *position);
            set_view_orientation(camera, scene, *yaw, *pitch);
            Vec::new()
        }
    }
}

/// Turn back on the cameras [`apply`] turned off.
pub fn restore(scene: &mut Scene, turned_off: &[u32]) {
    for &id in turned_off {
        if let Some(mut c) = scene.world.camera_mut(id) {
            c.active = true;
        }
    }
}

#[cfg(test)]
#[path = "views_tests.rs"]
mod tests;
