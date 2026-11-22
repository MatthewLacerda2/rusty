use std::time::Instant;

use glam::Vec3;

use super::quality::error;
use super::{available, Oidn};
use rusty_lighting::lightmap::{
    bake_with_progress, AreaShape, BakeLight, BakeProgress, BakeScene, LightShape, Lightmap,
};
use rusty_lighting::occlusion::bench::level::rooms;
use rusty_scene::lighting::settings::{BakeSettings, Denoiser};
use rusty_scene::AmbientRadiance;

const REFERENCE: u32 = 4096;
fn level(direct: bool) -> BakeScene {
    let mut scene = BakeScene::gather(&rooms(2), &|_, _| None);
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

type Row = (u32, Denoiser, u32);

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

fn dump(maps: &[Lightmap], name: &str) {
    let Some(dir) = std::env::var_os("RUSTY_DENOISE_DUMP") else {
        return;
    };
    let mut maps: Vec<&Lightmap> = maps.iter().collect();
    maps.sort_by_key(|m| std::cmp::Reverse((m.size, m.entity)));
    let byte = |c: f32| ((c / (1.0 + c)).powf(1.0 / 2.2) * 255.0) as u8;
    for (k, map) in maps.iter().take(4).enumerate() {
        let pixels = map
            .texels()
            .into_iter()
            .flat_map(|v| v.to_array().map(byte));
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
