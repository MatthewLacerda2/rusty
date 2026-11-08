//! src/app/animation/states.rs — the graph-state callbacks a step raises (#1901).
//!
//! After each animation step, every layer's active states — its current node
//! and, while a crossfade blends, the node it leaves — are compared with the
//! states last reported ([`StateTracker`]). A state no longer active exits, a
//! new one enters, one still active updates, Unity's `StateMachineBehaviour`
//! timing: a crossfade enters its target as it starts and exits its source as
//! it ends, and both update on the steps between; a hard cut exits and enters
//! in one step. Re-entering the playing node counts as a new state (its entry
//! count, [`NodeStates`](crate::components::NodeStates), changes).
//!
//! The order is deterministic: layer by layer (base first), and within a layer
//! every exit, then every enter, then every update, the current state before
//! the one fading out.

use crate::asset::anim_data::AnimationClip;
use crate::asset::animation_graph::AnimationGraph;
use crate::components::{
    AnimatorComponent, Motion, Playback, Reported, StateCallback, StateEvent, StateInfo,
};

use super::motion::MotionContext;

/// The base layer's name in a state's info, as Unity names it.
pub const BASE_LAYER: &str = "Base Layer";

/// One active state as this step leaves it.
struct Active<'a> {
    node: &'a str,
    entry: u32,
    raw: f32,
    current: bool,
    motion: Motion<'a>,
}

/// The state callbacks `anim`'s layers raise this step, in firing order, and
/// the tracker brought up to date. `travel` is each layer's current-playhead
/// travel this step ([`Stepped::travel`](super::Stepped)); `clips` and `graph`
/// give the states' lengths and tags.
pub fn state_callbacks(
    anim: &mut AnimatorComponent,
    clips: &[AnimationClip],
    graph: Option<&AnimationGraph>,
    travel: &[f32],
) -> Vec<StateCallback> {
    let layer_count = anim.layers.len() + 1;
    let mut tracker = std::mem::take(&mut anim.states);
    tracker
        .layers
        .resize(layer_count.max(tracker.layers.len()), Vec::new());
    let mut out = Vec::new();
    for (layer, prior) in tracker.layers.iter_mut().enumerate() {
        let Some(playback) = playback(anim, layer) else {
            out.extend(prior.drain(..).map(|r| exit(r.info)));
            continue;
        };
        let machine = graph.and_then(|g| g.machine(layer));
        let tag_of = |node: &str| {
            let tag = machine.and_then(|m| m.node(node)).map(|n| n.tag.as_str());
            tag.filter(|t| !t.is_empty()).map(str::to_string)
        };
        let ctx = MotionContext {
            clips,
            machine,
            parameters: &anim.parameters,
        };
        let name = layer_name(anim, layer);
        let moved = travel.get(layer).copied().unwrap_or(0.0);
        let mut next = Vec::new();
        let (mut entered, mut updated) = (Vec::new(), Vec::new());
        for active in actives(playback) {
            let known = prior
                .iter()
                .position(|r| r.node == active.node && r.entry == active.entry);
            let total = match known.map(|i| prior.remove(i)) {
                Some(r) if r.current && active.current => r.total + moved,
                Some(r) => r.total + (active.raw - r.raw),
                None => active.raw,
            };
            let info = StateInfo {
                layer,
                layer_name: name.clone(),
                state: active.node.to_string(),
                tag: tag_of(active.node),
                ..timing(&ctx, active.motion, total)
            };
            let list = if known.is_some() {
                &mut updated
            } else {
                &mut entered
            };
            list.push(info.clone());
            next.push(Reported {
                node: active.node.to_string(),
                entry: active.entry,
                raw: active.raw,
                total,
                current: active.current,
                info,
            });
        }
        out.extend(prior.drain(..).map(|r| exit(r.info)));
        let callback = |event| move |info| StateCallback { event, info };
        out.extend(entered.into_iter().map(callback(StateEvent::Enter)));
        out.extend(updated.into_iter().map(callback(StateEvent::Update)));
        *prior = next;
    }
    tracker.layers.truncate(layer_count);
    refresh_tags(anim, graph);
    anim.states = tracker;
    out
}

fn exit(info: StateInfo) -> StateCallback {
    StateCallback {
        event: StateEvent::Exit,
        info,
    }
}

/// Layer `layer`'s playback (0 is the base layer).
fn playback(anim: &AnimatorComponent, layer: usize) -> Option<&Playback> {
    match layer {
        0 => Some(&anim.base),
        i => anim.layers.get(i - 1).map(|l| &l.playback),
    }
}

fn layer_name(anim: &AnimatorComponent, layer: usize) -> String {
    match layer {
        0 => BASE_LAYER.to_string(),
        i => anim.layers[i - 1].name.clone(),
    }
}

/// `playback`'s active states: its current node, then the node a crossfade
/// leaves.
fn actives(playback: &Playback) -> Vec<Active<'_>> {
    let mut out = Vec::with_capacity(2);
    if let Some(node) = playback.current_node.as_deref() {
        out.push(Active {
            node,
            entry: playback.states.entry,
            raw: playback.time,
            current: true,
            motion: playback.motion(),
        });
    }
    let leaving = playback.states.previous_node.as_deref();
    if let (Some(node), Some(motion)) = (leaving, playback.previous_motion()) {
        out.push(Active {
            node,
            entry: playback.states.previous_entry,
            raw: playback.previous_time,
            current: false,
            motion,
        });
    }
    out
}

/// A state's `time`, `normalized_time` and `length` from its playhead
/// unwrapped since entry (`total`, playhead units: seconds for a clip, cycles
/// for a blend tree).
fn timing(ctx: &MotionContext<'_>, motion: Motion<'_>, total: f32) -> StateInfo {
    let playhead = ctx.playhead(motion);
    let finite = |x: f32| if x.is_finite() { x } else { 0.0 };
    let (time, normalized_time, length) = match motion {
        Motion::Clip(_) if playhead.wrap > 0.0 => (total, total / playhead.wrap, playhead.wrap),
        Motion::Clip(_) => (total, 0.0, playhead.wrap.max(0.0)),
        Motion::Tree(_) if playhead.per_second > 0.0 => {
            let length = 1.0 / playhead.per_second;
            (total * length, total, length)
        }
        Motion::Tree(_) => (0.0, total, 0.0),
    };
    StateInfo {
        time: finite(time),
        normalized_time: finite(normalized_time),
        length: finite(length),
        ..StateInfo::default()
    }
}

/// Every layer's active-node tag from `graph` (`Animator.GetCurrentTag`), so
/// a load or a graph edit is picked up without a re-entry.
fn refresh_tags(anim: &mut AnimatorComponent, graph: Option<&AnimationGraph>) {
    let Some(graph) = graph else { return };
    let layers = anim.layers.iter_mut().map(|l| &mut l.playback);
    for (layer, playback) in std::iter::once(&mut anim.base).chain(layers).enumerate() {
        let node = playback.current_node.as_deref();
        let node = node.and_then(|n| graph.machine(layer)?.node(n));
        let tag = node.map(|n| n.tag.as_str()).filter(|t| !t.is_empty());
        if playback.states.tag.as_deref() != tag {
            playback.states.tag = tag.map(str::to_string);
        }
    }
}

#[cfg(test)]
#[path = "states_tests.rs"]
mod states_tests;
