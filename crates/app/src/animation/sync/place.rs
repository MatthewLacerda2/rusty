use rusty_asset::anim_data::AnimationClip;
use rusty_asset::animation_graph::AnimationGraph;
use rusty_components::{stored_leaf, AnimatorComponent, LeafTime, Motion, Playback, Playhead};

use super::super::motion::MotionContext;
use super::SyncPos;

impl MotionContext<'_> {
    pub(super) fn place_pending(&self, playback: &mut Playback) {
        if playback.sync.from.is_none() {
            return;
        }
        let node = playback.current_node.as_deref();
        // Resolved before `from` is taken, so the placement stays pending until the incoming clips
        // load.
        let Some(lead) = self.lead(playback.motion(), node, None) else {
            return;
        };
        let Some(from) = playback.sync.from.take() else {
            return;
        };
        let out = self.lead(from.motion(), from.node.as_deref(), from.leader.as_deref());
        let pos = out.map(|out| out.position(from.time, from.looping));
        if let Some(pos) = pos {
            playback.time = lead.place(pos, playback.time, playback.loop_clip);
        }
        if lead.tree && !lead.markers.is_empty() {
            playback.sync.leader = Some(lead.clip.name.clone());
        }
        // Entered twice before a step (A, B, C): B, fading out, was never placed, so it goes there
        // too.
        let unplaced = playback.sync.following && playback.sync.previous_node != from.node;
        if let (Some(pos), true) = (pos, unplaced) {
            self.place_previous(playback, pos);
        }
    }

    fn place_previous(&self, playback: &mut Playback, pos: SyncPos) {
        let Some(motion) = playback.previous_motion() else {
            return;
        };
        let node = playback.sync.previous_node.as_deref();
        let Some(lead) = self.lead(motion, node, None) else {
            return;
        };
        let looping = playback.previous_loop;
        playback.previous_time = lead.place(pos, playback.previous_time, looping);
        if lead.tree && !lead.markers.is_empty() {
            playback.sync.previous_leader = Some(lead.clip.name.clone());
        }
    }

    pub(in crate::animation) fn track_leaves(
        &self,
        motion: Option<Motion<'_>>,
        (time, looping): (f32, bool),
        leaves: &mut Vec<LeafTime>,
    ) {
        let Some(Motion::Tree(node)) = motion else {
            leaves.clear();
            return;
        };
        let phase = Playhead::tree(0.0).bounds(looping).apply(time);
        let Some(sync) = self.tree_sync(node, phase, looping) else {
            leaves.clear();
            return;
        };
        // Updated in place while the leaf set matches the one stored, so a step allocates nothing.
        let (mut i, mut rebuilt) = (0, None::<Vec<LeafTime>>);
        self.for_each_leaf_clip(node, |clip, _, markers| {
            let near = stored_leaf(leaves, &clip.name);
            let time = sync.leaf_time(clip, markers, phase, near);
            match &mut rebuilt {
                None if leaves.get(i).is_some_and(|l| l.clip == clip.name) => {
                    leaves[i].time = time;
                }
                None => {
                    let mut placed = leaves[..i].to_vec();
                    placed.push(LeafTime {
                        clip: clip.name.clone(),
                        time,
                    });
                    rebuilt = Some(placed);
                }
                Some(placed) => placed.push(LeafTime {
                    clip: clip.name.clone(),
                    time,
                }),
            }
            i += 1;
        });
        match rebuilt {
            Some(placed) => *leaves = placed,
            None => leaves.truncate(i),
        }
    }
}

pub(in crate::animation) fn place_pending(
    anim: &mut AnimatorComponent,
    clips: &[AnimationClip],
    graph: Option<&AnimationGraph>,
) {
    let AnimatorComponent {
        base,
        layers,
        parameters,
        ..
    } = anim;
    let playbacks = std::iter::once(base).chain(layers.iter_mut().map(|l| &mut l.playback));
    for (layer, playback) in playbacks.enumerate() {
        let context = MotionContext {
            clips,
            machine: graph.and_then(|g| g.machine(layer)),
            parameters,
        };
        context.place_pending(playback);
    }
}
