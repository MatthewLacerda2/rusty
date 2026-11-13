//! A tessellated wall's openings stay open when its faces fuse by their union
//! (#1981): one mesh, as an exporter writes it, whose front and back faces
//! share corners with a window and a door, off round coordinates. Every cell
//! sees what it saw with each face filling alone, and the objects seen through
//! the window and past the door's jamb stay seen from the cells that see them.

use glam::Vec3;

use super::bench::shift;
use super::bench::town::authored::entity;
use super::gather_occlusion;
use super::sample::without_union;
use super::test_level::block;
use crate::Scene;

/// The wall's grid lines, the window's and the door's edges among them.
const XS: [f32; 13] = [
    -6.3, -5.03, -4.11, -3.3, -2.1, -1.07, 0.31, 1.23, 2.05, 3.15, 4.43, 5.29, 6.3,
];
const YS: [f32; 6] = [0.0, 0.71, 1.1, 2.2, 2.3, 3.1];
const WINDOW: ((f32, f32), (f32, f32)) = ((-3.3, -2.1), (1.1, 2.2));
const DOOR: ((f32, f32), (f32, f32)) = ((2.05, 3.15), (0.0, 2.3));
/// The wall's half thickness: its faces lie at z = ±HALF.
const HALF: f32 = 0.1;
/// Where the level is moved to, and by how much it is scaled.
const ORIGIN: Vec3 = Vec3::new(0.137, 0.053, -0.211);
const SCALE: f32 = 1.07;

/// The level: the wall, a box behind its window, one behind the wall past the
/// door's jamb, and a marker in front; the ids of the two boxes behind.
fn level() -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let open = |x: (f32, f32), y: (f32, f32)| {
        [WINDOW, DOOR]
            .iter()
            .any(|&(a, b)| x.0 >= a.0 && x.1 <= a.1 && y.0 >= b.0 && y.1 <= b.1)
    };
    let mut quads = Vec::new();
    for x in XS.windows(2) {
        for y in YS.windows(2) {
            if open((x[0], x[1]), (y[0], y[1])) {
                continue;
            }
            let p = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
            let (a, b, c, d) = (x[0], x[1], y[0], y[1]);
            quads.push([p(a, c, HALF), p(b, c, HALF), p(b, d, HALF), p(a, d, HALF)]);
            quads.push([
                p(a, d, -HALF),
                p(b, d, -HALF),
                p(b, c, -HALF),
                p(a, c, -HALF),
            ]);
        }
    }
    entity(&mut scene, "Wall", &quads);
    let window = block(
        &mut scene,
        "Window",
        Vec3::new(-2.93, 1.43, -5.37),
        Vec3::new(-2.47, 1.89, -4.91),
    );
    let jamb = block(
        &mut scene,
        "Jamb",
        Vec3::new(3.41, 0.63, -1.73),
        Vec3::new(3.87, 1.09, -1.27),
    );
    let front = (Vec3::new(-0.23, 0.41, 4.61), Vec3::new(0.19, 0.83, 5.03));
    block(&mut scene, "Marker", front.0, front.1);
    shift(&mut scene, ORIGIN, SCALE);
    (scene, window, jamb)
}

#[test]
fn a_tessellated_walls_window_and_door_stay_open() {
    let (scene, window, jamb) = level();
    let input = gather_occlusion(&scene);
    let fused = input.sampler().expect("a grid");
    let alone = without_union(|| input.sampler()).expect("a grid");
    let count = fused.grid.cell_count();
    let on: Vec<Vec<u64>> = (0..count).map(|i| fused.cell(i)).collect();
    let off: Vec<Vec<u64>> = (0..count).map(|i| alone.cell(i)).collect();
    assert!(
        fused.rays_cast() < alone.rays_cast(),
        "the union spares rays: {} against {}",
        fused.rays_cast(),
        alone.rays_cast()
    );
    for i in 0..count {
        assert_eq!(on[i], off[i], "cell {i} sees what it did with faces alone");
    }
    // Cells wholly in front of the wall.
    let wall = ORIGIN.z + HALF * SCALE;
    let front = (0..count).filter(|&i| fused.grid.bounds(i).0.z > wall);
    let front: Vec<usize> = front.collect();
    for (name, id) in [("window", window), ("jamb", jamb)] {
        let k = input.object(id).expect("a baked object");
        let sees = |i: &usize| on[*i][k / 64] & (1 << (k % 64)) != 0;
        assert!(
            front.iter().any(sees),
            "some front cell sees the {name} box"
        );
        assert!(
            !front.iter().all(sees),
            "the wall hides the {name} box from some front cell"
        );
    }
}
