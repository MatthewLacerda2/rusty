//! Check a recipe and order the nodes its targets need (#1485). Everything that can
//! be wrong with a recipe's wiring is refused here, before any node evaluates, so a
//! bake either runs whole or writes nothing.

use crate::core::collections::Map;

use super::RunError;
use crate::procgen::recipe::TextureRecipe;

/// The nodes to evaluate and how they connect, as indices into `recipe.nodes`.
pub(super) struct Plan {
    /// Every node a target needs, each after its inputs.
    pub order: Vec<usize>,
    /// Each node's inputs, in the node's order.
    pub inputs: Vec<Vec<usize>>,
    /// How many input slots of planned nodes read each node: its image is dropped
    /// when this many have run.
    pub uses: Vec<usize>,
    /// The requested outputs, in request order; never dropped.
    pub targets: Vec<usize>,
}

/// Check `recipe` whole and plan the evaluation of `targets`.
pub(super) fn plan(recipe: &TextureRecipe, targets: &[&str]) -> Result<Plan, RunError> {
    if recipe.resolution == 0 {
        return Err(RunError::BadResolution);
    }
    if recipe.nodes.is_empty() {
        return Err(RunError::Empty);
    }
    let by_id: Map<&str, usize> = recipe
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let inputs = recipe
        .nodes
        .iter()
        .map(|node| {
            node.check_arity().map_err(RunError::Arity)?;
            let index = |input: &String| {
                by_id
                    .get(input.as_str())
                    .copied()
                    .ok_or_else(|| RunError::UnknownInput {
                        node: node.id.clone(),
                        input: input.clone(),
                    })
            };
            node.inputs.iter().map(index).collect()
        })
        .collect::<Result<Vec<Vec<usize>>, _>>()?;
    let targets = targets
        .iter()
        .map(|id| {
            by_id
                .get(id)
                .copied()
                .ok_or_else(|| RunError::UnknownOutput(id.to_string()))
        })
        .collect::<Result<Vec<usize>, _>>()?;
    let order = Order::new(recipe, &inputs).visit_all(&targets)?;
    let mut uses = vec![0; recipe.nodes.len()];
    for &i in &order {
        for &j in &inputs[i] {
            uses[j] += 1;
        }
    }
    Ok(Plan {
        order,
        inputs,
        uses,
        targets,
    })
}

/// DFS visit state for cycle detection.
#[derive(Clone, Copy, PartialEq)]
enum Visit {
    Unseen,
    OnStack,
    Done,
}

/// A depth-first topological sort from the targets, inputs first.
struct Order<'a> {
    recipe: &'a TextureRecipe,
    inputs: &'a [Vec<usize>],
    state: Vec<Visit>,
    order: Vec<usize>,
}

impl<'a> Order<'a> {
    fn new(recipe: &'a TextureRecipe, inputs: &'a [Vec<usize>]) -> Self {
        Self {
            recipe,
            inputs,
            state: vec![Visit::Unseen; inputs.len()],
            order: Vec::new(),
        }
    }

    fn visit_all(mut self, targets: &[usize]) -> Result<Vec<usize>, RunError> {
        for &i in targets {
            self.visit(i)?;
        }
        Ok(self.order)
    }

    fn visit(&mut self, i: usize) -> Result<(), RunError> {
        match self.state[i] {
            Visit::Done => return Ok(()),
            Visit::OnStack => return Err(RunError::Cycle(self.recipe.nodes[i].id.clone())),
            Visit::Unseen => {}
        }
        self.state[i] = Visit::OnStack;
        for &j in &self.inputs[i] {
            self.visit(j)?;
        }
        self.state[i] = Visit::Done;
        self.order.push(i);
        Ok(())
    }
}
