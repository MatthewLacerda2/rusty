//! The post-process chain's uniform (`postfx/params.wgsl`) against its Rust mirror:
//! every post pass binds it, the authored effects included (#397).

use std::mem::{offset_of, size_of};

use super::assert_layout;
use crate::render::postfx::PostParams;

#[test]
fn post_params_match_wgsl_layout() {
    let pairs = [
        ("inv_view_proj", offset_of!(PostParams, inv_view_proj)),
        ("view_proj", offset_of!(PostParams, view_proj)),
        ("camera_pos", offset_of!(PostParams, camera_pos)),
        ("sky", offset_of!(PostParams, sky)),
    ];
    assert_layout("PostParams", &pairs, size_of::<PostParams>());
}
