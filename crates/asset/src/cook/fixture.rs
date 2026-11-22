//! A model exercising every cooking step: several sub-meshes without source tangents,
//! one past `clusters::MIN_TRIANGLES`, lightmap UVs ticked in its sidecar.

use std::fmt::Write;
use std::path::PathBuf;

use crate::lightmap_uv::LightmapUvSettings;

const GRID: usize = 64;

pub fn model(name: &str) -> PathBuf {
    let dir = rusty_test_support::temp::dir().join(format!("rusty_cook_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.obj");
    std::fs::write(&path, obj()).unwrap();
    let on = LightmapUvSettings {
        generate: true,
        ..LightmapUvSettings::default()
    };
    crate::sidecar::set_lightmap_uvs(&path, on).unwrap();
    let _ = std::fs::remove_file(cooked_file(&path));
    path
}

pub fn cooked_file(model: &std::path::Path) -> PathBuf {
    super::with_writes(true, || super::cooked_path(model)).expect("a temp path")
}

fn obj() -> String {
    let mut out = String::new();
    let mut base = 1;
    let side = GRID + 1;
    let bump = |x: usize, z: usize| ((x * 7 + z * 13) % 5) as f32 * 0.05;
    let grid = (0..side * side).map(|i| {
        let (x, z) = (i % side, i / side);
        (
            [x as f32, bump(x, z), z as f32],
            [x as f32 / 8.0, z as f32 / 8.0],
        )
    });
    let quads = (0..GRID * GRID).map(|q| {
        let (x, z) = (q % GRID, q / GRID);
        let i = z * side + x;
        [i, i + 1, i + side + 1, i + side]
    });
    write_object(&mut out, "Grid", grid.collect(), quads.collect(), &mut base);
    let corner = |i: usize| [(i & 1) as f32, (i >> 1 & 1) as f32, (i >> 2 & 1) as f32];
    let cube = (0..8).map(|i| (corner(i), [corner(i)[0], corner(i)[2]]));
    let faces = vec![
        [0, 1, 3, 2],
        [4, 6, 7, 5],
        [0, 4, 5, 1],
        [2, 3, 7, 6],
        [0, 2, 6, 4],
        [1, 5, 7, 3],
    ];
    write_object(&mut out, "Box", cube.collect(), faces, &mut base);
    let tri = vec![
        ([0.0, 0.0, 5.0], [0.0, 0.0]),
        ([1.0, 0.0, 5.0], [1.0, 0.0]),
        ([0.0, 1.0, 5.0], [0.0, 1.0]),
    ];
    let _ = writeln!(out, "o Tri");
    for (p, uv) in &tri {
        let _ = writeln!(
            out,
            "v {} {} {}\nvt {} {}\nvn 0 0 1",
            p[0], p[1], p[2], uv[0], uv[1]
        );
    }
    let _ = writeln!(
        out,
        "f {0}/{0}/{0} {1}/{1}/{1} {2}/{2}/{2}",
        base,
        base + 1,
        base + 2
    );
    out
}

fn write_object(
    out: &mut String,
    name: &str,
    corners: Vec<([f32; 3], [f32; 2])>,
    quads: Vec<[usize; 4]>,
    base: &mut usize,
) {
    let _ = writeln!(out, "o {name}");
    for (p, uv) in &corners {
        let n = [p[1] * 0.3, 1.0, p[0] * 0.1];
        let _ = writeln!(out, "v {} {} {}\nvt {} {}", p[0], p[1], p[2], uv[0], uv[1]);
        let _ = writeln!(out, "vn {} {} {}", n[0], n[1], n[2]);
    }
    for q in quads {
        let at: Vec<String> = q.iter().map(|i| (i + *base).to_string()).collect();
        let corner = |k: usize| format!("{0}/{0}/{0}", at[k]);
        let _ = writeln!(
            out,
            "f {} {} {} {}",
            corner(0),
            corner(1),
            corner(2),
            corner(3)
        );
    }
    *base += corners.len();
}
