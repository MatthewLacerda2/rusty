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
    /// The bake's job runs.
    Running,
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
            Self::Running => "running",
            Self::Done(_) => "done",
            Self::Cancelled(_) => "cancelled",
            Self::Failed(_) => "failed",
            Self::Refused(_) => "refused",
        }
    }

    /// The line that says why it ended as it did, once it ended.
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Idle | Self::Requested | Self::Running => None,
            Self::Done(m) | Self::Cancelled(m) | Self::Failed(m) | Self::Refused(m) => Some(m),
        }
    }
}

impl Scene {
    /// Whether the editor's background bake runs or a script asked for one: what
    /// `Lighting.IsRunning()` reads and what the blocking bakes refuse on.
    pub fn bake_busy(&self) -> bool {
        self.lighting_bake_running || self.lighting_bake_requested
    }

    /// Drop a pending `Lighting.GenerateAsync()` because `why` (Play, Stop, a
    /// load): a request belongs to the scene and the mode it was made in, so it
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
}
