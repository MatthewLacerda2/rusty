//! Trail and Line ribbons render (#441): a line across the view covers the centre
//! pixel in its colour, alpha and additive; it is hidden behind a solid (depth
//! tested); local points follow the entity; a recorded trail draws; and under
//! fog that swallows the wall, the ribbon vanishes into the fog colour exactly as
//! the wall behind it does (#437). Reuses the fog tests' flat black wall.

use glam::Vec3;
use rusty::components::{LineComponent, ParticleBlend, RibbonStyle, TrailComponent};
use rusty::dev::capture::CaptureHost;
use rusty::scene::{FogMode, Scene};

use super::fog_scene::{assert_close, centre, srgb8, wall_scene};

const DISTANCE: f32 = 20.0;
const RED: [f32; 4] = [1.0, 0.2, 0.2, 1.0];

/// Half a metre in front of the wall, on the view axis.
fn front() -> f32 {
    5.0 - DISTANCE + 0.5
}

/// The wall with one horizontal ribbon through the view axis at depth `z`.
fn with_line(z: f32, blend: ParticleBlend, edit: impl FnOnce(&mut LineComponent)) -> Scene {
    let mut scene = wall_scene(DISTANCE, [0.0; 3]);
    let id = scene.add_entity("Line".to_string());
    let mut line = LineComponent {
        positions: vec![Vec3::new(-30.0, 0.0, z), Vec3::new(30.0, 0.0, z)],
        style: RibbonStyle {
            blend,
            ..RibbonStyle::solid(4.0, RED)
        },
        ..Default::default()
    };
    edit(&mut line);
    scene.world.set_line(id, Some(line));
    scene
}

fn fogged(mut scene: Scene) -> Scene {
    scene.fog.mode = FogMode::Linear;
    scene.fog.color = Vec3::new(0.2, 0.4, 0.9);
    scene.fog.start = 0.0;
    scene.fog.end = 1.0;
    scene
}

#[test]
fn a_line_covers_the_centre_in_its_colour_and_hides_behind_solids() {
    let mut host = CaptureHost::new();
    let Some(alpha) = centre(
        &mut host,
        &with_line(front(), ParticleBlend::Alpha, |_| ()),
        "rib_alpha",
    ) else {
        return;
    };
    assert_close(alpha, [srgb8(1.0), srgb8(0.2), srgb8(0.2)], 3, "alpha line");
    let add = centre(
        &mut host,
        &with_line(front(), ParticleBlend::Additive, |_| ()),
        "rib_add",
    )
    .unwrap();
    assert_close(
        add,
        [srgb8(1.0), srgb8(0.2), srgb8(0.2)],
        3,
        "additive over black",
    );
    let behind = with_line(5.0 - DISTANCE - 2.0, ParticleBlend::Alpha, |_| ());
    let hidden = centre(&mut host, &behind, "rib_behind").unwrap();
    assert_close(hidden, [0, 0, 0], 2, "behind the wall");
}

#[test]
fn local_points_follow_the_entity_and_a_trail_draws() {
    let mut host = CaptureHost::new();
    // Local space on an entity lifted 6 m: the line leaves the view axis.
    let mut lifted = with_line(front(), ParticleBlend::Alpha, |l| l.use_world_space = false);
    let id = lifted.world.ids_with_line()[0];
    lifted.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 6.0, 0.0);
    let Some(moved) = centre(&mut host, &lifted, "rib_local") else {
        return;
    };
    assert_close(moved, [0, 0, 0], 2, "local line moved off-centre");

    let mut scene = wall_scene(DISTANCE, [0.0; 3]);
    let id = scene.add_entity("Tracer".to_string());
    let mut trail = TrailComponent {
        time: 10.0,
        min_vertex_distance: 0.0,
        style: RibbonStyle::solid(4.0, RED),
        ..Default::default()
    };
    for x in [-30.0, 0.0, 30.0] {
        trail.advance(Vec3::new(x, 0.0, front()), 0.1);
    }
    scene.world.set_trail(id, Some(trail));
    let drawn = centre(&mut host, &scene, "rib_trail").unwrap();
    assert_close(drawn, [srgb8(1.0), srgb8(0.2), srgb8(0.2)], 3, "trail");
}

#[test]
fn ribbons_fade_into_fog_like_the_wall_behind_them() {
    let mut host = CaptureHost::new();
    let Some(fog_only) = centre(
        &mut host,
        &fogged(wall_scene(DISTANCE, [0.0; 3])),
        "rib_fog",
    ) else {
        return;
    };
    let want = fog_only.map(i32::from);
    for blend in [ParticleBlend::Alpha, ParticleBlend::Additive] {
        let scene = fogged(with_line(front(), blend, |_| ()));
        let got = centre(&mut host, &scene, &format!("rib_fog_{blend:?}")).unwrap();
        assert_close(got, want, 2, &format!("{blend:?} line in fog"));
    }
}
