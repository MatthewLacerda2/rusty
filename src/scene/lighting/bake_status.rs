//! The editor's background bake as a script sees it (#1999): whether one runs or
//! is asked for, and what became of the last one. `Lighting.GenerateAsync()` asks
//! the editor's next frame to press Generate Lighting; `Lighting.IsRunning()` reads
//! [`Scene::bake_busy`], and `Lighting.GetBakeStatus()` reads [`BakeStatus`], so an
//! agent learns in band that its bake failed or was refused, not from the window's
//! console. Dev-only, as the bakes are.

use crate::scene::Scene;

/// What became of the editor's last background bake (Generate Lighting or Bake
/// Occlusion, whoever pressed it): the editor writes it as the bake moves.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum BakeStatus {
    /// No background bake since the editor opened.
    #[default]
    Idle,
    /// `Lighting.GenerateAsync()` asked; the editor's next frame starts it.
    Requested,
    /// The bake's job runs; how far, as the editor's last frame saw it.
    Running(BakeProgress),
    /// It ended and wrote its result; the summary line.
    Done(String),
    /// It was cancelled from its card; the old lighting stayed.
    Cancelled(String),
    /// It failed or was dropped midway; why, and what reached the scene.
    Failed(String),
    /// It never started; why.
    Refused(String),
}

impl BakeStatus {
    /// The state's name, as `Lighting.GetBakeStatus().state` returns it.
    pub fn state(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Requested => "requested",
            Self::Running(_) => "running",
            Self::Done(_) => "done",
            Self::Cancelled(_) => "cancelled",
            Self::Failed(_) => "failed",
            Self::Refused(_) => "refused",
        }
    }

    /// The line that says why it ended as it did, once it ended.
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Idle | Self::Requested | Self::Running(_) => None,
            Self::Done(m) | Self::Cancelled(m) | Self::Failed(m) | Self::Refused(m) => Some(m),
        }
    }
}

/// How far a running background bake is, for the Play refusal's line (#2042).
#[derive(Clone, Debug, PartialEq)]
pub struct BakeProgress {
    /// What it bakes, as the line says it: `"lighting"`, a later step of Generate
    /// Lighting (`"lighting (probes)"`), or Bake Occlusion's `"occlusion"`.
    pub what: &'static str,
    /// Its step's share done, 0 to 100, when the step counts its work.
    pub percent: Option<u8>,
    /// Whether the card offers Cancel now: only during the lightmaps, before a
    /// cancel was asked for. The probe and occlusion steps have none.
    pub cancellable: bool,
}

impl BakeProgress {
    /// A bake just started on its first step, nothing counted yet.
    pub fn starting(what: &'static str) -> Self {
        Self {
            what,
            percent: None,
            cancellable: what == "lighting",
        }
    }
}

impl Scene {
    /// Why Play is refused now, or `None` when it may start (#2042): a background
    /// bake runs or a script asked for one ([`Scene::bake_busy`]). The one test the
    /// Play button and `Editor.EnterPlay()` both read, the mirror of the bakes'
    /// refusal during Play: whichever was asked for first wins.
    pub fn play_refusal(&self) -> Option<String> {
        if !self.bake_busy() {
            return None;
        }
        let progress = match &self.lighting_bake_status {
            BakeStatus::Running(progress) => progress.clone(),
            // A `GenerateAsync` the editor's next frame starts: nothing runs yet,
            // so there is no Cancel to point at, only the wait.
            _ => BakeProgress {
                cancellable: false,
                ..BakeProgress::starting("lighting")
            },
        };
        let percent = progress
            .percent
            .map(|p| format!(": {p}%"))
            .unwrap_or_default();
        let then = match progress.cancellable {
            true => "Cancel the bake to play",
            false => "wait for the bake to finish to play",
        };
        Some(format!("Baking {}{percent} — {then}", progress.what))
    }

    /// Whether the editor's background bake runs or a script asked for one: what
    /// `Lighting.IsRunning()` reads and what the blocking bakes refuse on.
    pub fn bake_busy(&self) -> bool {
        self.lighting_bake_running || self.lighting_bake_requested
    }

    /// Drop a pending `Lighting.GenerateAsync()` because `why` (Stop, File ▸ New,
    /// a load): a request belongs to the scene and the mode it was made in, so it
    /// never bakes a scene the agent did not ask about. The status says so.
    pub fn drop_bake_request(&mut self, why: &str) {
        if std::mem::take(&mut self.lighting_bake_requested) {
            self.lighting_bake_status = BakeStatus::Refused(format!(
                "Lighting.GenerateAsync: {why} before the editor started the bake"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_request_reads_refused_and_nothing_else_is_touched() {
        let mut scene = Scene::new();
        scene.drop_bake_request("Play started");
        assert_eq!(scene.lighting_bake_status, BakeStatus::Idle, "none pending");
        scene.lighting_bake_requested = true;
        assert!(scene.bake_busy());
        scene.drop_bake_request("Play started");
        assert!(!scene.bake_busy());
        assert_eq!(scene.lighting_bake_status.state(), "refused");
        let why = scene.lighting_bake_status.message().unwrap();
        assert!(why.contains("Play started"), "{why}");
    }

    /// The Play refusal over every state the job can be in (#2042).
    #[test]
    fn play_is_refused_exactly_while_a_bake_is_busy() {
        let mut scene = Scene::new();
        assert_eq!(scene.play_refusal(), None, "idle");
        scene.lighting_bake_requested = true;
        scene.lighting_bake_status = BakeStatus::Requested;
        let asked = scene.play_refusal().expect("requested");
        assert_eq!(
            asked,
            "Baking lighting — wait for the bake to finish to play"
        );
        scene.lighting_bake_requested = false;
        scene.lighting_bake_running = true;
        scene.lighting_bake_status = BakeStatus::Running(BakeProgress {
            percent: Some(42),
            ..BakeProgress::starting("lighting")
        });
        let line = scene.play_refusal().expect("running");
        assert_eq!(line, "Baking lighting: 42% — Cancel the bake to play");
        scene.lighting_bake_status = BakeStatus::Running(BakeProgress::starting("occlusion"));
        let line = scene.play_refusal().expect("occlusion");
        assert_eq!(
            line,
            "Baking occlusion — wait for the bake to finish to play"
        );
        scene.lighting_bake_running = false;
        for ended in [
            BakeStatus::Done("baked".into()),
            BakeStatus::Cancelled("cancelled".into()),
            BakeStatus::Failed("failed".into()),
        ] {
            scene.lighting_bake_status = ended;
            assert_eq!(
                scene.play_refusal(),
                None,
                "{:?}",
                scene.lighting_bake_status
            );
        }
    }
}
