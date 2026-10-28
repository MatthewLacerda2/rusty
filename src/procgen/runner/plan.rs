//! Check a recipe and order the nodes its targets need (#1485). Everything that can
//! be wrong with a recipe's wiring is refused here, before any node evaluates, so a
//! bake either runs whole or writes nothing: every node is checked (ids, arity,
//! inputs, cycles), not only the ones the targets reach.

use crate::core::collections::Map;

use super::RunError;
use crate::procgen::recipe::{Node, TextureRecipe};

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

/// Which nodes a plan evaluates.
#[derive(Clone, Copy)]
pub(super) enum Scope {
    /// Only the nodes the targets need.
    Needed,
    /// Every node in the recipe, each still dropped after its last consumer (#1486).
    Every,
}

/// Check `recipe` whole and plan the evaluation of `targets`.
pub(super) fn plan(
    recipe: &TextureRecipe,
    targets: &[&str],
    scope: Scope,
) -> Result<Plan, RunError> {
    if recipe.resolution == 0 {
        return Err(RunError::BadResolution);
    }
    if recipe.nodes.is_empty() {
        return Err(RunError::Empty);
    }
    let mut by_id: Map<&str, usize> = Map::default();
    for (i, n) in recipe.nodes.iter().enumerate() {
        if by_id.insert(n.id.as_str(), i).is_some() {
            return Err(RunError::DuplicateId(n.id.clone()));
        }
    }
    let inputs = wire(recipe, &by_id)?;
    let targets = targets
        .iter()
        .map(|id| {
            by_id
                .get(id)
                .copied()
                .ok_or_else(|| RunError::UnknownOutput(id.to_string()))
        })
        .collect::<Result<Vec<usize>, _>>()?;
    let all: Vec<usize> = (0..recipe.nodes.len()).collect();
    let mut order = Order::new(recipe, &inputs).visit_all(&all)?;
    if let Scope::Needed = scope {
        let needed = reachable(&inputs, &targets);
        order.retain(|&i| needed[i]);
    }
    let mut uses = vec![0; recipe.nodes.len()];
    for j in order.iter().flat_map(|&i| &inputs[i]) {
        uses[*j] += 1;
    }
    Ok(Plan {
        order,
        inputs,
        uses,
        targets,
    })
}

/// Every node's inputs as indices, after checking its arity and that each id exists.
fn wire(recipe: &TextureRecipe, by_id: &Map<&str, usize>) -> Result<Vec<Vec<usize>>, RunError> {
    let wire_node = |node: &Node| {
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
    };
    recipe.nodes.iter().map(wire_node).collect()
}

/// Which nodes `targets` read, directly or through other nodes (themselves included).
fn reachable(inputs: &[Vec<usize>], targets: &[usize]) -> Vec<bool> {
    let mut seen = vec![false; inputs.len()];
    let mut stack = targets.to_vec();
    while let Some(i) = stack.pop() {
        if !std::mem::replace(&mut seen[i], true) {
            stack.extend(&inputs[i]);
        }
    }
    seen
}

/// DFS visit state for cycle detection.
#[derive(Clone, Copy, PartialEq)]
enum Visit {
    Unseen,
    OnStack,
    Done,
}

/// A depth-first topological sort from the given roots, inputs first.
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

    fn visit_all(mut self, roots: &[usize]) -> Result<Vec<usize>, RunError> {
        for &i in roots {
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
