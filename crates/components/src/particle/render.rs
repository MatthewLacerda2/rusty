use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::runtime::Particle;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ParticleRenderMode {
        /// A quad that always faces the camera (smoke, fire, muzzle flash).
        #[default]
        Billboard = "billboard",
        Stretched = "stretched",
        /// A quad lying flat in the world XZ plane (ground splashes, shockwave rings).
        Horizontal = "horizontal",
        /// A camera-facing quad that stays upright — turns about world Y only.
        Vertical = "vertical",
        Mesh = "mesh",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Flipbook {
    pub columns: u32,
    pub rows: u32,
    pub cycles: f32,
    pub random_start: bool,
}

impl Default for Flipbook {
    fn default() -> Self {
        Self {
            columns: 1,
            rows: 1,
            cycles: 1.0,
            random_start: false,
        }
    }
}

impl Flipbook {
    pub fn frames(&self) -> u32 {
        (self.columns.max(1)) * (self.rows.max(1))
    }

    pub fn frame_of(&self, p: &Particle) -> u32 {
        let frames = self.frames();
        if frames == 1 {
            return 0;
        }
        // Short of 1, so a dying particle still shows the last frame.
        let t = p.life_t().min(0.99999);
        let played = (t * self.cycles.max(0.0) * frames as f32).floor() as u64;
        let frame = played + u64::from(p.start_frame);
        (frame % u64::from(frames)) as u32
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ParticleRender {
    pub mode: ParticleRenderMode,
    pub length_scale: f32,
    /// `Stretched`: world units the quad lengthens per unit of speed (faster sparks streak longer).
    pub speed_scale: f32,
    pub mesh: Option<String>,
    pub material: Option<String>,
    pub flipbook: Flipbook,
    pub soft_distance: f32,
    pub lit: bool,
    pub reactive: crate::TaaReactive,
}

impl Default for ParticleRender {
    fn default() -> Self {
        Self {
            mode: ParticleRenderMode::Billboard,
            length_scale: 1.0,
            speed_scale: 0.0,
            mesh: None,
            material: None,
            flipbook: Flipbook::default(),
            soft_distance: 0.0,
            lit: false,
            reactive: crate::TaaReactive::Auto,
        }
    }
}

impl ParticleRender {
    /// Whether spawning must draw a tumble axis (mesh particles only), so every other
    /// emitter's seeded stream — and its replays — stays exactly as before.
    pub(crate) fn wants_axis(&self) -> bool {
        self.mode == ParticleRenderMode::Mesh
    }

    /// Whether spawning must draw a random start frame.
    pub(crate) fn wants_start_frame(&self) -> bool {
        self.flipbook.random_start && self.flipbook.frames() > 1
    }

    pub fn stretch_length(&self, size: f32, velocity: Vec3) -> f32 {
        size * self.length_scale + velocity.length() * self.speed_scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParticleEmitterComponent;

    fn spawned(render: ParticleRender) -> Vec<Particle> {
        let mut e = ParticleEmitterComponent {
            render,
            ..Default::default()
        };
        e.emit_at(crate::particle::EmitterFrame::IDENTITY, 8);
        e.runtime.particles
    }

    #[test]
    fn look_only_settings_leave_the_seeded_stream_alone() {
        let plain = spawned(ParticleRender::default());
        let styled = spawned(ParticleRender {
            mode: ParticleRenderMode::Stretched,
            soft_distance: 1.0,
            lit: true,
            ..Default::default()
        });
        let pos = |ps: &[Particle]| ps.iter().map(|p| p.position).collect::<Vec<_>>();
        assert_eq!(pos(&plain), pos(&styled));
    }

    #[test]
    fn mesh_particles_tumble_about_unit_axes_and_sheets_start_anywhere() {
        let mesh = spawned(ParticleRender {
            mode: ParticleRenderMode::Mesh,
            ..Default::default()
        });
        assert!(mesh.iter().all(|p| (p.axis.length() - 1.0).abs() < 1e-4));
        assert!(mesh.windows(2).any(|w| w[0].axis != w[1].axis));
        let book = Flipbook {
            columns: 4,
            rows: 4,
            cycles: 1.0,
            random_start: true,
        };
        let sheet = spawned(ParticleRender {
            flipbook: book,
            ..Default::default()
        });
        assert!(sheet.iter().all(|p| p.start_frame < 16));
        assert!(sheet
            .windows(2)
            .any(|w| w[0].start_frame != w[1].start_frame));
    }
}
