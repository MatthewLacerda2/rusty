use crate::Scene;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum BakeStatus {
    #[default]
    Idle,
    Requested,
    Running(BakeProgress),
    Done(String),
    Cancelled(String),
    Failed(String),
    /// It never started; why.
    Refused(String),
}

impl BakeStatus {
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

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Idle | Self::Requested | Self::Running(_) => None,
            Self::Done(m) | Self::Cancelled(m) | Self::Failed(m) | Self::Refused(m) => Some(m),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BakeProgress {
    pub what: &'static str,
    pub percent: Option<u8>,
    pub cancellable: bool,
}

/// Where a bake queue is: scene `index` (from 1) of `total`, its path.
#[derive(Clone, Debug, PartialEq)]
pub struct QueueSpot {
    pub index: usize,
    pub total: usize,
    pub scene: String,
}

impl QueueSpot {
    /// `scene 2 of 3: tower-bar.scene`, then the bake's own progress when one runs.
    pub fn line(&self, status: &BakeStatus) -> String {
        let at = format!("scene {} of {}: {}", self.index, self.total, self.scene);
        match status {
            BakeStatus::Running(p) => match p.percent {
                Some(percent) => format!("{at}, {} {percent}%", p.what),
                None => format!("{at}, {}", p.what),
            },
            _ => at,
        }
    }
}

impl BakeProgress {
    pub fn starting(what: &'static str) -> Self {
        Self {
            what,
            percent: None,
            cancellable: what == "lighting",
        }
    }
}

impl Scene {
    pub fn play_refusal(&self) -> Option<String> {
        if !self.bake_busy() {
            return None;
        }
        let progress = match &self.lighting_bake_status {
            BakeStatus::Running(progress) => progress.clone(),
            // A `GenerateAsync` not started yet: no Cancel to point at, only the wait.
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

    /// A bake runs or is asked for: the editor's Generate Lighting or Bake Occlusion, or
    /// its bake queue, between two scenes included.
    pub fn bake_busy(&self) -> bool {
        self.lighting_bake_running
            || self.lighting_bake_requested
            || self.lighting_queue_requested
            || self.lighting_queue.is_some()
    }

    /// A request belongs to the scene and mode it was made in, so it never bakes another.
    pub fn drop_bake_request(&mut self, why: &str) {
        if std::mem::take(&mut self.lighting_queue_requested) {
            self.lighting_bake_status = BakeStatus::Refused(format!(
                "Lighting.GenerateScenesAsync: {why} before the editor started the bake queue; \
                 Lighting.ResumeBakeQueue() continues it"
            ));
        }
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
