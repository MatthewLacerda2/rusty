use glam::{Mat4, Quat, Vec3};

use super::mesh::MeshTree;
use super::top::{face, pack, Packed, Placed};
use super::walk::Hit;
use crate::scene::MeshHandle;
use rusty_components::mesh::primitives::{generate_box, generate_plane};
use rusty_components::mesh::Vertex;

#[derive(Clone, Copy)]
pub(super) struct Object {
    pub mesh: usize,
    pub world: Mat4,
    pub face: u32,
}

/// Primitive geometry placed by hand, with where its rays start and how flat they run.
pub(super) struct Fixture {
    pub name: &'static str,
    pub meshes: Vec<(Vec<Vertex>, Vec<u32>)>,
    pub objects: Vec<Object>,
    pub eyes: (Vec3, Vec3),
    pub flat: f32,
}

fn object(mesh: usize, world: Mat4) -> Object {
    let face = face::BOTH;
    Object { mesh, world, face }
}

pub(super) fn trs(scale: Vec3, turn: Quat, at: Vec3) -> Mat4 {
    Mat4::from_scale_rotation_translation(scale, turn, at)
}

impl Fixture {
    pub(super) fn all() -> Vec<Self> {
        let cube = || vec![generate_box(1.0, 1.0, 1.0)];
        let step = |i: usize| {
            let at = Vec3::new(0.0, 0.25 * i as f32, 0.3 * i as f32);
            object(0, trs(Vec3::new(2.0, 0.25, 0.3), Quat::IDENTITY, at))
        };
        let mut seed = Seed(7);
        let mut spun = |i: usize| {
            let axis = Vec3::new(seed.next(), seed.next(), seed.next()) + Vec3::splat(1e-3);
            let at = Vec3::new((i % 4) as f32, ((i / 4) % 4) as f32, (i / 16) as f32) * 2.0;
            object(
                0,
                trs(
                    Vec3::splat(0.8),
                    Quat::from_axis_angle(axis.normalize(), 2.0),
                    at,
                ),
            )
        };
        let room = Self {
            name: "a box room",
            meshes: cube(),
            objects: vec![object(0, Mat4::from_scale(Vec3::new(10.0, 4.0, 10.0)))],
            eyes: (Vec3::new(-4.5, -1.8, -4.5), Vec3::new(4.5, 1.8, 4.5)),
            flat: 1.0,
        };
        let stair = Self {
            name: "a stair",
            meshes: cube(),
            objects: (0..8).map(step).collect(),
            eyes: (Vec3::new(-3.0, -1.0, -2.0), Vec3::new(3.0, 3.0, 4.0)),
            flat: 1.0,
        };
        let cubes = Self {
            name: "instanced cubes",
            objects: (0..64).map(&mut spun).collect(),
            meshes: cube(),
            eyes: (Vec3::splat(-2.0), Vec3::splat(8.0)),
            flat: 1.0,
        };
        let plane = Self {
            name: "a thin plane seen edge-on",
            meshes: vec![generate_plane(10.0, 10.0)],
            objects: vec![object(0, Mat4::IDENTITY)],
            eyes: (Vec3::new(-6.0, 0.001, -6.0), Vec3::new(6.0, 0.02, 6.0)),
            flat: 0.01,
        };
        vec![room, stair, cubes, plane]
    }

    pub(super) fn one(face: u32, world: Mat4) -> Self {
        Self {
            name: "one cube",
            meshes: vec![generate_box(1.0, 1.0, 1.0)],
            objects: vec![Object {
                mesh: 0,
                world,
                face,
            }],
            eyes: (Vec3::ZERO, Vec3::ZERO),
            flat: 1.0,
        }
    }

    pub(super) fn packed(&self) -> Packed {
        let trees: Vec<MeshTree> = self
            .meshes
            .iter()
            .map(|(v, i)| MeshTree::build(v, i, std::iter::once(0..i.len() as u32)))
            .collect();
        let placed: Vec<Placed> = (1..)
            .zip(&self.objects)
            .map(|(slot, o)| Placed {
                mesh: MeshHandle(o.mesh as u32),
                world: o.world,
                slot,
                faces: vec![o.face],
            })
            .collect();
        let meshes = trees.iter().enumerate();
        pack(
            meshes.map(|(i, t)| (MeshHandle(i as u32), t)),
            &placed,
            true,
        )
        .0
    }

    pub(super) fn world_tris(&self) -> Vec<[Vec3; 3]> {
        let mut tris = Vec::new();
        for o in &self.objects {
            let (vertices, indices) = &self.meshes[o.mesh];
            let at = |i: u32| {
                o.world
                    .transform_point3(Vec3::from_array(vertices[i as usize].position))
            };
            tris.extend(
                indices
                    .chunks_exact(3)
                    .map(|t| [at(t[0]), at(t[1]), at(t[2])]),
            );
        }
        tris
    }

    /// The world corners of the triangle `hit` names.
    pub(super) fn tri_of(&self, packed: &Packed, hit: Hit) -> [Vec3; 3] {
        let world = self.objects[hit.slot as usize - 1].world;
        packed.tris[hit.tri as usize]
            .corners()
            .map(|p| world.transform_point3(p))
    }
}

struct Seed(u64);

impl Seed {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// `n` seeded rays from inside `f.eyes`, each with a reach of 100.
pub(super) fn rays(f: &Fixture, n: usize) -> Vec<(Vec3, Vec3, f32)> {
    let mut s = Seed(0x9e37_79b9_7f4a_7c15);
    let (lo, hi) = f.eyes;
    (0..n)
        .map(|_| {
            let o = lo + (hi - lo) * Vec3::new(s.next(), s.next(), s.next());
            let d = Vec3::new(s.next(), s.next(), s.next()) * 2.0 - Vec3::ONE;
            let d = (d * Vec3::new(1.0, f.flat, 1.0)).normalize_or(Vec3::X);
            (o, d, 100.0)
        })
        .collect()
}
