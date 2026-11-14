//! crates/app/src/animation/sync/fade.rs — one step of a synced crossfade (#1896):
//! the heavier motion (the incoming one on a tie, Unreal's rule) advances at
//! its own rate and the lighter one is re-matched to its phase.

use rusty_components::{follow_clamped, wrap_delta, Playback, Playhead, Step, SyncSource};

use super::super::motion::MotionContext;

impl MotionContext<'_> {
    /// One step of a synced crossfade: the heavier motion (the incoming one on
    /// a tie) advances at its own rate, the lighter one is re-matched to its
    /// phase. Returns the current playhead's step.
    pub(in crate::animation) fn advance_synced(
        &self,
        playback: &mut Playback,
        (dt, speed): (f32, f32),
        current: Playhead,
        previous_per_second: f32,
    ) -> Step {
        let out = outgoing(playback);
        let step = playback.scaled(dt, speed);
        let forward = step >= 0.0;
        let incoming_leads = playback.crossfade_weight() >= 0.5;
        let from_previous = playback.previous_time;
        // A leading incoming motion still repays travel it was re-matched
        // back over while it followed, before the fade can end.
        let moved = incoming_leads.then(|| {
            let moved = playback.advance_current(step, current);
            playback.repay(moved, forward)
        });
        playback.advance_previous(dt, step, previous_per_second);
        let node = playback.current_node.clone();
        let lead_in = self.lead(
            playback.motion(),
            node.as_deref(),
            playback.sync.leader.as_deref(),
        );
        let lead_out = self.lead(out.motion(), out.node.as_deref(), out.leader.as_deref());
        let (Some(lead_in), Some(lead_out)) = (lead_in, lead_out) else {
            return moved.unwrap_or_else(|| {
                let moved = playback.advance_current(step, current);
                playback.repay(moved, forward)
            });
        };
        if let Some(moved) = moved {
            if playback.is_crossfading() {
                let pos = lead_in.position(playback.time, playback.loop_clip);
                let target = lead_out.place(pos, from_previous, out.looping);
                let wrap = lead_out.wrap();
                // The outgoing follower obeys the rules the incoming one does
                // (`Playback::follow`): round a loop the shortest way, and a
                // non-looping one on to its end and held there, never back.
                playback.previous_time = match out.looping && wrap > 0.0 {
                    true => from_previous + wrap_delta(target - from_previous, wrap),
                    false => follow_clamped(from_previous, target, wrap.max(0.0), forward),
                };
            }
            return moved;
        }
        let forward = playback.previous_time >= from_previous;
        let pos = lead_out.position(playback.previous_time, out.looping);
        let target = lead_in.place(pos, playback.time, playback.loop_clip);
        playback.follow(current, target, forward)
    }
}

/// The motion `playback`'s synced crossfade is fading out, as a [`SyncSource`].
fn outgoing(playback: &Playback) -> SyncSource {
    SyncSource {
        clip: playback.previous_clip.clone().unwrap_or_default(),
        tree: playback.previous_tree.clone(),
        node: playback.sync.previous_node.clone(),
        time: playback.previous_time,
        looping: playback.previous_loop,
        leader: playback.sync.previous_leader.clone(),
    }
}
