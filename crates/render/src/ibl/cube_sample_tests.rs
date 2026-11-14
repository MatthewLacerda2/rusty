//! Tests for the CPU cubemap direction <-> texel mapping (#245). Sibling of
//! `cube_sample.rs`; shares its 300-line source cap. Pure math — no GPU needed.

use glam::Vec3;

use super::*;

const COLORS: [Vec3; 6] = [
    Vec3::new(1.0, 0.0, 0.0), // PosX red
    Vec3::new(0.0, 1.0, 0.0), // NegX green
    Vec3::new(0.0, 0.0, 1.0), // PosY blue
    Vec3::new(1.0, 1.0, 0.0), // NegY yellow
    Vec3::new(1.0, 0.0, 1.0), // PosZ magenta
    Vec3::new(0.0, 1.0, 1.0), // NegZ cyan
];

/// Six faces, each a solid distinct colour scaled by `scale`, so a sampled
/// direction's colour tells us which face it hit.
fn colored_faces(res: u32, scale: f32) -> [Vec<Vec3>; 6] {
    std::array::from_fn(|f| vec![COLORS[f] * scale; (res * res) as usize])
}

/// A face's centre direction samples that face's own colour: `direction` and
/// `locate` round-trip.
#[test]
fn direction_round_trips_through_locate() {
    let res = 4;
    let proj = FaceProjections::new();
    let faces = colored_faces(res, 1.0);
    for face in CubemapFace::ALL {
        let dir = proj.direction(face, 0.0, 0.0);
        assert_eq!(sample(&proj, &faces, res, dir), COLORS[face as usize]);
    }
}

/// A fractional level blends the two levels around it, and a level past the chain
/// reads its last.
#[test]
fn sample_lod_blends_the_levels_around_it() {
    let proj = FaceProjections::new();
    let chain = [colored_faces(2, 1.0), colored_faces(1, 3.0)];
    let dir = proj.direction(CubemapFace::PosX, 0.0, 0.0);
    let at = |lod| sample_lod(&proj, &chain, 2, dir, lod).x;
    assert_eq!((at(0.0), at(0.5), at(1.0), at(4.0)), (1.0, 2.0, 3.0, 3.0));
}

/// The zero vector locates nowhere and samples black, never panics.
#[test]
fn zero_direction_is_black() {
    let proj = FaceProjections::new();
    let faces = colored_faces(2, 1.0);
    assert_eq!(sample(&proj, &faces, 2, Vec3::ZERO), Vec3::ZERO);
}

/// `direction` lays every face out as a `texture_cube` does (#1615): the D3D / Vulkan /
/// wgpu table `sky/prefilter.wgsl`'s `face_direction` writes, with `(s, t)` the
/// texel's coords, `t` growing down the rows. A face mirrored in-plane, or turned,
/// fails at the corners.
#[test]
fn direction_follows_the_texture_cube_layout() {
    let hardware = |face: CubemapFace, s: f32, t: f32| match face {
        CubemapFace::PosX => glam::Vec3::new(1.0, -t, -s),
        CubemapFace::NegX => glam::Vec3::new(-1.0, -t, s),
        CubemapFace::PosY => glam::Vec3::new(s, 1.0, t),
        CubemapFace::NegY => glam::Vec3::new(s, -1.0, -t),
        CubemapFace::PosZ => glam::Vec3::new(s, -t, 1.0),
        CubemapFace::NegZ => glam::Vec3::new(-s, -t, -1.0),
    };
    let proj = FaceProjections::new();
    for face in CubemapFace::ALL {
        for (s, t) in [(-0.8, -0.6), (0.7, -0.5), (-0.4, 0.9), (0.6, 0.3)] {
            // NDC v is up, the texel's t down.
            let got = proj.direction(face, s, -t);
            let want = hardware(face, s, t).normalize();
            assert!(
                got.distance(want) < 1e-4,
                "{face:?} ({s}, {t}): {got} vs {want}"
            );
        }
    }
}
