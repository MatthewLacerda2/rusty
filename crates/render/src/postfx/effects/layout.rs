use std::sync::LazyLock;

use super::{Composite, EffectCx, Grade, GradeCx, Pass, PostEffect, ScenePass, POST_EFFECTS};

pub(crate) struct Placed<F: 'static> {
    pub effect: &'static PostEffect,
    pub function: &'static F,
    pub offset: usize,
}

pub(crate) fn composite_layout() -> &'static [Placed<Composite>] {
    static LAYOUT: LazyLock<Vec<Placed<Composite>>> = LazyLock::new(|| {
        let mut all: Vec<_> = POST_EFFECTS
            .iter()
            .flat_map(|e| e.composite.iter().map(move |c| (e, c)))
            .collect();
        all.sort_by_key(|(_, c)| (c.phase, c.order));
        place(all, |c| c.slots)
    });
    &LAYOUT
}

pub(crate) fn grade_layout() -> &'static [Placed<Grade>] {
    static LAYOUT: LazyLock<Vec<Placed<Grade>>> = LazyLock::new(|| {
        let mut all: Vec<_> = POST_EFFECTS
            .iter()
            .flat_map(|e| e.grade.iter().map(move |g| (e, g)))
            .collect();
        all.sort_by_key(|(_, g)| g.order);
        place(all, |g| g.slots)
    });
    &LAYOUT
}

fn place<F>(
    sorted: Vec<(&'static PostEffect, &'static F)>,
    slots: fn(&F) -> usize,
) -> Vec<Placed<F>> {
    let mut offset = 0;
    sorted
        .into_iter()
        .map(|(effect, function)| {
            let placed = Placed {
                effect,
                function,
                offset,
            };
            offset += slots(function);
            placed
        })
        .collect()
}

/// At least one: WGSL has no empty array.
pub(super) fn span<F>(layout: &[Placed<F>], slots: fn(&F) -> usize) -> usize {
    let end = layout.last().map_or(0, |p| p.offset + slots(p.function));
    end.max(1)
}

pub(crate) fn composite_slots_len() -> usize {
    span(composite_layout(), |c| c.slots)
}

pub(crate) fn grade_slots_len() -> usize {
    span(grade_layout(), |g| g.slots)
}

pub(crate) fn composite_slots(cx: &EffectCx) -> Vec<[f32; 4]> {
    let mut slots = vec![[0.0; 4]; composite_slots_len()];
    for p in composite_layout() {
        let f = p.function;
        (f.pack)(cx, &mut slots[p.offset..p.offset + f.slots]);
    }
    slots
}

pub(crate) fn grade_slots(cx: &GradeCx) -> Vec<[f32; 4]> {
    let mut slots = vec![[0.0; 4]; grade_slots_len()];
    for p in grade_layout() {
        let g = p.function;
        (g.pack)(cx, &mut slots[p.offset..p.offset + g.slots]);
    }
    slots
}

pub(crate) fn scene_passes() -> &'static [ScenePass] {
    static PASSES: LazyLock<Vec<ScenePass>> = LazyLock::new(|| {
        let mut all: Vec<(u16, ScenePass)> = POST_EFFECTS
            .iter()
            .flat_map(|e| e.passes)
            .filter_map(|pass| match pass {
                Pass::Scene { order, run } => Some((*order, *run)),
                Pass::Ldr { .. } => None,
            })
            .collect();
        all.sort_by_key(|(order, _)| *order);
        all.into_iter().map(|(_, run)| run).collect()
    });
    &PASSES
}

pub(crate) fn ldr_passes() -> &'static [&'static Pass] {
    static PASSES: LazyLock<Vec<&'static Pass>> = LazyLock::new(|| {
        let mut all: Vec<(u16, &Pass)> = POST_EFFECTS
            .iter()
            .flat_map(|e| e.passes)
            .filter_map(|pass| match pass {
                Pass::Ldr { order, .. } => Some((*order, pass)),
                Pass::Scene { .. } => None,
            })
            .collect();
        all.sort_by_key(|(order, _)| *order);
        all.into_iter().map(|(_, pass)| pass).collect()
    });
    &PASSES
}
