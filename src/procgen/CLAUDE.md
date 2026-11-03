# src/procgen — Snyder, the procedural texture graph

Area rules for `src/procgen/`. Project-wide rules stay in the root `CLAUDE.md`.

## Adding an op
An op is **one file plus one row** (#1751), the way a post effect is a module in
every layer (`src/render/CLAUDE.md`, *Adding a post effect*). It edits no shared
`match`, enum body or table. The test-only `dummy`
(`catalog/color/dummy.rs`) is the worked example: grep `dummy` for its file and
its rows.

- **Its file** — `catalog/<family>/<op>.rs`, the family being its menu category:
  `generators/` (no inputs), `color/`, `vector/`, `math/`, `filter/`. It holds:
  - the **params struct** (serde `Serialize`/`Deserialize`; the recipe's fields,
    with `#[serde(default = "…")]` for an optional one and a private default fn in
    the same file). An op without params is a unit struct (`pub struct Invert;`).
    An enum param is declared in the same file with
    `#[serde(rename_all = "snake_case")]` and a `#[default]` variant when it has a
    default. A socketable param (one a link may drive, #1494) is a `Param`;
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
- **Its math** — in its own file when it is its own, like `dummy`'s. Math that
  siblings share goes in `ops/<family>` beside its kin, and the op's `run` calls it
  (most existing ops call `ops::color`, `ops::filter`, … that way). A family past
  about ten op files clusters into subfolders, as `dir-size` asks.
- **Its rows** — its `pub mod` line in `catalog/<family>/mod.rs`, and its row in
  `catalog!` (`catalog/mod.rs`): `Variant(family::op::Params)`, or
  `Variant = family::op::Op` for a unit op. Both lists are `// sorted:`, so two
  branches adding ops land on different lines. The row derives the `OpKind`
  variant (serde reads the tag from it), the re-export (`recipe::<Params>`), its
  place in `OPS` (what `Texture.Ops()` lists) and its dispatch.
- **Docs** — its entry in `docs/api/Texture.md`'s *The op-set* under its family, and
  any *Tiling* or *Units* row its params need.
- **Tests** — the catalog-wide tests run on it with nothing added:
  `recipe/catalog_tests.rs` (its tag, params, defaults and enum values against
  serde; its seed reaches the runner), `recipe/socket_tests.rs` (a link parses
  exactly on `socket` params), `ops/arity_tests.rs` (it runs on its fewest and most
  inputs, panics on fewer, reads its last optional input), `ops/link_tests.rs` (a
  link to a constant bakes like the number) and the Lua enum-case test. Its own
  behaviour gets its own tests beside its math.

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
