//! Depth of field as a post effect (#1445): a scene pass, first in the chain, over
//! the view depths the velocity target holds; motion blur then reads the focused
//! scene. A display-only render texture, the Scene view and a view with no target
//! (a shader preview) skip it.

use super::super::{History, Owner, Pass, PassCx, PostEffect};
use super::DofParams;
use crate::postfx::params::Gate;
use crate::postfx::params::{effect_runs, PostSource};
use crate::{RenderView, Renderer};
use rusty_components::DofMode;
use rusty_core::settings::graphics_options::GraphicsOption;
use rusty_core::settings::GraphicsSettings;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "depth_of_field",
    owner: Owner::Volume,
    gate: Gate::option(GraphicsOption::DepthOfField),
    history: History::None("blurs this frame's colour by this frame's depth"),
    module: None,
    composite: &[],
    grade: &[],
    passes: &[Pass::Scene {
        order: 10,
        run: focus,
    }],
};

fn focus(r: &Renderer, view: &mut RenderView, cx: &PassCx) {
    let runs = cx.effect.full && !cx.stack.editor_mode;
    let (post, graphics) = (cx.effect.post, cx.effect.graphics);
    let params = dof_params(post, graphics, view.scene_size().height).filter(|_| runs);
    let gpu = (&r.device, &r.queue);
    let passes = &r.post_passes.depth_of_field;
    view.post_fx
        .run_depth_of_field(gpu, passes, params, &r.gpu_timer);
}

/// The depth of field `post`'s volume runs this frame, for a view `height` pixels
/// tall at its quality's working resolution; `None` when [`dof_enabled`] does not hold.
fn dof_params(post: PostSource, graphics: GraphicsSettings, height: u32) -> Option<DofParams> {
    let dof = &post
        .volume
        .filter(|_| dof_enabled(post, graphics))?
        .depth_of_field;
    let levels = &graphics.quality.depth_of_field_resolution_divisor;
    let divisor = dof.quality.pick(levels, dof.resolution_divisor);
    DofParams::new(dof, height, divisor)
}

/// Whether `post`'s volume runs depth of field, through [`effect_runs`]: its mode is
/// not `Off` and the player's option allows it. The
/// velocity target, whose depths it reads, is written when this holds too.
pub(crate) fn dof_enabled(post: PostSource, graphics: GraphicsSettings) -> bool {
    let on = post
        .volume
        .is_some_and(|vc| vc.depth_of_field.mode != DofMode::Off);
    effect_runs(graphics, EFFECT.gate, on)
}
