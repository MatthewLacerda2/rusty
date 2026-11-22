use rusty_asset::anim_data::AnimationClip;
use rusty_asset::animation_graph::AnimationGraph;
use rusty_components::{
    AnimatorComponent, Bounds, Motion, Playback, Reported, StateCallback, StateEvent, StateInfo,
    Step,
};

use super::motion::MotionContext;

/// The base layer's name in a state's info, as Unity names it.
pub const BASE_LAYER: &str = "Base Layer";

struct Active<'a> {
    node: &'a str,
    entry: u32,
    raw: f32,
    current: bool,
    motion: Motion<'a>,
}

pub fn state_callbacks(
    anim: &mut AnimatorComponent,
    clips: &[AnimationClip],
    graph: Option<&AnimationGraph>,
) -> Vec<StateCallback> {
    let layer_count = anim.layers.len() + 1;
    let mut tracker = std::mem::take(&mut anim.states);
    let tracked = layer_count.max(tracker.layers.len());
    tracker.layers.resize(tracked, Vec::new());
    let mut out = Vec::new();
    for (layer, prior) in tracker.layers.iter_mut().enumerate() {
        // The travel `MotionContext::advance` left in `states.travel`, taken so it counts once.
        let moved = playback_mut(anim, layer).map_or(0.0, |p| std::mem::take(&mut p.states.travel));
        let ctx = MotionContext {
            clips,
            machine: graph.and_then(|g| g.machine(layer)),
            parameters: &anim.parameters,
        };
        *prior = diff_layer(anim, layer, &ctx, std::mem::take(prior), moved, &mut out);
    }
    tracker.layers.truncate(layer_count);
    anim.states = tracker;
    out
}

fn diff_layer(
    anim: &AnimatorComponent,
    layer: usize,
    ctx: &MotionContext<'_>,
    mut prior: Vec<Reported>,
    moved: f32,
    out: &mut Vec<StateCallback>,
) -> Vec<Reported> {
    let tag_of = |node: &str| {
        let tag = ctx
            .machine
            .and_then(|m| m.node(node))
            .map(|n| n.tag.as_str());
        tag.filter(|t| !t.is_empty()).map(str::to_string)
    };
    let actives = playback(anim, layer).map_or_else(Vec::new, actives);
    let (mut next, mut entered, mut updated) = (Vec::new(), Vec::new(), Vec::new());
    for active in actives {
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
            layer_name: layer_name(anim, layer),
            state: active.node.to_string(),
            tag: tag_of(active.node),
            ..timing(ctx, active.motion, total)
        };
        [&mut entered, &mut updated][usize::from(known.is_some())].push(info.clone());
        next.push(active.reported(total, info));
    }
    out.extend(prior.into_iter().map(|r| exit(r.info)));
    let callback = |event| move |info| StateCallback { event, info };
    out.extend(entered.into_iter().map(callback(StateEvent::Enter)));
    out.extend(updated.into_iter().map(callback(StateEvent::Update)));
    next
}

impl Active<'_> {
    fn reported(&self, total: f32, info: StateInfo) -> Reported {
        Reported {
            node: self.node.to_string(),
            entry: self.entry,
            raw: self.raw,
            total,
            current: self.current,
            info,
        }
    }
}

fn exit(info: StateInfo) -> StateCallback {
    StateCallback {
        event: StateEvent::Exit,
        info,
    }
}

fn playback(anim: &AnimatorComponent, layer: usize) -> Option<&Playback> {
    match layer {
        0 => Some(&anim.base),
        i => anim.layers.get(i - 1).map(|l| &l.playback),
    }
}

fn playback_mut(anim: &mut AnimatorComponent, layer: usize) -> Option<&mut Playback> {
    match layer {
        0 => Some(&mut anim.base),
        i => anim.layers.get_mut(i - 1).map(|l| &mut l.playback),
    }
}

fn layer_name(anim: &AnimatorComponent, layer: usize) -> String {
    match layer {
        0 => BASE_LAYER.to_string(),
        i => anim.layers[i - 1].name.clone(),
    }
}

/// The node a crossfade leaves first, then the current one: the order the callbacks report.
fn actives(playback: &Playback) -> Vec<Active<'_>> {
    let mut out = Vec::with_capacity(2);
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
    if let Some(node) = playback.current_node.as_deref() {
        out.push(Active {
            node,
            entry: playback.states.entry,
            raw: playback.time,
            current: true,
            motion: playback.motion(),
        });
    }
    out
}

/// A non-looping playhead's change once clamped, so its state holds at `normalized_time` 1;
/// the events read the raw span, which runs on past the end.
pub(super) fn clamped_travel(bounds: Bounds, step: Step) -> f32 {
    let (from, to) = step.span;
    match bounds {
        Bounds::Clamp(_) => bounds.apply(to) - bounds.apply(from),
        Bounds::Wrap(_) | Bounds::Open => to - from,
    }
}

/// `total` is the playhead unwrapped since entry: seconds for a clip, cycles for a tree.
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

#[cfg(test)]
#[path = "states_tests.rs"]
mod states_tests;

#[cfg(test)]
#[path = "states_edge_tests.rs"]
mod states_edge_tests;
