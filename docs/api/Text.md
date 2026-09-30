## `Text`

Read and tune an entity's `TextComponent` (#419) — the UI's label, TextMeshPro's
`TextMeshProUGUI` trimmed to what HUDs and menus need. It draws its string inside
the entity's laid-out rect from a **signed-distance-field** atlas, so it stays sharp
at any size and the outline, drop shadow and glow are nearly free. Sizes are in
canvas **reference units**; effect widths and letter spacing are in **ems** (a
fraction of the font size), so they scale with the text. Colours are
**display-space (sRGB) RGBA with straight alpha**, each `0..1`. Getters return a
neutral default (zeros, `false`, `nil`) when the entity has no Text; setters are then
no-ops. Adding a Text also adds a `RectTransform`. The model — fonts, wrapping,
overflow, auto-size, rich text — is in [`docs/ui.md`](../ui.md).

| Function | Signature | Returns |
|---|---|---|
| `Text.GetText` / `SetText` | `(id)` / `(id, string)` | the string (`\n` breaks a line; rich-text tags when on) |
| `Text.GetFont` / `SetFont` | `(id)` / `(id, path)` | `.ttf` / `.otf` path, or `nil` for the bundled default (`nil` / `""` clears it) |
| `Text.GetBoldFont` / `SetBoldFont` | `(id)` / `(id, path)` | the face `<b>` uses; `nil` synthesizes bold |
| `Text.GetItalicFont` / `SetItalicFont` | `(id)` / `(id, path)` | the face `<i>` uses; `nil` synthesizes italic |
| `Text.GetFontSize` / `SetFontSize` | `(id)` / `(id, size)` | em size in reference units (≥ 0.5) |
| `Text.GetColor` / `SetColor` | `(id)` / `(id, r, g, b, a)` | fill `r, g, b, a` (each clamped to 0..1) |
| `Text.GetAlignment` / `SetAlignment` | `(id)` / `(id, name)` | `"TopLeft"`, `"TopCenter"`, `"TopRight"`, `"MiddleLeft"`, `"MiddleCenter"`, `"MiddleRight"`, `"BottomLeft"`, `"BottomCenter"` or `"BottomRight"` |
| `Text.GetWrap` / `SetWrap` | `(id)` / `(id, bool)` | break lines at word boundaries to fit the rect's width |
| `Text.GetOverflow` / `SetOverflow` | `(id)` / `(id, name)` | `"Overflow"` (spill), `"Truncate"` (drop what does not fit) or `"Ellipsis"` (truncate, ending with `…`) |
| `Text.GetLineSpacing` / `SetLineSpacing` | `(id)` / `(id, multiplier)` | line pitch multiplier (1 = the font's own; ≥ 0) |
| `Text.GetLetterSpacing` / `SetLetterSpacing` | `(id)` / `(id, ems)` | extra advance per character, in ems (negative tightens) |
| `Text.GetAutoSize` / `SetAutoSize` | `(id)` / `(id, enabled[, min, max])` | `enabled, min, max` — pick the largest size in `[min, max]` that fits the rect (omitted bounds keep theirs) |
| `Text.GetRichText` / `SetRichText` | `(id)` / `(id, bool)` | parse the tag subset (off draws tags literally — for echoing user input) |
| `Text.GetRaycastTarget` / `SetRaycastTarget` | `(id)` / `(id, bool)` | whether the pointer can hit it (read by #420) |
| `Text.GetOutline` / `SetOutline` | `(id)` / `(id, width, r, g, b, a)` | `width, r, g, b, a` — thickness in ems (0 = none), drawn outside the glyph edge |
| `Text.GetShadow` / `SetShadow` | `(id)` / `(id, dx, dy, r, g, b, a)` | `dx, dy, r, g, b, a` — drop-shadow offset in reference units (0, 0 = none) |
| `Text.GetGlow` / `SetGlow` | `(id)` / `(id, size, r, g, b, a)` | `size, r, g, b, a` — glow reach past the edge in ems (0 = none): the neon look |
| `Text.GetPreferredSize` | `(id)` | `width, height` it wants: the widest line unwrapped, and the block height wrapped at the element's current rect width (Unity's `preferredWidth` / `preferredHeight`) |
| `Text.MeasureString` | `(id, string)` | `width, height` of `string` on one unwrapped block in the entity's font and size — plain text at its `font_size`, trailing spaces counted (`\n` still breaks; `0, 0` without a Text). Where an input field puts its caret. |
| `Text.GetLayout` | `(id)` | `{ width, height, lines, font_size, truncated }` as drawn in the current rect (`font_size` is auto-size's pick), or `nil` when the entity has no Text or no rect |

**Rich text** (on by default): `<color=#rrggbb>` / `<color=#rrggbbaa>`, `<b>`, `<i>`
and `<size=n>` (reference units), each closed by its `</…>` tag. Anything else — an
unknown or malformed tag — draws literally. Outline + glow + synthesized bold reach
at most **0.25 em** past the edge (the field's spread); wider values are capped. An
ammo counter is `SetText(id, ammo .. " / " .. reserve)`; a neon label is
`SetGlow(id, 0.15, 0, 1, 1, 0.8)`; a subtitle box is `SetWrap(id, true)` +
`SetAutoSize(id, true, 18, 36)`.
