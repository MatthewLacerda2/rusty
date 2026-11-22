use glam::{Quat, Vec3};

/// `CubicSpline` falls back to `Linear`: its in/out tangents are not imported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Interpolation {
    Step,
    #[default]
    Linear,
}

/// `values` holds one value per entry of `times`, which are finite and ascend: the glTF
/// import drops a channel that breaks either ([`ImportedAsset::validate`] holds it). An
/// empty track contributes nothing (the joint keeps its bind value).
///
/// [`ImportedAsset::validate`]: crate::ImportedAsset::validate
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Track<T> {
    pub times: Vec<f32>,
    pub values: Vec<T>,
    pub interpolation: Interpolation,
}

impl<T> Track<T> {
    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    fn end_time(&self) -> f32 {
        self.times.last().copied().unwrap_or(0.0)
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct JointTrack {
    pub translation: Track<Vec3>,
    pub rotation: Track<Quat>,
    pub scale: Track<Vec3>,
}

impl JointTrack {
    pub fn is_empty(&self) -> bool {
        self.translation.is_empty() && self.rotation.is_empty() && self.scale.is_empty()
    }

    fn end_time(&self) -> f32 {
        self.translation
            .end_time()
            .max(self.rotation.end_time())
            .max(self.scale.end_time())
    }
}

/// `tracks.len()` equals the bound skin's joint count; an untouched slot holds an empty track.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AnimationClip {
    pub name: String,
    pub tracks: Vec<JointTrack>,
    pub duration: f32,
}

impl AnimationClip {
    pub fn recompute_duration(&mut self) {
        self.duration = self
            .tracks
            .iter()
            .map(JointTrack::end_time)
            .fold(0.0_f32, f32::max);
    }
}
