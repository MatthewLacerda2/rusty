use super::*;
use image::{Rgb, RgbImage};

const RES: u32 = 256;

fn checker_png() -> String {
    let path = rusty_test_support::temp::dir().join("plane_checker.png");
    let image = RgbImage::from_fn(64, 64, |x, y| {
        if (x / 32 + y / 32) % 2 == 0 {
            Rgb([230, 20, 20])
        } else {
            Rgb([20, 230, 20])
        }
    });
    image.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

fn grooves_png() -> String {
    let path = rusty_test_support::temp::dir().join("plane_grooves.png");
    let (s, c) = 35f32.to_radians().sin_cos();
    let byte = |v: f32| ((v * 0.5 + 0.5) * 255.0).round() as u8;
    let image = RgbImage::from_fn(64, 64, |x, _| {
        let lean = if (x / 8) % 2 == 0 { s } else { -s };
        Rgb([byte(lean), 128, byte(c)])
    });
    image.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

fn render(
    material: &MaterialAsset,
    tag: &str,
    mesh: PreviewMesh,
    repeats: Option<u32>,
) -> Option<RgbImage> {
    let materials = BTreeMap::from([("m".to_string(), material.clone())]);
    let out = rusty_test_support::temp::dir().join(format!("2186_{tag}_{mesh}_{repeats:?}.png"));
    let options = PreviewOptions {
        mesh,
        resolution: RES,
        repeats,
    };
    let written = capture_material(&materials, "m", &out, options).expect("preview must not error");
    if !written {
        eprintln!("no GPU adapter — skipping the render half of this test");
        return None;
    }
    Some(image::open(&out).unwrap().to_rgb8())
}

fn cell(p: &Rgb<u8>) -> Option<u8> {
    let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
    (r > g + 24 && r > b + 24)
        .then_some(0)
        .or((g > r + 24 && g > b + 24).then_some(1))
}

fn cells_across_far_edge(image: &RgbImage) -> usize {
    let covered = |y| {
        (0..RES)
            .filter(|&x| cell(image.get_pixel(x, y)).is_some())
            .count()
    };
    let far = (0..RES)
        .find(|&y| covered(y) > RES as usize / 2)
        .expect("the plane is in shot");
    let row = (0..RES).filter_map(|x| cell(image.get_pixel(x, far + 4)));
    let mut runs = 0;
    let mut last = None;
    for c in row {
        if last != Some(c) {
            runs += 1;
            last = Some(c);
        }
    }
    runs
}

#[test]
fn gpu_the_plane_shows_as_many_checker_periods_as_it_repeats() {
    let material = MaterialAsset {
        base_color_map: Some(checker_png().as_str().into()),
        ..MaterialAsset::default()
    };
    let Some(once) = render(&material, "checker", PreviewMesh::Plane, Some(1)) else {
        return;
    };
    let thrice = render(&material, "checker", PreviewMesh::Plane, Some(3)).unwrap();
    assert_eq!(cells_across_far_edge(&once), 2, "one period: two cells");
    assert_eq!(
        cells_across_far_edge(&thrice),
        6,
        "three periods: six cells"
    );
}

fn relief_contrast(image: &RgbImage) -> f32 {
    let (lo, hi) = (RES * 7 / 16, RES * 9 / 16);
    let mut luma: Vec<f32> = (lo..hi)
        .flat_map(|y| (lo..hi).map(move |x| (x, y)))
        .map(|(x, y)| {
            image
                .get_pixel(x, y)
                .0
                .iter()
                .map(|&c| c as f32)
                .sum::<f32>()
        })
        .collect();
    luma.sort_by(f32::total_cmp);
    let at = |q: f32| luma[((luma.len() - 1) as f32 * q) as usize];
    (at(0.95) - at(0.05)) / (at(0.95) + at(0.05)).max(1.0)
}

#[test]
fn gpu_the_raking_light_reads_relief_the_front_lit_sphere_flattens() {
    let material = MaterialAsset {
        base_color: [0.7, 0.7, 0.7],
        roughness: 0.8,
        normal_map: Some(grooves_png().as_str().into()),
        ..MaterialAsset::default()
    };
    let Some(plane) = render(&material, "grooves", PreviewMesh::Plane, Some(3)) else {
        return;
    };
    let sphere = render(&material, "grooves", PreviewMesh::Sphere, Some(3)).unwrap();
    let (plane, sphere) = (relief_contrast(&plane), relief_contrast(&sphere));
    eprintln!("relief contrast: plane {plane:.3}, sphere {sphere:.3}");
    assert!(
        plane > sphere * 1.5,
        "plane {plane:.3} vs sphere {sphere:.3}"
    );
}
