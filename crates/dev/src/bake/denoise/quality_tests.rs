//! How close a denoised bake gets to a converged one (#1884), measured by hand in a
//! release build: `cargo test --release --features dev --lib denoised_bake_error --
//! --ignored --nocapture` (its two reference bakes take about 8 minutes each, past
//! nextest's 5-minute kill). A measurement, never a gate; it is what chose
//! [`BakeSettings::DEFAULT_SAMPLES`]. The level is the occlusion bench's 2 × 2
//! rooms on two storeys at the default density, a lamp pair in two rooms only, so
//! the other six are lit by bounce through the doorways; once with the lamps'
//! direct light baked, once with only their bounce. Each bake is compared with a
//! raw `REFERENCE`-path one over the charts' interiors (`quality::error`).
//! `RUSTY_DENOISE_DUMP=<dir>` also writes each bake's four largest lightmaps there
//! as PNGs, to look at.

use std::time::Instant;

use glam::Vec3;

use super::quality::error;
use super::{available, Oidn};
use rusty_scene::lighting::lightmap::{
    bake_with_progress, AreaShape, BakeLight, BakeProgress, BakeScene, BakeSettings, Denoiser,
    LightShape, Lightmap,
};
use rusty_scene::lighting::occlusion::bench::level::rooms;
use rusty_scene::AmbientRadiance;

/// Paths per texel of the converged bake every row is measured against.
const REFERENCE: u32 = 4096;
/// The level, closed and with no sky: a point lamp and an Area panel in two of its
/// eight rooms, their direct light baked too when `direct` (Unity's `Baked` lamps),
/// else only their bounce (`Mixed`), so the denoiser's work is all there is to see.
fn level(direct: bool) -> BakeScene {
    let mut scene = BakeScene::gather(&rooms(2), &|_| None);
    scene.sky = AmbientRadiance::Hemisphere(Vec3::ZERO);
    for top in [Vec3::new(0.0, 3.4, 0.0), Vec3::new(10.0, 7.4, 10.0)] {
        let (position, center) = (
            top + Vec3::new(3.0, 0.0, 3.0),
            top + Vec3::splat(6.0).with_y(0.0),
        );
        let (right, up, forward, range) = (Vec3::X, Vec3::Z * 0.5, Vec3::NEG_Y, 12.0);
        let area = AreaShape {
            center,
            right,
            up,
            forward,
            range,
        };
        for shape in [
            LightShape::Point { position, range },
            LightShape::Area(area),
        ] {
            let radiance = Vec3::splat(4.0);
            let (bakes_direct, mask_channel) = (direct, None);
            scene.lights.push(BakeLight {
                shape,
                radiance,
                bakes_direct,
                mask_channel,
            });
        }
    }
    scene
}

/// One bake: its samples, its denoiser and its Gaussian's radius.
type Row = (u32, Denoiser, u32);

/// `scene` baked as `row` says, the seconds it took, and those of them OIDN spent.
fn baked(scene: &BakeScene, (samples, denoiser, radius): Row) -> (Vec<Lightmap>, f64, f64) {
    let settings = BakeSettings {
        samples,
        denoiser,
        filter_radius: radius,
        ..Default::default()
    };
    let oidn = std::sync::Arc::new(Oidn::default());
    let progress = match denoiser {
        Denoiser::Oidn => BakeProgress::default().with_denoiser(oidn.clone()),
        _ => BakeProgress::default(),
    };
    let t = Instant::now();
    let maps = bake_with_progress(scene, &settings, &progress).expect("not cancelled");
    let secs = t.elapsed().as_secs_f64();
    (maps, secs, oidn.spent().1.as_secs_f64())
}

/// The four largest of `maps`, tone-mapped into `dir/1884_<name>_<k>.png`.
fn dump(maps: &[Lightmap], name: &str) {
    let Some(dir) = std::env::var_os("RUSTY_DENOISE_DUMP") else {
        return;
    };
    let mut maps: Vec<&Lightmap> = maps.iter().collect();
    maps.sort_by_key(|m| std::cmp::Reverse((m.size, m.entity)));
    let byte = |c: f32| ((c / (1.0 + c)).powf(1.0 / 2.2) * 255.0) as u8;
    for (k, map) in maps.iter().take(4).enumerate() {
        let pixels = map.texels.iter().flat_map(|v| v.to_array().map(byte));
        let image = image::RgbImage::from_raw(map.size, map.size, pixels.collect());
        let path = std::path::Path::new(&dir).join(format!("1884_{name}_{k}.png"));
        image.expect("size² texels").save(path).expect("written");
    }
}

#[test]
// lint: allow(skip) a release-build measurement against a long reference bake, run by hand
#[ignore = "a release-build measurement, run by hand"]
fn denoised_bake_error() {
    available().expect("this measurement needs OIDN");
    for (direct, label) in [(true, "baked"), (false, "bounce")] {
        let scene = level(direct);
        let (reference, secs, _) = baked(&scene, (REFERENCE, Denoiser::None, 0));
        dump(&reference, &format!("{label}_reference"));
        let count = reference.len();
        println!("{label} lamps: reference {REFERENCE} spp raw, {count} lightmaps, {secs:.1} s");
        println!("| spp | denoiser | relative | MAE / mean | bake | of it OIDN |");
        let r = BakeSettings::default().filter_radius;
        let rows = [(128, Denoiser::Filter, r), (32, Denoiser::Filter, r)];
        let rows = rows
            .into_iter()
            .chain([(32, Denoiser::None, 0), (32, Denoiser::Oidn, 0)]);
        for row in rows.chain([8, 16, 32, 64].map(|spp| (spp, Denoiser::Oidn, r))) {
            let (maps, secs, oidn) = baked(&scene, row);
            let e = error(&scene, &maps, &reference);
            let alone = match (row.1, row.2) {
                (Denoiser::Oidn, 0) => " alone",
                _ => "",
            };
            let (samples, name) = (row.0, format!("{}{alone}", row.1.name()));
            dump(&maps, &format!("{label}_{name}_{samples}"));
            let [mae, rel] = [e.mae, e.relative].map(|x| x * 100.0);
            println!("| {samples} | {name} | {rel:.1}% | {mae:.1}% | {secs:.1} s | {oidn:.1} s |");
        }
    }
}
