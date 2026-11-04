# src/procgen — Snyder, the procedural texture graph

Area rules for `src/procgen/`. Project-wide rules stay in the root `CLAUDE.md`.

## Adding an op
An op is **one file plus one row** (#1751), the way a post effect is a module in
every layer (`src/render/CLAUDE.md`, *Adding a post effect*). It edits no shared
`match`, enum body or table. The test-only `dummy`
(`catalog/color/dummy.rs`) is the worked example: grep `dummy` for its file and
its rows.

- **Its file** — `catalog/<family>/<op>.rs`, the family being its menu category:
  `generators/` (no inputs), `color/`, `vector/`, `math/`, `filter/`. `filter/` is
  clustered by what an op reads (`blurs/`, `mask/`, `relief/`, below), so a filter's
  file is `catalog/filter/<cluster>/<op>.rs`, less a prefix every op of the cluster
  shares (`flood/fill.rs` holds `flood_fill`: the `prefix-group` rule). It holds:
  - the **params struct** (serde `Serialize`/`Deserialize`; the recipe's fields,
    with `#[serde(default = "…")]` for an optional one and a private default fn in
    the same file). An op without params is a unit struct (`pub struct Invert;`).
    A socketable param (one a link may drive, #1494) is a `Param`;
  - its **enums**, declared in the same file, `pub`, with
    `#[serde(rename_all = "snake_case")]` and a `#[default]` variant when it has a
    default. Anything else names one by its catalog path,
    `crate::procgen::catalog::<family>::<op>::<Enum>` (the family modules are
    `pub(crate)` for this): math in `ops/` does, as `ops/dummy.rs` names
    `catalog::color::dummy::DummyOutput`. Never add a new op's enum to
    `recipe/kinds.rs`: that file is the older ops' shared vocabulary, and appending
    to it is the collision this layout removes;
  - its **`Row`**: `const INFO: OpInfo = op(tag, category, arity, &[params…])`,
    built with `req`, `opt`, `choice`, `socket` and `SEED` from `recipe`. The tag
    is the variant's name in snake_case. `arity(min, max)` (or `SOURCE`, `ONE`)
    says how many inputs it accepts; optional inputs are the trailing ones;
  - its **`Op`**: `run(&self, cx: &Cx) -> Image`, and, only when it needs them,
    `seed` (it has a `seed` param), `sockets` (every `socket` param, by name, in
    row order), `arity` (a param narrows the row's arity, as `math`'s unary funcs)
    and `func` (what an arity error names). `Cx` gives `input(i)` for a required
    input, `optional(i)` for a trailing one, `own()` for a copy of the first to edit
    in place, `field(&param)` for a socketable value, `paint(sampler)` for a
    generator, `px(radius)` for a tile-unit radius in pixels, and `seed`, `size`,
    `sources`.
- **Its math** — in its own file when it is small and its own. Math that siblings
  share, or that needs its own tests, goes in `ops/<family>` beside its kin, and the
  op's `run` calls it (most existing ops call `ops::color`, `ops::filter`, … that
  way); `dummy` keeps its math in `ops/dummy.rs` to show that shape. A new math
  file is one line in `ops/mod.rs`'s sorted `mod` list.
- **Its rows** — its `pub mod` line in its folder's `mod.rs`, and its row in
  `catalog!` (`catalog/mod.rs`): `Variant(family::op::Params)`, or
  `Variant = family::op::Op` for a unit op (a `///` doc above a unit row lands on
  its variant). Both lists are `// sorted:`, so two branches adding ops land on
  different lines. The row derives the `OpKind` variant (serde reads the tag from
  it), the re-export (`recipe::<Params>`), its place in `OPS` (what `Texture.Ops()`
  lists) and its dispatch.
- **Docs** — its bullet in `docs/api/Texture.md`'s *The op-set*, one per op under a
  `<!-- sorted: -->` marker, naming its `category` and inputs; a bullet in *The
  params that link* for each `socket` param (same marker); and any *Tiling* or
  *Units* row its params need. *Seeds* lists no ops: a `seed=0` in the op's bullet
  is what says it is stochastic.
- **Tests** — the catalog-wide tests run on it with nothing added, and none holds a
  count or a fixture to edit:
  `recipe/catalog_tests.rs` (its tag, params, defaults and enum values against
  serde; its seed reaches the runner), `recipe/socket_tests.rs` (a link parses
  exactly on `socket` params), `ops/arity_tests.rs` (it runs on its fewest and most
  inputs, panics on fewer, reads its last optional input), `ops/link_tests.rs` (a
  link to a constant bakes like the number), `ops/seed_tests.rs` (a seeded op's
  bake changes with its seed, at its defaults or with one enum or optional number
  moved, and `seed = 0` bakes what no seed does) and the Lua enum-case test. A
  seeded op whose seed matters only under a setting none of those reach fails
  `seed_tests`: say so in that test's `settings`, rather than writing a fixture.
  Its own behaviour gets its own tests beside its math.

## Where the planned ops go
The families as #1739–#1743 fill them, so no branch clusters a folder another is
adding to. `dir-size` counts a folder's `.rs` files except its `mod.rs` and test files,
and fails past 12; an allow for a folder under the cap is itself refused
(`dir_size::check_dir`), so a full folder is clustered, never excused.

- `color/` (10 with `dummy`): `histogram_scan`, `levels` (#1743, landed): tone
  adjustments beside `auto_levels` and `gamma`.
- `filter/blurs/`: `directional_blur`, `slope_blur` (#1739); `highpass`, `sharpen`
  (#1743, landed): a kernel's smear, or the difference from one.
- `filter/flood/` (#1741, landed): `flood_fill`, `flood_random`, `flood_gradient`,
  `flood_bbox` (as `fill.rs`, …), their math in `ops/flood.rs` (the islands of a mask, on the torus).
- `filter/mask/`: `edge_detect` (#1743, landed), beside `distance`.
- `filter/relief/`: `ambient_occlusion` (#1743, landed), beside `cavity` and
  `curvature`. A filter's tests share `filter/test_probe.rs` (run one op on one
  image, roll it to show the op wraps).
- `generators/` (10): the anisotropic noise (#1740) makes 11, under the cap; the
  rest of #1740 is params on existing generators.
- `vector/` (7): `directional_warp` (#1739), beside `warp`; #1742 is params and
  inputs on `tile`.

A two-input op with an optional mask and an output mode is `dummy`'s shape:
`arity(2, 3)`, `cx.input(0)` and `cx.input(1)` read directly, `cx.optional(2)` for
the mask, and an `output` enum param written `OpParam { default:
Some(Lit::Text("…")), ..choice("output", &[…]) }`.

## Rules
- **Determinism.** procgen is a sim module: a stochastic op draws from
  `hash` with the node's seed (`cx.seed`), never `rand` or a clock, so a recipe
  bakes byte-identically.
- **Tiling.** A periodic param is a whole count of periods per tile
  (`ops::generators::count`), and a neighbourhood op wraps, so every map tiles.
- **Resolution-free units.** A reach is in tile units (`cx.px` turns it into
  pixels per axis), so a recipe looks the same at any resolution and aspect.
