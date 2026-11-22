#![deny(clippy::unwrap_used)]

pub mod backend;
pub mod capture;
pub mod device;
pub mod doppler;
pub mod introspection;
pub mod maestro;
pub mod mix;
pub mod mixer;
pub mod occlusion;
#[doc(hidden)]
pub mod recording;
pub mod reverb;
pub mod spatial;
pub mod speaker;
mod stop;

pub use backend::{AudioBackend, NullBackend, PlayParams, VoiceId, VoiceMix};
pub use device::KiraBackend;
pub use introspection::{AudioEvent, AudioEventKind, AudioEventLog, SpatialResult, VoiceInfo};
pub use maestro::AudioMaestro;
pub use mix::{MixEnv, Rolloff, Shot};
pub use mixer::{GroupPatch, GroupState, Mixer};
pub use occlusion::{Occluder, OcclusionSettings};
pub use reverb::ReverbState;
pub use spatial::Listener;
pub use speaker::SpeakerMode;
