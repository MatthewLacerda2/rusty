use glam::Vec3;

use crate::lighting::sh::Sh9;
use crate::lighting::DiffuseModel;
use crate::{AmbientRadiance, LightType, Scene};

#[derive(Clone, Copy, Debug)]
struct AnalyticEnv {
    pub sky: AmbientRadiance,
    pub sun_dir: Option<Vec3>,
    pub sun_radiance: Vec3,
    pub diffuse: DiffuseModel,
}

impl AnalyticEnv {
    pub fn from_scene(scene: &Scene) -> Self {
        let sky = scene.drawn_ambient().radiance();
        let mut sun_dir = None;
        let mut sun_radiance = Vec3::ZERO;
        for id in scene.world.ids_with_light() {
            if !scene.world.is_active_in_hierarchy(id) {
                continue;
            }
            let light = scene.world.light(id).expect("id came from ids_with_light");
            if light.light_type == LightType::Directional {
                let forward = scene.world.transform_direction(id, Vec3::NEG_Z);
                sun_dir = Some((-forward).normalize_or_zero());
                sun_radiance = light.color * light.intensity;
                break;
            }
        }
        Self {
            sky,
            sun_dir,
            sun_radiance,
            diffuse: scene.lighting_settings.diffuse_model,
        }
    }

    pub fn project(&self) -> Sh9 {
        let mut sh = Sh9::zero();
        let dirs = fibonacci_sphere(512);
        let w = 4.0 * std::f32::consts::PI / dirs.len() as f32;
        for d in &dirs {
            sh.add_sample(*d, self.sky.incoming(*d), w);
        }
        if let Some(dir) = self.sun_dir {
            let mut sun = Sh9::zero();
            sun.add_sample(dir, self.sun_radiance, 1.0);
            sh.accumulate(&sun.band_scaled(self.diffuse.sh_band_scale()));
        }
        sh
    }
}

pub fn analytic_fill(scene: &mut Scene) {
    let env = AnalyticEnv::from_scene(scene);
    for probe in scene.probes.probes_mut() {
        let mut local = env;
        let h = 1.0 + 0.25 * probe.position.y;
        local.sky = env.sky.scaled(h.max(0.0));
        probe.sh = local.project();
    }
}

pub fn fibonacci_sphere(n: usize) -> Vec<Vec3> {
    let golden = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
        let r = (1.0 - y * y).max(0.0).sqrt();
        let theta = golden * i as f32;
        v.push(Vec3::new(theta.cos() * r, y, theta.sin() * r));
    }
    v
}

#[cfg(test)]
#[path = "probe_fill_tests.rs"]
mod tests;
