//! src/components/particle/render.rs — how an emitter's particles are drawn (#440).
//!
//! The render settings are pure authoring data on the emitter; the particle pass
//! (`render::passes::particles`) reads them. Every default reproduces the pre-#440
//! look — an unlit, hard-edged, camera-facing sprite with one frame — so scenes
//! saved before this file load unchanged.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::runtime::Particle;

/// The shape each particle is drawn as.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticleRenderMode {
    /// A quad that always faces the camera (smoke, fire, muzzle flash).
    #[default]
    Billboard,
    /// A quad stretched along the particle's velocity (sparks, tracers, streaks).
    Stretched,
    /// A quad lying flat in the world XZ plane (ground splashes, shockwave rings).
    Horizontal,
    /// A camera-facing quad that stays upright — turns about world Y only.
    Vertical,
    /// An instanced mesh with its own material (shell casings, debris, glass).
    Mesh,
}

impl ParticleRenderMode {
    /// Every mode, in inspector / API order.
    pub const ALL: [Self; 5] = [
        Self::Billboard,
        Self::Stretched,
        Self::Horizontal,
        Self::Vertical,
        Self::Mesh,
    ];

    /// The mode's API / inspector name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Billboard => "billboard",
            Self::Stretched => "stretched",
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
            Self::Mesh => "mesh",
        }
    }

    /// Parse an API name (case-insensitive).
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(name))
    }
}

/// A sprite-sheet animation: the texture is a `columns × rows` grid of frames,
/// read left-to-right, top-to-bottom. `1 × 1` (the default) is a plain sprite.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Flipbook {
    pub columns: u32,
    pub rows: u32,
    /// How many times the whole sheet plays over one particle's life.
    pub cycles: f32,
    /// Start each particle on a random frame (varied smoke puffs, debris sprites).
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
    /// Frames on the sheet (at least 1).
    pub fn frames(&self) -> u32 {
        (self.columns.max(1)) * (self.rows.max(1))
    }

    /// The frame `p` shows now: its start frame plus its life played `cycles` times.
    pub fn frame_of(&self, p: &Particle) -> u32 {
        let frames = self.frames();
        if frames == 1 {
            return 0;
        }
        // Just short of 1, so a particle on its last tick still shows the last frame.
        let t = p.life_t().min(0.99999);
        let played = (t * self.cycles.max(0.0) * frames as f32).floor() as u64;
        let frame = played + u64::from(p.start_frame);
        (frame % u64::from(frames)) as u32
    }
}

/// An emitter's render settings. Serde-defaulted as a block, so a pre-#440 scene
/// loads the default (billboard, unlit, hard, one frame).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ParticleRender {
    pub mode: ParticleRenderMode,
    /// `Stretched`: the quad's length as a multiple of its size…
    pub length_scale: f32,
    /// …plus this many world units per unit of speed (faster sparks streak longer).
    pub speed_scale: f32,
    /// `Mesh`: a primitive name (`"Cube"`) or a model asset path.
    pub mesh: Option<String>,
    /// `Mesh`: the scene material the meshes are drawn with (engine default if unset).
    pub material: Option<String>,
    pub flipbook: Flipbook,
    /// Soft particles: fade out over this many world units in front of the scene
    /// behind, so smoke never cuts a hard line into a wall. `0` disables it.
    pub soft_distance: f32,
    /// Lit particles: shaded by the ambient / light probes and the scene lights
    /// (dark in a dark room, lit by a muzzle flash). Off = the sprite's own colour.
    pub lit: bool,
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

    /// A stretched particle's quad length for a given `size` and `velocity`.
    pub fn stretch_length(&self, size: f32, velocity: Vec3) -> f32 {
        size * self.length_scale + velocity.length() * self.speed_scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::ParticleEmitterComponent;

    fn spawned(render: ParticleRender) -> Vec<Particle> {
        let mut e = ParticleEmitterComponent {
            render,
            ..Default::default()
        };
        e.emit_at(Vec3::ZERO, 8);
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
