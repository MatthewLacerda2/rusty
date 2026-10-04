//! src/shadergen/post_params.rs — the runtime params of a post-processing volume's
//! authored effect chain (#671): `Graphics.SetPostParam("damage_vignette.intensity", 0.7)`.
//!
//! A postfx module's runtime params are laid out exactly like a surface shader's
//! ([`ParamLayout`], its `<name>.params.json` sidecar), but the values live on the
//! active volume (`VisualCorrectionComponent::post_params`), not on a material, and a
//! volume runs a *chain* of modules. So one name addresses the whole chain: it is
//! checked against each effect's layout in run order, and every effect whose layout
//! resolves it draws with the stored value (the renderer packs each effect's uniform
//! from the same map). A name no effect resolves, or a wrong number of values, is an
//! error naming it and listing the runtime params the chain has.
//!
//! Values are stored under the name the script used, so a get answers with what the
//! matching set wrote; the renderer resolves each stored name per effect.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::blocks::find;
use super::params::{unknown_param, ParamLayout, ParamSlot};
use super::recipe::PassKind;
use super::DEFAULT_OUT_DIR;

/// The param layouts of a volume's effects, in run order.
#[derive(Clone, Debug, Default)]
pub struct PostChain {
    layouts: Vec<ParamLayout>,
}

impl PostChain {
    /// Read the sidecar of each module in `effects` from the authored-shader dir the
    /// renderer loads them from. A module that is not baked yet (or whose sidecar is
    /// unreadable) has no runtime params, as the renderer skips it.
    pub fn load(effects: &[String]) -> Self {
        let layouts = effects
            .iter()
            .map(|name| PathBuf::from(format!("{DEFAULT_OUT_DIR}/{name}.wgsl")))
            .map(|path| ParamLayout::read_beside(&path).unwrap_or_default())
            .collect();
        Self { layouts }
    }

    /// A chain over already-read layouts.
    pub fn from_layouts(layouts: Vec<ParamLayout>) -> Self {
        Self { layouts }
    }

    /// The slot `name` resolves to in the first effect that has it, with that
    /// effect's layout.
    pub fn resolve(&self, name: &str) -> Result<(&ParamLayout, &ParamSlot), String> {
        self.layouts
            .iter()
            .find_map(|l| l.resolve(name).ok().map(|s| (l, s)))
            .ok_or_else(|| unknown_param(name, &self.names()))
    }

    /// Every runtime param name of the chain, each effect's in slot order.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for n in self.layouts.iter().flat_map(ParamLayout::names) {
            if !names.contains(&n) {
                names.push(n);
            }
        }
        names
    }

    /// Store `value` for `name` in `values`: one number broadcasts to every lane.
    pub fn set(
        &self,
        values: &mut BTreeMap<String, Vec<f32>>,
        name: &str,
        value: Vec<f32>,
    ) -> Result<(), String> {
        let (layout, slot) = self.resolve(name)?;
        let value = layout.coerce(slot, value)?;
        values.insert(name.to_owned(), value);
        Ok(())
    }

    /// The value `name` draws with: the stored one, else its baked default.
    pub fn get(&self, values: &BTreeMap<String, Vec<f32>>, name: &str) -> Result<Vec<f32>, String> {
        let (_, slot) = self.resolve(name)?;
        Ok(values
            .get(name)
            .cloned()
            .unwrap_or_else(|| slot.default.clone()))
    }
}

/// A runtime postfx param's catalog default, for a volume-less read: `name` is
/// `"<block>.<param>"` or `"<block>.<index>.<param>"` naming a runtime param of a
/// postfx block.
pub fn catalog_default(name: &str) -> Result<Vec<f32>, String> {
    let parts: Vec<&str> = name.split('.').collect();
    let (block, param) = match parts[..] {
        [b, p] => (b, p),
        [b, i, p] if i.parse::<usize>().is_ok() => (b, p),
        _ => ("", ""),
    };
    find(PassKind::Postfx, block)
        .and_then(|b| b.params.iter().find(|p| p.name == param && p.runtime))
        .map(|p| vec![p.default; p.arity])
        .ok_or_else(|| unknown_param(name, &[]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shadergen::assemble::assemble_with_params;
    use crate::shadergen::recipe::{BlockSel, ShaderRecipe};

    fn layout(ids: &[&str]) -> ParamLayout {
        let recipe = ShaderRecipe {
            pass: PassKind::Postfx,
            name: "t".into(),
            blocks: ids
                .iter()
                .map(|id| BlockSel {
                    id: (*id).into(),
                    params: Default::default(),
                })
                .collect(),
        };
        assemble_with_params(&recipe, "").unwrap().1
    }

    #[test]
    fn a_name_resolves_in_any_effect_of_the_chain() {
        let chain = PostChain::from_layouts(vec![
            layout(&["grayscale"]),
            layout(&["damage_vignette", "film_grain"]),
        ]);
        let mut values = BTreeMap::new();
        assert_eq!(
            chain.get(&values, "damage_vignette.intensity").unwrap(),
            [0.5]
        );
        chain
            .set(&mut values, "damage_vignette.intensity", vec![0.9])
            .unwrap();
        chain
            .set(&mut values, "damage_vignette.color", vec![0.8])
            .unwrap();
        assert_eq!(
            chain.get(&values, "damage_vignette.intensity").unwrap(),
            [0.9]
        );
        assert_eq!(
            values["damage_vignette.color"], [0.8; 3],
            "one number broadcasts"
        );
    }

    #[test]
    fn unknown_baked_and_misfit_values_are_errors_naming_them() {
        let chain = PostChain::from_layouts(vec![layout(&["damage_vignette"])]);
        let mut values = BTreeMap::new();
        let baked = chain.set(&mut values, "damage_vignette.pulse_speed", vec![1.0]);
        assert!(baked.unwrap_err().contains("is baked"));
        let absent = chain
            .set(&mut values, "radial_blur.strength", vec![1.0])
            .unwrap_err();
        assert!(
            absent.contains("damage_vignette.intensity"),
            "lists the params: {absent}"
        );
        assert!(
            absent.contains("is not a runtime param"),
            "not called baked: {absent}"
        );
        let arity = chain.set(&mut values, "damage_vignette.color", vec![1.0, 0.0]);
        assert!(arity.unwrap_err().contains("takes 3 number(s)"));
        assert!(values.is_empty(), "a refused write stores nothing");
    }

    #[test]
    fn an_unbaked_chain_has_no_params_and_catalog_defaults_need_none() {
        let chain = PostChain::load(&["t671_never_baked".into()]);
        assert!(chain.names().is_empty());
        assert_eq!(catalog_default("damage_vignette.intensity").unwrap(), [0.5]);
        assert_eq!(
            catalog_default("damage_vignette.0.color").unwrap(),
            [1.0; 3]
        );
        assert!(catalog_default("posterize.levels").is_err());
        assert!(catalog_default("damage_vignette.x.intensity").is_err());
        assert!(catalog_default("nope").is_err());
    }
}
